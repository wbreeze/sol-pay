//! Behavioural tests for the flow in `state-machine.plantuml`.

use solana_signer::Signer;

use crate::harness::*;

const LIMIT: u64 = 500_000;
/// Comfortably more than the tests spend, unless a test is about running out.
const RICH: u64 = 10_000_000;

#[test]
fn settles_only_once_the_threshold_is_crossed() {
    let mut env = Env::new(RICH);
    env.open(LIMIT).unwrap();

    // One short of the threshold: usage accrues, nothing moves.
    for _ in 0..(VIEWS_TO_THRESHOLD - 1) {
        env.meter(1).unwrap();
    }
    let c = env.contract();
    assert_eq!(c.used, THRESHOLD - PAGE_PRICE);
    assert_eq!(c.paid, 0, "nothing collected below the threshold");
    assert_eq!(env.token_balance(&env.treasury), 0);

    // The view that reaches the threshold transfers the whole unpaid balance.
    env.meter(1).unwrap();
    let c = env.contract();
    assert_eq!(c.used, THRESHOLD);
    assert_eq!(c.paid, THRESHOLD, "settle clears the entire unpaid balance");
    assert_eq!(env.token_balance(&env.treasury), THRESHOLD);
}

#[test]
fn residue_stays_below_the_threshold() {
    // This is the property that made the design's final-collection step
    // unreachable, so it is worth asserting rather than assuming.
    let mut env = Env::new(RICH);
    env.open(LIMIT).unwrap();

    for _ in 0..137 {
        env.meter(1).unwrap();
        let c = env.contract();
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

    let views_to_limit = (LIMIT / PAGE_PRICE) as u32;
    env.meter(views_to_limit).unwrap();
    assert_eq!(env.contract().used, LIMIT);

    assert_error(env.meter(1), "LimitReached");
    assert_eq!(env.contract().used, LIMIT, "failed charge left usage alone");
}

#[test]
fn settle_and_increment_fail_together() {
    // The atomicity the design calls for: if the transfer cannot happen, the
    // usage bump that would have justified it must not stick either.
    let short = THRESHOLD - PAGE_PRICE; // enough to accrue, not enough to pay
    let mut env = Env::new(short);
    env.open(LIMIT).unwrap();

    for _ in 0..(VIEWS_TO_THRESHOLD - 1) {
        env.meter(1).unwrap();
    }
    let before = env.contract().used;
    assert_eq!(before, THRESHOLD - PAGE_PRICE);

    // This view crosses the threshold, so it must transfer — and cannot.
    let result = env.meter(1);
    assert!(result.is_err(), "settle should fail on an underfunded payer");
    assert_eq!(
        env.contract().used,
        before,
        "usage must not advance when the transfer fails"
    );
    assert_eq!(env.token_balance(&env.treasury), 0);
}

#[test]
fn open_requires_a_delegate_the_payer_actually_granted() {
    let mut env = Env::new(RICH);

    // No approve at all.
    let ix = env.ix_open(LIMIT);
    let payer = env.payer.insecure_clone();
    assert_error(
        env.send(&[ix], &[&payer], &payer.pubkey()),
        "DelegateNotSet",
    );

    // Approve, but to somebody else.
    let stranger = solana_pubkey::Pubkey::new_unique();
    let ixs = [env.ix_approve_to(&stranger, LIMIT), env.ix_open(LIMIT)];
    assert_error(
        env.send(&ixs, &[&payer], &payer.pubkey()),
        "DelegateMismatch",
    );

    // Approve the right delegate for too little.
    let ixs = [env.ix_approve(LIMIT - 1), env.ix_open(LIMIT)];
    assert_error(
        env.send(&ixs, &[&payer], &payer.pubkey()),
        "DelegateAllowanceTooLow",
    );

    assert!(!env.contract_exists(), "no contract from a failed open");
}

#[test]
fn rejects_a_limit_below_the_site_minimum() {
    let mut env = Env::new(RICH);
    assert_error(env.open(MIN_LIMIT - 1), "LimitBelowMinimum");
}

#[test]
fn renewal_forgives_what_was_paid() {
    let mut env = Env::new(RICH);
    env.open(LIMIT).unwrap();

    // Two calls, deliberately: one that settles, then a few views under the
    // threshold. A single 53-view call would transfer the lot and leave no
    // residue to carry.
    env.meter(VIEWS_TO_THRESHOLD).unwrap();
    env.meter(3).unwrap();
    let before = env.contract();
    assert_eq!(before.paid, THRESHOLD);
    let residue = before.used - before.paid;
    assert!(residue > 0);

    let ixs = [env.ix_approve(LIMIT), env.ix_renew(LIMIT)];
    let payer = env.payer.insecure_clone();
    env.send(&ixs, &[&payer], &payer.pubkey()).unwrap();

    let after = env.contract();
    assert_eq!(after.used, residue, "only the unpaid residue carries over");
    assert_eq!(after.paid, 0);
    assert_eq!(after.limit, LIMIT);
}

#[test]
fn close_leaves_the_residue_uncollected() {
    let mut env = Env::new(RICH);
    env.open(LIMIT).unwrap();

    env.meter(3).unwrap(); // below the threshold, so nothing was collected
    let residue = env.contract().used;
    assert!(residue > 0 && residue < THRESHOLD);
    assert_eq!(env.token_balance(&env.treasury), 0);

    let ix = env.ix_close();
    let payer = env.payer.insecure_clone();
    env.send(&[ix], &[&payer], &payer.pubkey()).unwrap();

    assert!(!env.contract_exists());
    assert_eq!(
        env.token_balance(&env.treasury),
        0,
        "residue below the threshold is forgiven, not collected"
    );
}

#[test]
fn metering_needs_the_site_authority() {
    let mut env = Env::new(RICH);
    env.open(LIMIT).unwrap();

    // The payer is not the server and must not be able to meter.
    let ix = env.ix_meter(1);
    let payer = env.payer.insecure_clone();
    let mut forged = ix.clone();
    forged.accounts[1].pubkey = payer.pubkey();
    assert!(
        env.send(&[forged], &[&payer], &payer.pubkey()).is_err(),
        "only the site authority may meter"
    );
}

#[test]
fn approve_replaces_rather_than_adds_to_the_allowance() {
    // Renewal depends on this: it passes the new limit outright.
    let mut env = Env::new(RICH);
    env.open(LIMIT).unwrap();
    assert_eq!(env.delegated_amount(&env.payer_ata), LIMIT);

    let ix = env.ix_approve(MIN_LIMIT);
    let payer = env.payer.insecure_clone();
    env.send(&[ix], &[&payer], &payer.pubkey()).unwrap();
    assert_eq!(env.delegated_amount(&env.payer_ata), MIN_LIMIT);
}
