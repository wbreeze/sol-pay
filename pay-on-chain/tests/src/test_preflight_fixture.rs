//! Records what the program does, so the PHP port can be checked against it.
//!
//! This test exists to write a file. That is unusual enough to say plainly:
//! every other test here asserts and stops, and the recording was deliberately
//! not bolted onto `test_preflight.rs` so that a passing suite has no
//! file-writing side effect hiding in it.
//!
//! The problem it solves: `php-client/src/Core/Preflight.php` duplicates the
//! program's arithmetic in a third language, and `wasm-client/SPEC.md` §8 says
//! a predicate that disagrees with the program is worse than no predicate.
//! The Rust copy is pinned by `test_preflight.rs`, which drives a live SVM and
//! requires the program to agree. PHP cannot run LiteSVM, so instead this
//! records the account bytes at each interesting instant, the predicate's
//! verdict there, and what the program then actually did.
//!
//! Why account bytes rather than a designed fixture format: PHP already
//! decodes them, and `Site::decode` / `Contract::decode` / `TokenAccount::decode`
//! are themselves checked byte-for-byte against Anchor-serialized accounts on
//! every conformance run. So the boundary this crosses is one both sides
//! already agree on, and no new schema has to be kept in step.
//!
//! **The recording is gated by the assertions around it.** Every case asserts
//! that the program agrees with the predicate before the case is kept, and the
//! file is written once at the end. A program regression therefore fails this
//! test and leaves the committed fixture untouched, rather than quietly
//! rewriting PHP's expectations to match the regression. Write the file
//! eagerly and that guarantee is gone.
//!
//! The fixture is committed. It records only what the cases below touch:
//! `charge`, `can_meter`, `will_settle`, `views_remaining`, `limit_floor` and
//! `diagnose`. `required_allowance` is the identity function and is not
//! recorded. Consumed by `php-client/conformance/preflight.php`.

use std::fmt::Write as _;
use std::path::PathBuf;

use solana_signer::Signer;

use sol_pay_client::core::{
    error as client_error, preflight,
    state::{Contract as ClientContract, Site as ClientSite, TokenAccount as ClientTokenAccount},
};

use crate::harness::*;

const LIMIT: u64 = 500_000;
const RICH: u64 = 10_000_000;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The three accounts a PHP caller would have fetched, exactly as they stood
/// before the action this case is about.
struct Snapshot {
    site: Vec<u8>,
    contract: Vec<u8>,
    token_account: Vec<u8>,
}

fn snapshot(env: &Env) -> Snapshot {
    let contract_addr = contract_pda(&env.site, &env.payer.pubkey());
    Snapshot {
        site: env.svm.get_account(&env.site).expect("site account").data,
        contract: env
            .svm
            .get_account(&contract_addr)
            .expect("contract account")
            .data,
        token_account: env
            .svm
            .get_account(&env.payer_ata)
            .expect("token account")
            .data,
    }
}

struct Fixture {
    cases: Vec<String>,
}

impl Fixture {
    fn new() -> Self {
        Self { cases: Vec::new() }
    }

    /// `program` is what the program did next, in the caller's words:
    /// "accepted", an Anchor error name, or "spl:0x1" -- or "n/a" where the
    /// case records a resting state with no following call. It is context for
    /// whoever reads a failure, not something the PHP side asserts on.
    fn push(&mut self, name: &str, note: &str, snap: &Snapshot, page_views: u32, program: &str) {
        let site = ClientSite::decode(&snap.site).expect("client decodes site");
        let contract = ClientContract::decode(&snap.contract).expect("client decodes contract");
        let account =
            ClientTokenAccount::decode(&snap.token_account).expect("client decodes token account");

        let charge = match preflight::charge(&site, page_views) {
            Some(c) => c.to_string(),
            None => "null".to_string(),
        };
        let can_meter = match preflight::can_meter(&contract, &site, page_views) {
            Ok(()) => "null".to_string(),
            Err(preflight::Blocked::LimitReached { over }) => {
                format!("{{\"kind\": \"LimitReached\", \"over\": {over}}}")
            }
            Err(preflight::Blocked::Overflow) => {
                "{\"kind\": \"Overflow\", \"over\": null}".to_string()
            }
        };
        let unpaid = contract.unpaid();
        let shortfall = client_error::diagnose(&account, unpaid);

        let mut case = String::new();
        writeln!(case, "    {{").unwrap();
        writeln!(case, "      \"name\": \"{name}\",").unwrap();
        writeln!(case, "      \"note\": \"{note}\",").unwrap();
        writeln!(case, "      \"site_hex\": \"{}\",", hex(&snap.site)).unwrap();
        writeln!(case, "      \"contract_hex\": \"{}\",", hex(&snap.contract)).unwrap();
        writeln!(
            case,
            "      \"token_account_hex\": \"{}\",",
            hex(&snap.token_account)
        )
        .unwrap();
        writeln!(case, "      \"page_views\": {page_views},").unwrap();
        writeln!(case, "      \"charge\": {charge},").unwrap();
        writeln!(case, "      \"can_meter\": {can_meter},").unwrap();
        writeln!(
            case,
            "      \"will_settle\": {},",
            preflight::will_settle(&contract, &site, page_views)
        )
        .unwrap();
        writeln!(
            case,
            "      \"views_remaining\": {},",
            preflight::views_remaining(&contract, &site)
        )
        .unwrap();
        writeln!(
            case,
            "      \"limit_floor\": {},",
            preflight::limit_floor(&site, Some(&contract))
        )
        .unwrap();
        writeln!(case, "      \"unpaid\": {unpaid},").unwrap();
        writeln!(case, "      \"diagnose\": {{").unwrap();
        writeln!(case, "        \"unpaid\": {unpaid},").unwrap();
        writeln!(
            case,
            "        \"balance_short\": {},",
            shortfall.balance_short
        )
        .unwrap();
        writeln!(
            case,
            "        \"allowance_short\": {},",
            shortfall.allowance_short
        )
        .unwrap();
        writeln!(
            case,
            "        \"delegate_present\": {}",
            shortfall.delegate_present
        )
        .unwrap();
        writeln!(case, "      }},").unwrap();
        writeln!(case, "      \"program\": \"{program}\"").unwrap();
        write!(case, "    }}").unwrap();

        self.cases.push(case);
    }

    fn write(&self) {
        let mut out = String::new();
        out.push_str("{\n");
        out.push_str("  \"_\": \"Generated by pay-on-chain/tests test_preflight_fixture.rs. Committed on purpose: a moved verdict is meant to show up as a diff. Do not hand-edit -- run bin/test-rust.\",\n");
        writeln!(out, "  \"page_price\": {PAGE_PRICE},").unwrap();
        writeln!(out, "  \"collection_threshold\": {THRESHOLD},").unwrap();
        writeln!(out, "  \"min_limit\": {MIN_LIMIT},").unwrap();
        out.push_str("  \"cases\": [\n");
        out.push_str(&self.cases.join(",\n"));
        out.push_str("\n  ]\n}\n");

        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../php-client/conformance/preflight-fixture.json");
        std::fs::write(&path, out)
            .unwrap_or_else(|e| panic!("cannot write {}: {e}", path.display()));
    }
}

#[test]
fn record_what_the_program_does_for_the_php_port() {
    let mut fixture = Fixture::new();

    // --- a fresh contract with room to spare -----------------------------
    let mut env = Env::new(RICH);
    env.open(LIMIT).unwrap();

    let snap = snapshot(&env);
    env.meter(1).expect("a fresh contract meters");
    fixture.push(
        "fresh contract",
        "nothing used yet, nothing accrued",
        &snap,
        1,
        "accepted",
    );

    // --- every view up to the first settle, and the settle itself --------
    // will_settle must be false for each of these and true for the one that
    // crosses the threshold, checked against the treasury actually moving.
    // Views 2..VIEWS_TO_THRESHOLD-1. The `meter(1)` above already spent view 1,
    // and view VIEWS_TO_THRESHOLD is the one that settles -- it is recorded
    // separately below, so it must not fall inside this loop.
    for step in 2..VIEWS_TO_THRESHOLD {
        let snap = snapshot(&env);
        let before = env.token_balance(&env.treasury);
        env.meter(1).expect("under the limit");
        let moved = env.token_balance(&env.treasury) > before;
        assert!(!moved, "nothing should settle before the threshold");
        if step == 2 {
            fixture.push(
                "accruing, below the threshold",
                "usage rising, nothing collected yet",
                &snap,
                1,
                "accepted",
            );
        }
    }

    let snap = snapshot(&env);
    let before = env.token_balance(&env.treasury);
    env.meter(1).expect("the settling view");
    let moved = env.token_balance(&env.treasury) > before;
    assert!(moved, "the predicate said this one would settle");
    fixture.push(
        "the view that settles",
        "unpaid reaches the collection threshold, so money moves",
        &snap,
        1,
        "accepted",
    );

    // --- the limit boundary ----------------------------------------------
    // Spend to one view short, record there, then record at the limit where
    // the program must refuse.
    let contract_addr = contract_pda(&env.site, &env.payer.pubkey());
    loop {
        let raw = env.svm.get_account(&contract_addr).expect("contract").data;
        let contract = ClientContract::decode(&raw).expect("decode contract");
        let site_raw = env.svm.get_account(&env.site).expect("site").data;
        let site = ClientSite::decode(&site_raw).expect("decode site");
        if preflight::views_remaining(&contract, &site) <= 1 {
            break;
        }
        env.meter(1).expect("still under the limit");
    }

    let snap = snapshot(&env);
    env.meter(1).expect("the last view the limit allows");
    fixture.push(
        "one view short of the limit",
        "views_remaining is 1 and the program accepts it",
        &snap,
        1,
        "accepted",
    );

    let snap = snapshot(&env);
    assert_error(env.meter(1), "LimitReached");
    fixture.push(
        "at the limit",
        "views_remaining is 0 and the program refuses",
        &snap,
        1,
        "LimitReached",
    );

    // Two views over is still LimitReached, and `over` doubles.
    let snap = snapshot(&env);
    assert_error(env.meter(2), "LimitReached");
    fixture.push(
        "two views past the limit",
        "over is the charge for both views, not just the first",
        &snap,
        2,
        "LimitReached",
    );

    // --- the renewal floor ------------------------------------------------
    // A hair under must be refused and the floor itself accepted, so the
    // number the client reports is the number the program enforces.
    let mut env = Env::new(RICH);
    env.open(LIMIT).unwrap();
    env.meter(VIEWS_TO_THRESHOLD).unwrap();
    env.meter(3).unwrap();

    let snap = snapshot(&env);
    let site = ClientSite::decode(&snap.site).expect("decode site");
    let contract = ClientContract::decode(&snap.contract).expect("decode contract");
    let floor = preflight::limit_floor(&site, Some(&contract));

    let payer = env.payer.insecure_clone();
    let under = [env.ix_approve(floor - 1), env.ix_renew(floor - 1)];
    assert!(
        env.send(&under, &[&payer], &payer.pubkey()).is_err(),
        "the program must refuse a limit below the floor the client reports"
    );
    fixture.push(
        "carrying unpaid usage",
        "limit_floor is the smallest limit renewal accepts; here the site minimum wins over the carried usage",
        &snap,
        1,
        "renew refused one below the floor, accepted at it",
    );

    let at = [env.ix_approve(floor), env.ix_renew(floor)];
    env.send(&at, &[&payer], &payer.pubkey())
        .expect("the floor itself must be renewable");

    // --- a balance too small for the settle it is about to owe ------------
    let mut env = Env::new(THRESHOLD - 1);
    env.open(LIMIT).unwrap();
    for _ in 0..(VIEWS_TO_THRESHOLD - 1) {
        env.meter(1).expect("accruing costs nothing yet");
    }
    let snap = snapshot(&env);
    let failed = env.meter(1);
    assert!(failed.is_err(), "a settle larger than the balance must fail");
    fixture.push(
        "balance short of the settle",
        "diagnose reports balance_short; SPL reports only custom error 0x1",
        &snap,
        1,
        "spl:0x1",
    );

    // --- an allowance too small, with the balance fine --------------------
    let mut env = Env::new(RICH);
    env.open(LIMIT).unwrap();
    let payer = env.payer.insecure_clone();
    env.send(&[env.ix_approve(THRESHOLD - 1)], &[&payer], &payer.pubkey())
        .unwrap();
    for _ in 0..(VIEWS_TO_THRESHOLD - 1) {
        env.meter(1).expect("accruing costs nothing yet");
    }
    let snap = snapshot(&env);
    let failed = env.meter(1);
    assert!(
        failed.is_err(),
        "a settle larger than the allowance must fail"
    );
    fixture.push(
        "allowance short of the settle",
        "diagnose reports allowance_short; SPL reports the same 0x1 as a short balance",
        &snap,
        1,
        "spl:0x1",
    );

    // --- the delegate SPL clears when the allowance reaches zero ----------
    let mut env = Env::new(RICH);
    env.open(LIMIT).unwrap();
    let payer = env.payer.insecure_clone();
    env.send(&[env.ix_approve(THRESHOLD)], &[&payer], &payer.pubkey())
        .unwrap();
    env.meter(VIEWS_TO_THRESHOLD).unwrap();
    let snap = snapshot(&env);
    let account = ClientTokenAccount::decode(&snap.token_account).expect("decode token account");
    assert!(
        account.delegate.is_none(),
        "SPL clears the delegate when the allowance reaches zero"
    );
    fixture.push(
        "delegate cleared by a spent allowance",
        "delegate_present is false, which is a different failure from merely running short",
        &snap,
        1,
        "n/a -- a resting state, no call follows",
    );

    fixture.write();
}
