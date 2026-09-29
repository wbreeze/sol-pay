//! Do the client's predicates agree with the program that enforces them?
//!
//! `core::preflight` duplicates the program's arithmetic, because the client
//! cannot call into it. Duplicated arithmetic drifts. So for each case a
//! predicate calls blocked, the matching call must actually fail here, with
//! the error the predicate named -- and where a predicate says a call would
//! succeed, it must.
//!
//! It also pins what is left of a question the delegate design had to ask:
//! which code SPL Token returns when a settle is short. With no allowance
//! there is one answer, and `core::error::shortfall` is the number beside it.

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
    let meter = env.svm.get_account(&env.meter_addr()).expect("meter account");
    (
        ClientSite::decode(&site.data).expect("client decodes site"),
        ClientMeter::decode(&meter.data).expect("client decodes meter"),
    )
}

fn client_fund_token_account(env: &Env) -> ClientTokenAccount {
    let acct = env.svm.get_account(&env.fund_ata).expect("token account");
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
    assert_eq!(preflight::can_meter(&meter, &site, 1, NOW), Ok(()));
    env.meter(1).expect("the predicate said this would work");

    // Now the limit is exactly reached, and one more item is over.
    let (site, meter) = client_view(&env);
    assert_eq!(preflight::items_remaining(&meter, &site), 0);
    assert_eq!(
        preflight::can_meter(&meter, &site, 1, NOW),
        Err(preflight::Blocked::LimitReached { over: ITEM_PRICE })
    );
    assert_error(env.meter(1), "LimitReached");
}

/// SPEC §8: `can_meter`'s clock argument and the program's `Clock::get()`
/// agree on `<=`, one second either side of the expiry.
#[test]
fn can_meter_and_the_program_agree_on_the_expiry_second() {
    let mut env = Env::new(RICH);
    env.open(LIMIT).unwrap();
    let (site, meter) = client_view(&env);

    env.set_now(meter.expiry);
    assert_eq!(preflight::can_meter(&meter, &site, 1, meter.expiry), Ok(()));
    env.meter(1).expect("the predicate said the expiry second meters");

    let (site, meter) = client_view(&env);
    env.set_now(meter.expiry + 1);
    assert_eq!(
        preflight::can_meter(&meter, &site, 1, meter.expiry + 1),
        Err(preflight::Blocked::Expired)
    );
    assert_error(env.meter(1), "Expired");
}

/// Expired and full at once reports expired, because the program checks the
/// expiry first.
#[test]
fn expiry_is_reported_before_the_limit() {
    let mut env = Env::new(RICH);
    env.open(LIMIT).unwrap();
    env.meter((LIMIT / ITEM_PRICE) as u32).unwrap();

    let (site, meter) = client_view(&env);
    let later = meter.expiry + 1;
    env.set_now(later);
    assert_eq!(
        preflight::can_meter(&meter, &site, 1, later),
        Err(preflight::Blocked::Expired)
    );
    assert_error(env.meter(1), "Expired");
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
    let under = env.ix_renew(floor - 1);
    assert!(
        env.as_reader(&[under]).is_err(),
        "the program must refuse a limit below the floor the client reports"
    );

    // The floor itself must be accepted.
    let at = env.ix_renew(floor);
    env.as_reader(&[at])
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
        (C::LimitBelowUsage, P::LimitBelowUsage),
        (C::MathOverflow, P::MathOverflow),
        (C::MintMismatch, P::MintMismatch),
        (C::Expired, P::Expired),
        (C::ExpiryInPast, P::ExpiryInPast),
        (C::Unauthorized, P::Unauthorized),
        (C::FundNotEmpty, P::FundNotEmpty),
        (C::FundHasMeters, P::FundHasMeters),
    ];
    for (client, program) in pairs {
        let code = u32::from(program);
        assert_eq!(client.code(), code, "{client:?} code disagrees with the program");
        assert_eq!(client_error::PayError::from_code(code), Some(client));
    }
    // One past the last variant is nobody's.
    assert_eq!(client_error::PayError::from_code(6012), None);
}

/// SPEC §8: the code SPL Token actually returns for a short fund, which is
/// all that is left of the ambiguity §6.4 used to have. A short balance is
/// custom error 1, and `shortfall` says by how much.
#[test]
fn a_short_fund_is_spl_error_1_and_shortfall_measures_it() {
    let mut env = Env::new(THRESHOLD - 1);
    env.open(LIMIT).unwrap();

    let failed = env
        .meter(ITEMS_TO_THRESHOLD)
        .expect_err("a settle larger than the fund must fail");
    assert!(
        failed.contains("0x1"),
        "expected SPL custom error 0x1 for a short balance, got: {failed}"
    );

    let account = client_fund_token_account(&env);
    assert_eq!(client_error::shortfall(&account, THRESHOLD), 1);
    assert_eq!(client_error::shortfall(&account, THRESHOLD - 1), 0);
}

/// SPEC §8: the other way a settle fails for lack of money is a frozen fund
/// token account, and SPL names it with a code of its own -- 17, which the
/// client knows as `TokenError::AccountFrozen`. Nothing to measure; the
/// balance is fine.
#[test]
fn a_frozen_fund_is_spl_error_17() {
    use anchor_spl::token::spl_token;
    use solana_program_pack::Pack;

    let mut env = Env::new(RICH);
    env.open(LIMIT).unwrap();

    let mut acct = env.svm.get_account(&env.fund_ata).expect("fund token account");
    let mut state = spl_token::state::Account::unpack(&acct.data).expect("unpacks");
    state.state = spl_token::state::AccountState::Frozen;
    state.pack_into_slice(&mut acct.data);
    env.svm.set_account(env.fund_ata, acct).unwrap();

    let failed = env
        .meter(ITEMS_TO_THRESHOLD)
        .expect_err("a settle from a frozen account must fail");
    assert!(
        failed.contains("0x11"),
        "expected SPL custom error 0x11 for a frozen account, got: {failed}"
    );
    assert_eq!(
        client_error::TokenError::from_code(17),
        Some(client_error::TokenError::AccountFrozen)
    );
    assert_eq!(client_error::shortfall(&client_fund_token_account(&env), THRESHOLD), 0);
}
