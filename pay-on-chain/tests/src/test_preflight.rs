//! Do the client's predicates agree with the program that enforces them?
//!
//! `core::preflight` duplicates the program's arithmetic, because the client
//! cannot call into it. Duplicated arithmetic drifts. So for each case a
//! predicate calls blocked, the matching call must actually fail here, with
//! the error the predicate named -- and where a predicate says a call would
//! succeed, it must.
//!
//! This file also settles a question the published documentation does not
//! answer: which code SPL Token returns for a short balance versus a short
//! delegated allowance. `core::error::diagnose` exists because of the answer.

use solana_signer::Signer;

use sol_pay_client::core::{
    error as client_error, preflight,
    state::{Meter as ClientMeter, Site as ClientSite, TokenAccount as ClientTokenAccount},
};

use crate::harness::*;

const LIMIT: u64 = 500_000;
const RICH: u64 = 10_000_000;

/// The client's view of the on-chain accounts, read the way an integrator
/// would: fetch the account, decode it, ask the predicate.
fn client_view(env: &Env) -> (ClientSite, ClientMeter) {
    let site = env.svm.get_account(&env.site).expect("site account");
    let meter = env
        .svm
        .get_account(&meter_pda(&env.site, &env.reader.pubkey()))
        .expect("meter account");
    (
        ClientSite::decode(&site.data).expect("client decodes site"),
        ClientMeter::decode(&meter.data).expect("client decodes meter"),
    )
}

fn client_token_account(env: &Env) -> ClientTokenAccount {
    let acct = env.svm.get_account(&env.reader_ata).expect("token account");
    ClientTokenAccount::decode(&acct.data).expect("client decodes token account")
}

#[test]
fn can_meter_blocks_exactly_when_the_program_refuses() {
    let mut env = Env::new(RICH);
    env.open(LIMIT).unwrap();

    // Spend up to one item short of the limit.
    let items_under_limit = (LIMIT / ITEM_PRICE) as u32 - 1;
    for _ in 0..items_under_limit {
        env.meter(1).unwrap();
    }

    let (site, meter) = client_view(&env);
    assert_eq!(preflight::items_remaining(&meter, &site), 1);
    assert_eq!(preflight::can_meter(&meter, &site, 1), Ok(()));
    env.meter(1).expect("the predicate said this would work");

    // Now the limit is exactly reached, and one more item is over.
    let (site, meter) = client_view(&env);
    assert_eq!(preflight::items_remaining(&meter, &site), 0);
    assert_eq!(
        preflight::can_meter(&meter, &site, 1),
        Err(preflight::Blocked::LimitReached {
            over: ITEM_PRICE
        })
    );
    assert_error(env.meter(1), "LimitReached");
}

#[test]
fn will_settle_predicts_when_money_actually_moves() {
    let mut env = Env::new(RICH);
    env.open(LIMIT).unwrap();

    for _ in 0..(ITEMS_TO_THRESHOLD - 1) {
        let (site, meter) = client_view(&env);
        assert!(!preflight::will_settle(&meter, &site, 1));
        env.meter(1).unwrap();
        assert_eq!(env.token_balance(&env.treasury), 0, "nothing moved yet");
    }

    let (site, meter) = client_view(&env);
    assert!(preflight::will_settle(&meter, &site, 1));
    env.meter(1).unwrap();
    assert_eq!(
        env.token_balance(&env.treasury),
        THRESHOLD,
        "the predicate said this one would settle"
    );
}

#[test]
fn limit_floor_is_the_smallest_limit_renewal_accepts() {
    let mut env = Env::new(RICH);
    env.open(LIMIT).unwrap();

    // Settle once, then accrue a residue too small to collect.
    env.meter(ITEMS_TO_THRESHOLD).unwrap();
    env.meter(3).unwrap();

    let (site, meter) = client_view(&env);
    let floor = preflight::limit_floor(&site, Some(&meter));
    assert_eq!(floor, MIN_LIMIT.max(meter.unpaid()));

    // A hair under the floor must be refused by the program.
    let reader = env.reader.insecure_clone();
    let ixs = [env.ix_approve(floor - 1), env.ix_renew(floor - 1)];
    assert!(
        env.send(&ixs, &[&reader], &reader.pubkey()).is_err(),
        "the program must refuse a limit below the floor the client reports"
    );

    // The floor itself must be accepted.
    let ixs = [env.ix_approve(floor), env.ix_renew(floor)];
    env.send(&ixs, &[&reader], &reader.pubkey())
        .expect("the floor itself must be renewable");
}

/// Each client variant must carry the code Anchor assigned the program's.
///
/// Anchor leaves `#[error_code]` discriminants at 0..n and adds
/// `ERROR_CODE_OFFSET` in its generated `From<_> for u32`, so the conversion
/// is the only honest source for a code. The offset itself is pinned here too:
/// the client hardcodes 6000, and an Anchor upgrade that moved it would fail
/// on this line rather than mislabel every error at runtime.
#[test]
fn client_error_codes_match_the_program() {
    use client_error::PayError as C;
    use pay_on_chain::errors::PayError as P;

    assert_eq!(
        client_error::ANCHOR_ERROR_BASE,
        anchor_lang::error::ERROR_CODE_OFFSET,
        "the client's error base is no longer Anchor's"
    );

    let pairs = [
        (C::LimitBelowMinimum, P::LimitBelowMinimum),
        (C::MinimumBelowThreshold, P::MinimumBelowThreshold),
        (C::ZeroItemPrice, P::ZeroItemPrice),
        (C::LimitReached, P::LimitReached),
        (C::DelegateNotSet, P::DelegateNotSet),
        (C::DelegateMismatch, P::DelegateMismatch),
        (C::DelegateAllowanceTooLow, P::DelegateAllowanceTooLow),
        (C::LimitBelowUsage, P::LimitBelowUsage),
        (C::MathOverflow, P::MathOverflow),
    ];
    for (client, program) in pairs {
        let code = u32::from(program);
        assert_eq!(
            client.code(),
            code,
            "{client:?} code disagrees with the program"
        );
        assert_eq!(client_error::PayError::from_code(code), Some(client));
    }
}

/// The question the docs do not answer, and the reason `diagnose` exists.
///
/// A settle can fail two ways that a site must respond to differently: the
/// reader's balance is short (top up) or the delegated allowance is short
/// (re-authorize). If SPL distinguishes them by code, `diagnose` is redundant.
#[test]
fn spl_does_not_distinguish_a_short_balance_from_a_short_allowance() {
    // --- short balance: authorize plenty, hold almost nothing ---
    let mut env = Env::new(THRESHOLD - 1);
    env.open(LIMIT).unwrap();
    let short_balance = env
        .meter(ITEMS_TO_THRESHOLD)
        .expect_err("a settle larger than the balance must fail");

    let d = client_error::diagnose(&client_token_account(&env), THRESHOLD);
    assert_eq!(d.balance_short, 1, "client sees the balance shortfall");
    assert_eq!(d.allowance_short, 0, "the allowance was never the problem");
    assert!(d.delegate_present);

    // --- short allowance: hold plenty, authorize almost nothing ---
    let mut env = Env::new(RICH);
    env.open(LIMIT).unwrap();
    // Re-approve below what the next settle needs. `approve` replaces.
    let reader = env.reader.insecure_clone();
    env.send(&[env.ix_approve(THRESHOLD - 1)], &[&reader], &reader.pubkey())
        .unwrap();
    let short_allowance = env
        .meter(ITEMS_TO_THRESHOLD)
        .expect_err("a settle larger than the allowance must fail");

    let d = client_error::diagnose(&client_token_account(&env), THRESHOLD);
    assert_eq!(d.allowance_short, 1, "client sees the allowance shortfall");
    assert_eq!(d.balance_short, 0, "the balance was never the problem");
    assert!(d.delegate_present);

    // Both are SPL custom error 1. If this assertion ever fails because the
    // two diverge, `diagnose` can be deleted and the code read directly.
    assert!(
        short_balance.contains("0x1"),
        "expected SPL custom error 0x1 for a short balance, got: {short_balance}"
    );
    assert!(
        short_allowance.contains("0x1"),
        "expected SPL custom error 0x1 for a short allowance, got: {short_allowance}"
    );
}

/// Spending an allowance to zero clears the delegate outright, which is a
/// different failure from merely running short.
#[test]
fn an_allowance_spent_to_zero_clears_the_delegate() {
    let mut env = Env::new(RICH);
    env.open(LIMIT).unwrap();

    // Approve exactly one settle's worth, then take it.
    let reader = env.reader.insecure_clone();
    env.send(&[env.ix_approve(THRESHOLD)], &[&reader], &reader.pubkey())
        .unwrap();
    env.meter(ITEMS_TO_THRESHOLD).unwrap();
    assert_eq!(env.token_balance(&env.treasury), THRESHOLD);

    let account = client_token_account(&env);
    assert_eq!(account.delegated_amount, 0);
    assert!(
        account.delegate.is_none(),
        "SPL clears the delegate when the allowance reaches zero"
    );

    let d = client_error::diagnose(&account, ITEM_PRICE);
    assert!(!d.delegate_present, "diagnose must report the cleared delegate");
    assert!(!d.is_clear());
}
