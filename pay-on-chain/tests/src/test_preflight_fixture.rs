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
//! decodes them, and `Site::decode` / `Meter::decode` / `TokenAccount::decode`
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
//! `charge`, `can_meter` (with the clock it was asked at), `will_settle`,
//! `items_remaining`, `limit_floor` and `shortfall`. Consumed by
//! `php-client/conformance/preflight.php`.
//!
//! Since the fund redesign (SPEC §4.7) the token account recorded is the
//! fund's, and each case carries the `now` its predicates were asked at, so
//! the expiry boundary is recorded like the limit boundary is.

use std::fmt::Write as _;
use std::path::PathBuf;

use sol_pay_client::core::{
    error as client_error, preflight,
    state::{Meter as ClientMeter, Site as ClientSite, TokenAccount as ClientTokenAccount},
};

use crate::harness::*;

const LIMIT: u64 = 500_000;
const RICH: u64 = 10_000_000;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The three accounts a PHP caller would have fetched, exactly as they stood
/// before the action this case is about, and the time it asked at.
struct Snapshot {
    site: Vec<u8>,
    meter: Vec<u8>,
    token_account: Vec<u8>,
    now: i64,
}

fn snapshot(env: &Env, now: i64) -> Snapshot {
    Snapshot {
        site: env.svm.get_account(&env.site).expect("site account").data,
        meter: env
            .svm
            .get_account(&env.meter_addr())
            .expect("meter account")
            .data,
        token_account: env
            .svm
            .get_account(&env.fund_ata)
            .expect("fund token account")
            .data,
        now,
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
    /// "accepted", an Anchor error name, or "spl:0x1". It is context for
    /// whoever reads a failure, not something the PHP side asserts on.
    fn push(&mut self, name: &str, note: &str, snap: &Snapshot, items: u32, program: &str) {
        let site = ClientSite::decode(&snap.site).expect("client decodes site");
        let meter = ClientMeter::decode(&snap.meter).expect("client decodes meter");
        let account =
            ClientTokenAccount::decode(&snap.token_account).expect("client decodes token account");

        let charge = match preflight::charge(&site, items) {
            Some(c) => c.to_string(),
            None => "null".to_string(),
        };
        let can_meter = match preflight::can_meter(&meter, &site, items, snap.now) {
            Ok(()) => "null".to_string(),
            Err(preflight::Blocked::Expired) => {
                "{\"kind\": \"Expired\", \"over\": null}".to_string()
            }
            Err(preflight::Blocked::LimitReached { over }) => {
                format!("{{\"kind\": \"LimitReached\", \"over\": {over}}}")
            }
            Err(preflight::Blocked::Overflow) => {
                "{\"kind\": \"Overflow\", \"over\": null}".to_string()
            }
        };
        let unpaid = meter.unpaid();
        let shortfall = client_error::shortfall(&account, unpaid);

        let mut case = String::new();
        writeln!(case, "    {{").unwrap();
        writeln!(case, "      \"name\": \"{name}\",").unwrap();
        writeln!(case, "      \"note\": \"{note}\",").unwrap();
        writeln!(case, "      \"site_hex\": \"{}\",", hex(&snap.site)).unwrap();
        writeln!(case, "      \"meter_hex\": \"{}\",", hex(&snap.meter)).unwrap();
        writeln!(
            case,
            "      \"token_account_hex\": \"{}\",",
            hex(&snap.token_account)
        )
        .unwrap();
        writeln!(case, "      \"items\": {items},").unwrap();
        writeln!(case, "      \"now\": {},", snap.now).unwrap();
        writeln!(case, "      \"charge\": {charge},").unwrap();
        writeln!(case, "      \"can_meter\": {can_meter},").unwrap();
        writeln!(
            case,
            "      \"will_settle\": {},",
            preflight::will_settle(&meter, &site, items)
        )
        .unwrap();
        writeln!(
            case,
            "      \"items_remaining\": {},",
            preflight::items_remaining(&meter, &site)
        )
        .unwrap();
        writeln!(
            case,
            "      \"limit_floor\": {},",
            preflight::limit_floor(&site, Some(&meter))
        )
        .unwrap();
        writeln!(case, "      \"unpaid\": {unpaid},").unwrap();
        writeln!(case, "      \"shortfall\": {shortfall},").unwrap();
        writeln!(case, "      \"program\": \"{program}\"").unwrap();
        write!(case, "    }}").unwrap();

        self.cases.push(case);
    }

    fn write(&self) {
        let mut out = String::new();
        out.push_str("{\n");
        out.push_str("  \"_\": \"Generated by pay-on-chain/tests test_preflight_fixture.rs. Committed on purpose: a moved verdict is meant to show up as a diff. Do not hand-edit -- run bin/test-rust.\",\n");
        writeln!(out, "  \"item_price\": {ITEM_PRICE},").unwrap();
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

    // --- a fresh meter with room to spare -----------------------------
    let mut env = Env::new(RICH);
    env.open(LIMIT).unwrap();

    let snap = snapshot(&env, NOW);
    env.meter(1).expect("a fresh meter meters");
    fixture.push(
        "fresh meter",
        "nothing used yet, nothing accrued",
        &snap,
        1,
        "accepted",
    );

    // --- every item up to the first settle, and the settle itself --------
    // will_settle must be false for each of these and true for the one that
    // crosses the threshold, checked against the treasury actually moving.
    // Items 2..ITEMS_TO_THRESHOLD-1. The `meter(1)` above already spent item 1,
    // and item ITEMS_TO_THRESHOLD is the one that settles -- it is recorded
    // separately below, so it must not fall inside this loop.
    for step in 2..ITEMS_TO_THRESHOLD {
        let snap = snapshot(&env, NOW);
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

    let snap = snapshot(&env, NOW);
    let before = env.token_balance(&env.treasury);
    env.meter(1).expect("the settling item");
    let moved = env.token_balance(&env.treasury) > before;
    assert!(moved, "the predicate said this one would settle");
    fixture.push(
        "the item that settles",
        "unpaid reaches the collection threshold, so money moves",
        &snap,
        1,
        "accepted",
    );

    // --- the limit boundary ----------------------------------------------
    // Spend to one item short, record there, then record at the limit where
    // the program must refuse.
    loop {
        let raw = env.svm.get_account(&env.meter_addr()).expect("meter").data;
        let meter = ClientMeter::decode(&raw).expect("decode meter");
        let site_raw = env.svm.get_account(&env.site).expect("site").data;
        let site = ClientSite::decode(&site_raw).expect("decode site");
        if preflight::items_remaining(&meter, &site) <= 1 {
            break;
        }
        env.meter(1).expect("still under the limit");
    }

    let snap = snapshot(&env, NOW);
    env.meter(1).expect("the last item the limit allows");
    fixture.push(
        "one item short of the limit",
        "items_remaining is 1 and the program accepts it",
        &snap,
        1,
        "accepted",
    );

    let snap = snapshot(&env, NOW);
    assert_error(env.meter(1), "LimitReached");
    fixture.push(
        "at the limit",
        "items_remaining is 0 and the program refuses",
        &snap,
        1,
        "LimitReached",
    );

    // Two items over is still LimitReached, and `over` doubles.
    let snap = snapshot(&env, NOW);
    assert_error(env.meter(2), "LimitReached");
    fixture.push(
        "two items past the limit",
        "over is the charge for both items, not just the first",
        &snap,
        2,
        "LimitReached",
    );

    // --- the renewal floor ------------------------------------------------
    // A hair under must be refused and the floor itself accepted, so the
    // number the client reports is the number the program enforces.
    let mut env = Env::new(RICH);
    env.open(LIMIT).unwrap();
    env.meter(ITEMS_TO_THRESHOLD).unwrap();
    env.meter(3).unwrap();

    let snap = snapshot(&env, NOW);
    let site = ClientSite::decode(&snap.site).expect("decode site");
    let meter = ClientMeter::decode(&snap.meter).expect("decode meter");
    let floor = preflight::limit_floor(&site, Some(&meter));

    let under = env.ix_renew(floor - 1);
    assert!(
        env.as_reader(&[under]).is_err(),
        "the program must refuse a limit below the floor the client reports"
    );
    fixture.push(
        "carrying unpaid usage",
        "limit_floor is the smallest limit renewal accepts; here the site minimum wins over the carried usage",
        &snap,
        1,
        "renew refused one below the floor, accepted at it",
    );

    let at = env.ix_renew(floor);
    env.as_reader(&[at])
        .expect("the floor itself must be renewable");

    // --- a fund too small for the settle it is about to owe ---------------
    let mut env = Env::new(THRESHOLD - 1);
    env.open(LIMIT).unwrap();
    for _ in 0..(ITEMS_TO_THRESHOLD - 1) {
        env.meter(1).expect("accruing costs nothing yet");
    }
    let snap = snapshot(&env, NOW);
    let failed = env.meter(1);
    assert!(
        failed.map_err(|e| e.contains("0x1")) == Err(true),
        "a settle larger than the fund must fail with SPL custom error 1"
    );
    fixture.push(
        "fund short of the settle",
        "shortfall is what the fund's token account lacks; SPL reports custom error 0x1",
        &snap,
        1,
        "spl:0x1",
    );

    // --- the expiry boundary ----------------------------------------------
    // The expiry second itself meters; the next one is refused.
    let mut env = Env::new(RICH);
    env.open(LIMIT).unwrap();
    let expiry = env.meter_account().expiry;

    env.set_now(expiry);
    let snap = snapshot(&env, expiry);
    env.meter(1).expect("the expiry second still meters");
    fixture.push(
        "at the expiry second",
        "now equals the expiry and the program still meters",
        &snap,
        1,
        "accepted",
    );

    env.set_now(expiry + 1);
    let snap = snapshot(&env, expiry + 1);
    assert_error(env.meter(1), "Expired");
    fixture.push(
        "one second past the expiry",
        "can_meter reports Expired, and the program refuses",
        &snap,
        1,
        "Expired",
    );

    fixture.write();
}
