//! Behavioural tests for the fund design, SPEC §4.7 through §4.9, and the
//! program-side claims SPEC §8 lists.

use solana_keypair::Keypair;
use solana_signer::Signer;

use crate::harness::*;

const LIMIT: u64 = 500_000;
/// Comfortably more than the tests spend, unless a test is about running out.
const RICH: u64 = 10_000_000;

// --- metering, as before the redesign ---------------------------------------

#[test]
fn settles_only_once_the_threshold_is_crossed() {
    let mut env = Env::new(RICH);
    env.open(LIMIT).unwrap();
    assert_eq!(env.token_balance(&env.fund_ata), RICH, "the deposit landed");

    // One short of the threshold: usage accrues, nothing moves.
    for _ in 0..(ITEMS_TO_THRESHOLD - 1) {
        env.meter(1).unwrap();
    }
    let c = env.meter_account();
    assert_eq!(c.used, THRESHOLD - ITEM_PRICE);
    assert_eq!(c.paid, 0, "nothing collected below the threshold");
    assert_eq!(env.token_balance(&env.treasury), 0);

    // The item that reaches the threshold transfers the whole unpaid balance,
    // out of the fund.
    env.meter(1).unwrap();
    let c = env.meter_account();
    assert_eq!(c.used, THRESHOLD);
    assert_eq!(c.paid, THRESHOLD, "settle clears the entire unpaid balance");
    assert_eq!(env.token_balance(&env.treasury), THRESHOLD);
    assert_eq!(env.token_balance(&env.fund_ata), RICH - THRESHOLD);
}

#[test]
fn residue_stays_below_the_threshold() {
    // The property that makes closing's forgiveness bounded, so it is worth
    // asserting rather than assuming.
    let mut env = Env::new(RICH);
    env.open(LIMIT).unwrap();

    for _ in 0..137 {
        env.meter(1).unwrap();
        let c = env.meter_account();
        assert!(
            c.used - c.paid < THRESHOLD,
            "residue {} reached the threshold {}",
            c.used - c.paid,
            THRESHOLD
        );
    }
}

#[test]
fn refuses_to_carry_usage_past_the_limit() {
    let mut env = Env::new(RICH);
    env.open(LIMIT).unwrap();

    let items_to_limit = (LIMIT / ITEM_PRICE) as u32;
    env.meter(items_to_limit).unwrap();
    assert_eq!(env.meter_account().used, LIMIT);

    assert_error(env.meter(1), "LimitReached");
    assert_eq!(env.meter_account().used, LIMIT, "failed charge left usage alone");
}

#[test]
fn settle_and_increment_fail_together() {
    // If the transfer cannot happen, the usage bump that would have justified
    // it must not stick either. The fund is what runs short now.
    let short = THRESHOLD - ITEM_PRICE; // enough to accrue, not enough to pay
    let mut env = Env::new(short);
    env.open(LIMIT).unwrap();

    for _ in 0..(ITEMS_TO_THRESHOLD - 1) {
        env.meter(1).unwrap();
    }
    let before = env.meter_account().used;
    assert_eq!(before, THRESHOLD - ITEM_PRICE);

    // This item crosses the threshold, so it must transfer — and cannot.
    let result = env.meter(1);
    assert!(result.is_err(), "settle should fail on an underfunded fund");
    assert_eq!(
        env.meter_account().used,
        before,
        "usage must not advance when the transfer fails"
    );
    assert_eq!(env.token_balance(&env.treasury), 0);
}

#[test]
fn rejects_a_limit_below_the_site_minimum() {
    let mut env = Env::new(RICH);
    assert_error(env.open(MIN_LIMIT - 1), "LimitBelowMinimum");
    assert!(!env.exists(&env.fund), "the whole setup transaction rolled back");
}

#[test]
fn renewal_forgives_what_was_paid() {
    let mut env = Env::new(RICH);
    env.open(LIMIT).unwrap();

    // Two calls, deliberately: one that settles, then a few items under the
    // threshold. A single 53-item call would transfer the lot and leave no
    // residue to carry.
    env.meter(ITEMS_TO_THRESHOLD).unwrap();
    env.meter(3).unwrap();
    let before = env.meter_account();
    assert_eq!(before.paid, THRESHOLD);
    let residue = before.used - before.paid;
    assert!(residue > 0);

    let ix = env.ix_renew(LIMIT);
    env.as_reader(&[ix]).unwrap();

    let after = env.meter_account();
    assert_eq!(after.used, residue, "only the unpaid residue carries over");
    assert_eq!(after.paid, 0);
    assert_eq!(after.limit, LIMIT);
}

#[test]
fn metering_needs_the_site_authority() {
    let mut env = Env::new(RICH);
    env.open(LIMIT).unwrap();

    // The reader is not the server and must not be able to meter.
    let ix = env.ix_meter(1);
    let reader = env.reader.insecure_clone();
    let mut forged = ix.clone();
    forged.accounts[1].pubkey = reader.pubkey();
    assert!(
        env.send(&[forged], &[&reader], &reader.pubkey()).is_err(),
        "only the site authority may meter"
    );
}

// --- closing: residue, rent, and who may sign -------------------------------

#[test]
fn close_leaves_the_residue_uncollected_and_returns_the_rent() {
    let mut env = Env::new(RICH);
    env.open(LIMIT).unwrap();
    assert_eq!(env.fund_account().meters, 1);

    env.meter(3).unwrap(); // below the threshold, so nothing was collected
    let residue = env.meter_account().used;
    assert!(residue > 0 && residue < THRESHOLD);

    let meter_rent = env.lamports(&env.meter_addr());
    let reader_before = env.lamports(&env.reader.pubkey());

    // Key-signed, with the server paying the fee: the reader's lamports can
    // only have moved by the meter's rent.
    env.sign_out().unwrap();

    assert!(!env.meter_exists());
    assert_eq!(env.fund_account().meters, 0, "the fund counts it closed");
    assert_eq!(
        env.token_balance(&env.treasury),
        0,
        "residue below the threshold is forgiven, not collected"
    );
    assert_eq!(
        env.lamports(&env.reader.pubkey()),
        reader_before + meter_rent,
        "the rent goes to the reader, whoever signs"
    );
}

/// SPEC §8: the reader, the meter's key, and nobody else.
#[test]
fn the_reader_or_the_key_may_close_and_nobody_else() {
    // A stranger, fee paid by themselves.
    let mut env = Env::new(RICH);
    env.open(LIMIT).unwrap();
    let stranger = Keypair::new();
    env.svm.airdrop(&stranger.pubkey(), 1_000_000_000).unwrap();
    let ix = env.ix_close(&stranger.pubkey());
    assert_error(
        env.send(&[ix], &[&stranger], &stranger.pubkey()),
        "Unauthorized",
    );
    assert!(env.meter_exists());

    // The site authority is not a signer that may close either.
    let server = env.authority.insecure_clone();
    let ix = env.ix_close(&server.pubkey());
    assert_error(env.send(&[ix], &[&server], &server.pubkey()), "Unauthorized");

    // The reader.
    let ix = env.ix_close(&env.reader.pubkey());
    env.as_reader(&[ix]).unwrap();
    assert!(!env.meter_exists());

    // The key, on a fresh meter.
    let mut env = Env::new(RICH);
    env.open(LIMIT).unwrap();
    env.sign_out().unwrap();
    assert!(!env.meter_exists());
}

/// The key may close and nothing else: it cannot renew, which is how a
/// limit would be raised.
#[test]
fn the_key_cannot_renew() {
    let mut env = Env::new(RICH);
    env.open(LIMIT).unwrap();

    let mut ix = env.ix_renew(LIMIT * 2);
    ix.accounts[0].pubkey = env.key.pubkey();
    let key = env.key.insecure_clone();
    let server = env.authority.insecure_clone();
    assert_error(env.send(&[ix], &[&server, &key], &server.pubkey()), "Unauthorized");
    assert_eq!(env.meter_account().limit, LIMIT);
}

/// Naming a new key is how a second device takes over, and the old key is
/// dead the moment the renewal lands (SPEC §4.8).
#[test]
fn renewing_with_a_new_key_retires_the_old_one() {
    let mut env = Env::new(RICH);
    env.open(LIMIT).unwrap();

    let old = env.key.insecure_clone();
    let new = Keypair::new();
    let ix = env.ix_renew_with(&new.pubkey(), LIMIT, NOW + HOUR);
    env.as_reader(&[ix]).unwrap();
    assert_eq!(env.meter_account().key, new.pubkey());

    let server = env.authority.insecure_clone();
    let ix = env.ix_close(&old.pubkey());
    assert_error(
        env.send(&[ix], &[&server, &old], &server.pubkey()),
        "Unauthorized",
    );

    env.key = new;
    env.sign_out().unwrap();
    assert!(!env.meter_exists());
}

// --- expiry -------------------------------------------------------------------

/// SPEC §8: the boundary, one second either side. The program meters at
/// `now == expiry` and refuses at `expiry + 1`, which is what
/// `preflight::can_meter` assumes.
#[test]
fn expiry_ends_metering_at_the_boundary() {
    let mut env = Env::new(RICH);
    env.open(LIMIT).unwrap();
    let expiry = env.meter_account().expiry;

    env.set_now(expiry);
    env.meter(1).expect("the expiry second itself still meters");

    env.set_now(expiry + 1);
    assert_error(env.meter(1), "Expired");
}

#[test]
fn an_expiry_already_passed_is_refused() {
    let mut env = Env::new(RICH);
    env.expiry = NOW;
    assert_error(env.open(LIMIT), "ExpiryInPast");
}

/// An expired meter accepts a renewal from the reader and a close from either
/// signer; renewing is what brings it back.
#[test]
fn an_expired_meter_can_be_renewed_or_closed() {
    let mut env = Env::new(RICH);
    env.open(LIMIT).unwrap();
    env.set_now(NOW + HOUR + 1);
    assert_error(env.meter(1), "Expired");

    let ix = env.ix_renew_with(&env.key.pubkey(), LIMIT, NOW + 2 * HOUR);
    env.as_reader(&[ix]).unwrap();
    env.meter(1).expect("renewed, it meters again");

    env.set_now(NOW + 3 * HOUR);
    env.sign_out().expect("closing is allowed past the expiry");
}

// --- the fund -----------------------------------------------------------------

/// SPEC §8: the fund's seeds sign the transfer, so the only token account a
/// settle can draw on is one the fund owns. The reader's own account, however
/// full, is not one.
#[test]
fn a_settle_draws_only_on_the_funds_own_token_account() {
    let mut env = Env::new(RICH);
    // Deposit half; the reader keeps the rest in their own account.
    let ixs = [env.ix_open_fund(), env.ix_deposit(RICH / 2), env.ix_open(LIMIT)];
    env.as_reader(&ixs).unwrap();

    let mut forged = env.ix_meter(ITEMS_TO_THRESHOLD);
    forged.accounts[4].pubkey = env.reader_ata;
    let server = env.authority.insecure_clone();
    assert!(
        env.send(&[forged], &[&server], &server.pubkey()).is_err(),
        "the reader's own token account is not the fund's"
    );
    assert_eq!(env.token_balance(&env.reader_ata), RICH - RICH / 2);
}

/// SPEC §8: a meter answers to one fund. A settle naming another reader's
/// fund against this meter must fail.
#[test]
fn a_settle_cannot_substitute_another_fund() {
    let mut env = Env::new(RICH);
    env.open(LIMIT).unwrap();

    // A second fund of the same reader, same mint, index 1, with money in it.
    let other = fund_pda(&env.reader.pubkey(), &env.mint, 1);
    let ixs = [env.ix_open_fund_in(&env.mint.clone(), 1)];
    env.as_reader(&ixs).unwrap();

    let mut forged = env.ix_meter(1);
    forged.accounts[2].pubkey = other;
    forged.accounts[4].pubkey = fund_ata_of(&other, &env.mint);
    let server = env.authority.insecure_clone();
    assert!(
        env.send(&[forged], &[&server], &server.pubkey()).is_err(),
        "the meter names its fund; another will not do"
    );
}

/// SPEC §4.7: the index lets one reader hold several funds in one mint, and
/// an index in use cannot be opened twice.
#[test]
fn an_index_in_use_is_refused_and_another_is_not() {
    let mut env = Env::new(RICH);
    env.as_reader(&[env.ix_open_fund()]).unwrap();

    assert!(
        env.as_reader(&[env.ix_open_fund()]).is_err(),
        "the same index again must fail at account creation"
    );

    let mint = env.mint;
    env.as_reader(&[env.ix_open_fund_in(&mint, 1)])
        .expect("a second index in the same mint is a second fund");
    assert!(env.exists(&fund_pda(&env.reader.pubkey(), &mint, 1)));
}

/// Anyone may create an associated token account for any owner. A stranger
/// doing so for the fund's address first must not stop the reader opening it.
#[test]
fn a_token_account_created_first_by_a_stranger_does_not_block_the_fund() {
    let mut env = Env::new(RICH);
    let stranger = Keypair::new();
    env.svm.airdrop(&stranger.pubkey(), 1_000_000_000).unwrap();
    let ix = env.ix_create_ata(&stranger.pubkey(), &env.fund);
    env.send(&[ix], &[&stranger], &stranger.pubkey()).unwrap();
    assert!(env.exists(&env.fund_ata));

    env.open(LIMIT).expect("open_fund creates idempotently");
    assert_eq!(env.token_balance(&env.fund_ata), RICH);
}

/// SPEC §4.7: a fund meets only sites in its own mint, and the refusal comes
/// at setup rather than at the first settle.
#[test]
fn a_fund_meets_only_sites_in_its_mint() {
    let mut env = Env::new(RICH);
    let (other_mint, _) = env.add_mint(RICH);
    let other_fund = fund_pda(&env.reader.pubkey(), &other_mint, 0);

    let ixs = [
        env.ix_open_fund_in(&other_mint, 0),
        env.ix_open_at(&env.site, &other_fund, &env.key.pubkey(), LIMIT, NOW + HOUR),
    ];
    assert_error(env.as_reader(&ixs), "MintMismatch");
}

/// SPEC §4.7: one fund serves any number of sites, one meter at each, and
/// each site draws on the same balance.
#[test]
fn one_fund_serves_several_sites() {
    let mut env = Env::new(RICH);
    env.open(LIMIT).unwrap();
    let other = env.add_site();

    let ix = env.ix_open_at(&other.site, &env.fund, &env.key.pubkey(), LIMIT, NOW + HOUR);
    env.as_reader(&[ix]).unwrap();
    assert_eq!(env.fund_account().meters, 2);

    env.meter(ITEMS_TO_THRESHOLD).unwrap();
    let ix = env.ix_meter_at(&other.site, &other.authority.pubkey(), &other.treasury, ITEMS_TO_THRESHOLD);
    let signer = other.authority.insecure_clone();
    env.send(&[ix], &[&signer], &signer.pubkey()).unwrap();

    assert_eq!(env.token_balance(&env.treasury), THRESHOLD);
    assert_eq!(env.token_balance(&other.treasury), THRESHOLD);
    assert_eq!(env.token_balance(&env.fund_ata), RICH - 2 * THRESHOLD);

    // And a second meter at the same site from the same fund is refused: the
    // address is taken.
    let again = env.ix_open(LIMIT);
    assert!(env.as_reader(&[again]).is_err(), "one meter per site per fund");
}

/// SPEC §4.7: the reader takes back whatever is left whenever they like, and
/// nobody else can.
#[test]
fn only_the_reader_withdraws_and_may_do_so_with_meters_open() {
    let mut env = Env::new(RICH);
    env.open(LIMIT).unwrap();

    let stranger = Keypair::new();
    env.svm.airdrop(&stranger.pubkey(), 1_000_000_000).unwrap();
    let theirs = env.empty_token_account(&stranger.pubkey());
    let mut forged = env.ix_withdraw(&theirs, 1_000);
    forged.accounts[0].pubkey = stranger.pubkey();
    assert_error(
        env.send(&[forged], &[&stranger], &stranger.pubkey()),
        "Unauthorized",
    );

    let mine = env.empty_token_account(&env.reader.pubkey());
    let ix = env.ix_withdraw(&mine, RICH - 1_000);
    env.as_reader(&[ix]).unwrap();
    assert_eq!(env.token_balance(&mine), RICH - 1_000);
    assert_eq!(env.token_balance(&env.fund_ata), 1_000);
    assert!(env.meter_exists(), "withdrawing does not touch the meter");
}

/// SPEC §4.7: `close_fund` refuses while a meter is open or a balance
/// remains, and returns both rents when it succeeds.
#[test]
fn close_fund_needs_no_meters_and_no_balance() {
    let mut env = Env::new(RICH);
    env.open(LIMIT).unwrap();

    assert_error(env.as_reader(&[env.ix_close_fund()]), "FundHasMeters");

    let ix = env.ix_close(&env.reader.pubkey());
    env.as_reader(&[ix]).unwrap();
    assert_error(env.as_reader(&[env.ix_close_fund()]), "FundNotEmpty");

    let mine = env.empty_token_account(&env.reader.pubkey());
    let ix = env.ix_withdraw(&mine, RICH);
    env.as_reader(&[ix]).unwrap();

    let rents = env.lamports(&env.fund) + env.lamports(&env.fund_ata);
    let before = env.lamports(&env.reader.pubkey());
    let ix = env.ix_close_fund();
    let reader = env.reader.insecure_clone();
    let server = env.authority.insecure_clone();
    // The server pays the fee so the reader's lamports move by the rents alone.
    env.send(&[ix], &[&server, &reader], &server.pubkey()).unwrap();

    assert!(!env.exists(&env.fund));
    assert!(!env.exists(&env.fund_ata));
    assert_eq!(env.lamports(&env.reader.pubkey()), before + rents);
}

/// Nobody else can close a reader's fund, even an empty one.
#[test]
fn only_the_reader_closes_the_fund() {
    let mut env = Env::new(RICH);
    env.as_reader(&[env.ix_open_fund()]).unwrap();

    let stranger = Keypair::new();
    env.svm.airdrop(&stranger.pubkey(), 1_000_000_000).unwrap();
    let mut forged = env.ix_close_fund();
    forged.accounts[0].pubkey = stranger.pubkey();
    assert_error(
        env.send(&[forged], &[&stranger], &stranger.pubkey()),
        "Unauthorized",
    );
    assert!(env.exists(&env.fund));
}
