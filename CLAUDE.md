# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

Pay-as-you-go metering for site content on Solana. Two Rust crates, one PHP
proof of concept, no front end:

- `pay-on-chain/` — the Anchor program plus its LiteSVM test crate.
- `wasm-client/` — `sol-pay-client`, instruction builders published both to
  crates.io (native Rust core) and to npm as a browser bundle.
- `php-client/` — `wbreeze/sol-pay-client`, published on Packagist, covering
  the server-signed half of `wasm-client/src/core` for PHP servers with no
  Rust toolchain and no WASM runtime. Not in the Rust workspace; it *is*
  wired into `bin/` (`bin/test-php`) and CI (`php-conformance.yml`). It
  publishes from a mirror repository rather than from here — see below.

`wasm-client/SPEC.md` is the API specification and the reasoning behind the
public surface; `state-machine.plantuml` (rendered to `state-machine.png`) is
the design document the program implements. Read the relevant section of SPEC
before changing the client's public API.

## Commands

Run these from the repository root; each wraps `cargo` with `--locked` and the
right working directory.

```
bin/build-rust               # anchor build + wasm-pack build
bin/build-rust --program     # just the Anchor program
bin/build-rust --client      # just the browser bundle (wasm-client/pkg)
bin/test-rust                # both suites
bin/test-rust settles_only   # argument passes through as a cargo test filter
bin/clean [--program|--client]
bin/update-locks [--program|--client]   # cargo update inside ranges, then test
```

A single test: `bin/test-rust <name substring>`, or from inside a crate
directory, `cargo test --locked <substring>`.

`bin/test-rust` builds the program if `pay-on-chain/target/deploy/pay_on_chain.so`
is missing, but it does **not** rebuild after a program source change — run
`bin/build-rust --program` yourself when you edit the program.

Do not add a `cargo update` to a build path. Lock changes go through
`bin/update-locks`, which refuses to run with a dirty lock file so the diff is
legible, and runs the suites before it declares success.

### No validator

The program tests run against LiteSVM in-process. `solana-test-validator` is
needed only for a manual local deploy, and a fresh clone must run
`anchor keys sync` first (the program keypair is gitignored, so a fresh
`anchor build` generates a different address than `declare_id!` names). See
`pay-on-chain/README.md`.

### Toolchain

Each crate pins Rust 1.89.0 via its own `rust-toolchain.toml`; there is none at
the repository root. Any `rustup` command — notably
`rustup target add wasm32-unknown-unknown` — must therefore run **inside** the
crate directory, or it answers for the default toolchain instead of the pinned
one. This has broken CI before; the scripts and workflows carry comments saying
so.

Anchor 0.32.1, installed through `avm`.

`anchor build` compiles the on-chain program with a *third* toolchain,
separate from both `rust-toolchain.toml`'s 1.89.0 and whatever `rustup`
considers default: the Solana CLI's bundled SBF platform-tools, whose own
default rustc version is fixed by whichever Solana CLI release is
installed, not by anything in this repo. `.github/workflows/rust.yml` pins
that release explicitly (`release.anza.xyz/v3.1.14`, currently) rather than
`stable`, because `stable` resolved to a release whose default
platform-tools (v1.51) bundles rustc 1.84 -- too old to parse any
dependency's `Cargo.toml` that requires the `edition2024` feature, a
growing set as of 2026 (`zeroize`, `blake3`, `rand`, `indexmap`, the `toml`
crates, and others this repo pulls in transitively without depending on
directly). Bumping the pin forward when this recurs: check
`cargo build-sbf --version` locally, and prefer the smallest release that
fixes it (a platform-tools minor bump, not a Solana CLI major version) —
`agave-install init <version>` switches versions locally without losing the
previous one, so this is safe to try before committing to it in CI.

This was verified locally twice (a plain `anchor build` and a build against
an isolated, freshly-installed Solana `$HOME`, both succeeding with
`rustc 1.89`) but the *first* CI run with the pin still failed, on rustc
1.84 again, after printing an unexplained `✨ 2.3.0 initialized` line that
neither Anchor's nor `cargo-build-sbf`'s source has an obvious path to
produce. The one confirmed anomaly in that run: `Swatinem/rust-cache@v2`
restored a cache key from *before* the pin landed — stale `~/.cargo/bin`
and `target/` from the broken toolchain — so the `program` job's
`prefix-key` was bumped to force one clean cache, and a `Report SBF
toolchain` step (`cargo build-sbf --version`) was added right after the
install step so a recurrence is diagnosed from a two-line log instead of
guessed at from inside `anchor build`'s much longer output. If it recurs
after a clean cache, the mystery is real and not staleness — read that
diagnostic step's output first.

## Architecture

### The chain model

- `Site` PDA, seeds `["site", authority]` — one per site authority, holding
  `mint`, `treasury`, `page_price`, `collection_threshold`, `min_limit`.
- `Contract` PDA, seeds `["contract", site, payer]` — per payer per site,
  holding `limit`, `used`, `paid`.
- The payer SPL-`approve`s the **contract PDA** as delegate for the full limit.
  `open_contract` and `renew_contract` verify that delegation on chain
  (`require_delegate`) rather than trusting the client, so the `approve` must
  come earlier in the same transaction.
- `meter_and_settle` is signed by the site authority alone, with the payer
  absent. It adds `page_price * page_views` to `used`, and when the unpaid
  balance reaches `collection_threshold` it transfers the whole unpaid balance
  by CPI, signing as the contract PDA. Increment and transfer succeed or fail
  together.
- One token account has one delegate, so a payer holds one active contract per
  token account.

Two amounts are easy to confuse: the **spending limit** caps `used` and is what
the payer authorizes; the **collection threshold** is the smallest unpaid
balance worth a transfer.

### The client crate

`wasm-client/src/core/` is plain Rust — addresses, instruction and transaction
construction, account decoding, unit conversion, preflight arithmetic, error
naming — with no browser dependency and no I/O. Every function is a pure
function of its arguments: no RPC, no signing, no randomness, no ambient state.
`src/lib.rs` is a thin `wasm-bindgen` wrapper behind the opt-in `wasm` feature,
so a Rust server can depend on the crate without pulling in wasm-bindgen. The
default feature set is empty; `cargo test` therefore exercises the core only,
and the wasm layer is type-checked by building the bundle.

Anything that varies with the deployment or the token program hangs off
`core::Program`; the free functions in `core::pda`, `core::ix`, `core::tx` and
`core::error` are the same calls against the canonical deployment on SPL Token.
Well-known addresses are hardcoded in `core::ids` to keep the bundle small,
including `PAY_ON_CHAIN_ID`, which must equal `declare_id!` in the program.

### The design rule for the client's API

From SPEC §2: **be rigid where the chain is rigid, silent where the site has a
legitimate choice.** The library owns encoding, PDA derivation, decoding,
ordering constraints, preflight arithmetic and error mapping. It has no view on
what limit to suggest, how to format amounts, when to meter, how the site
identifies a visitor, or what to do when a payment fails. It never signs,
verifies signatures, builds sign-in messages, talks to an RPC, or stores
anything. An API that would decide site policy does not belong in this crate.

### Where the two sides meet

`pay-on-chain/tests/` is the only place the program and the client build
together, so the parity checks live there (`test_client_parity.rs`): instruction
bytes against Anchor's generated types, the client's hardcoded program id
against `declare_id!`, discriminators, account sizes, each `PayError` variant
against its discriminant, and — the important one — every preflight predicate
against actual LiteSVM behaviour, since a predicate that disagrees with the
program is worse than none. A claim SPEC makes about the program belongs in a
test here, not in prose.

Version constraint: the solana crates must all stay on the 2.x generation,
which is what `anchor-lang` 0.32.1, `litesvm` and `spl-token` agree on. Two
generations in one tree means two copies of `solana-pubkey` and a compile
failure. CI greps `cargo tree -d` for it; `pay-on-chain/tests/Cargo.toml` and
`wasm-client/Cargo.toml` carry comments explaining specific pins — read them
before changing a dependency.

### php-client

`php-client/src/Core/` (namespace `SolPay\Core`, PSR-4) covers the server row
of `wasm-client/SPEC.md` §3's "Two consumers" table — everything a PHP site
authority signs — and deliberately omits the payer-signed instructions
(`open_contract`, `renew_contract`, `close_contract`, `approve_checked`,
`revoke`): those are signed by a wallet adapter in the browser regardless of
what the server runs, so a PHP port gains nothing by having them.

| `wasm-client/src/core` | `php-client/src/Core` |
| --- | --- |
| `pda.rs` | `Pda` |
| `ix.rs` (server-signed subset) | `Ix` |
| `state.rs` | `Site`, `Contract`, `TokenAccount`, `Mint`, `Reader` (internal) |
| `preflight.rs` | `Preflight`, `Blocked` |
| `error.rs` | `PayError`, `TokenError`, `Cause`, `Shortfall` |
| `units.rs` | `Units` |
| `program.rs` | `Program` |
| `ids.rs` | `Ids` |

Every public pubkey is a base58 string, matching wasm-client's own JS
boundary (`wasm-client/README.md`, "addresses cross its boundary as base58
strings"). PHP has no unsigned 64-bit type, so this package's safe integer
ceiling is `PHP_INT_MAX` (~9.2e18) rather than `u64::MAX` (~1.8e19) —
`Reader::u64()`, `Preflight`, and `Units` each document this where it matters;
ordinary token amounts never approach either ceiling. Where Rust splits an API
into methods-on-a-handle plus free functions defaulting to the canonical
deployment, PHP just takes a `Program` explicitly (see `Ix`/`Program`'s class
docs) — same coverage, one implementation, since PHP doesn't have the
call-site friction that split exists to avoid.

`Curve25519.php`'s field arithmetic and the on-curve test were promoted into
`Fe`/`Ed25519` here from `php-client/pda-spike/php/`, namespaced and pruned
to what the live package needs (the spike's libsodium-comparison code stays
behind, since it exists only to demonstrate the pitfall below). The spike's
own copies stay frozen as the record of that dated experiment — don't "fix"
one without checking whether the other needs it too.

**Drift control.** `php-client/vectors-gen` is the one place a PHP claim about
the chain is checked against ground truth rather than transcribed by hand. It
sits beside `src/` rather than under `pda-spike/` because it stopped being the
spike's: `php-client/conformance/vectors.php`, `wasm-client/conformance/node.mjs`
and `pda-spike/php/verify.php` all read the one `vectors.json` it writes.
It emits five things, each sourced from the real crate or program rather
than copied:

- PDA derivation and one `meter_and_settle` instruction, from the published
  `sol-pay-client` crate (crates.io) — checked by `PdaTest`/`IxTest`.
- One genuine Anchor-serialized `Site` and `Contract` account — built from
  `pay-on-chain::state::{Site,Contract}` using their own `#[account]`-derived
  `DISCRIMINATOR` and `AnchorSerialize`, a path dependency on
  `pay-on-chain/programs/pay-on-chain` — checked by `StateTest`.
- The `PayError` code table, computed as `PayError::<variant> as u32 +
  anchor_lang::error::ERROR_CODE_OFFSET` against `pay-on-chain`'s own enum —
  checked by `ErrorTest`.
- The `TokenError` code table, read the same way from `spl_token::error::TokenError`
  (already in the tree transitively via `anchor-spl`) — also checked by `ErrorTest`.
- Three compiled legacy transaction messages and their wire bytes, from
  `solana-message` and `solana-transaction` — checked byte-for-byte against
  `Tx::compile`/`Tx::wire` by `conformance/vectors.php`. The three cases
  reach the branches one case cannot: an empty readonly-signer partition,
  cross-instruction flag merging, and the fee payer being prepended rather
  than sorted. `php-client/README.md`, "The order this has to happen in",
  says which is which — read it before adding or removing a case. **Those two crates are pinned to the solana 2.x generation
  and that is not a free choice** — `ix::meter_and_settle` returns a 2.x
  `Instruction`, and 4.x wants a 3.x one plus `solana_address::Address`, so
  reaching for current versions would mean rebuilding the instruction across
  a generation boundary by hand. The reasoning is in `vectors-gen/Cargo.toml`;
  read it before "updating" those pins.

`Preflight` and `Units` are pure arithmetic with no chain-serialized bytes to
cross-check; their PHPUnit tests mirror wasm-client's own `#[cfg(test)]`
modules test-for-test. For `Units` that is the appropriate level of rigor. For
`Preflight` it is not, and the gap is closed by a **second fixture with
different provenance**: `pay-on-chain/tests/src/test_preflight_fixture.rs` is
a test that exists to write a file, recording account bytes, the predicate's
verdict and what the program then did; `conformance/preflight.php` replays it.
Committed, from the *local* program, moved only by `bin/test-rust` — the
opposite of `vectors.json`, which is regenerated every run from the
*published* crate. **Do not merge the two**; they answer different questions.
Coverage is what the recorded cases reach — `requiredAllowance` and
`Blocked::Overflow` are not pinned, and the PHP script says so in place.
Widen it by adding a case to the Rust recorder: one place, both ports.

`SolPay\Tx` closes the gap that section describes: a PHP site authority can
now compile the message that carries an `Ix` instruction and frame it for the
wire. `compile` and `wire` are pure, legacy-message-only, and checked
byte-for-byte against Solana's own crates. **The rules to know before
touching it** — because both are invisible until a vector disagrees with you
— are that keys ascend by *raw pubkey bytes* inside each header partition
rather than by the order the instructions named them, and that the fee payer
is prepended rather than sorted and is forced writable even when the
instruction marked it readonly. `php-client/README.md`, "Transaction
assembly", carries the rest. What remains open there is devnet: nothing in
those vectors has a real signature, a current blockhash, or has paid a fee,
so the demonstrator is the first time this code meets a chain — and SPEC §7's
amendment is held until it has.

Regenerate and re-check after touching `state.rs`, `errors.rs`, `pda.rs`, or
`ix.rs`:

```
bin/test-php                   # regenerate vectors, check src/Core against them
cd php-client && composer test # PdaTest, IxTest, StateTest, ErrorTest, TxTest
```

`bin/test-php` replaced the three `cd` steps this used to list. It runs the
generator **every time**, then `php-client/conformance/vectors.php`, which
checks the **package** and not the spike: both PDA families with bumps, the
instruction data and every account's flags, both decoders, both error tables.
There is deliberately no skip-if-present guard — there was one, and a cached
fixture silently turned this from drift control into a check against whatever
the generator last said; the reasoning is in the script. `bin/test-node`
regenerates unconditionally too, for a different reason written there: it
shares the one fixture file, so one rule for both. `pda-spike/php/verify.php`
still runs and still only covers the spike's standalone `Pda`/`Base58`.

The PHPUnit tests hardcode the vector's values as literals rather than
reading `vectors.json` at runtime — same style `PdaTest`/`IxTest` already
used — so a drift shows up as a specific failing assertion pointing at the
stale literal, not a missing-fixture-file error. That is the complement of
the conformance run rather than a duplicate of it: literals cannot notice the
crate moving, and fresh vectors cannot tell you which local edit broke
something.

`php-client/pda-spike/README.md` carries a finding worth knowing before
touching `Ed25519`: PHP's `sodium` extension exposes no ed25519 core API on
any build tested so far, so the natural-looking shortcut
(`sodium_crypto_core_ed25519_is_valid_point`) doesn't exist and a stricter
substitute would silently derive wrong addresses on roughly half of all
inputs — which is why the on-curve test is hand-written rather than delegated
to libsodium.

**Running the suite:**

```
cd php-client && composer install && vendor/bin/phpunit --testdox
```

Composer isn't part of any other tool here, so installing it (`brew install
composer`) pulls in unversioned PHP as a dependency and can unlink a
version-pinned `php@X.Y` Homebrew formula from `php` on `PATH` — happened
once already, changing the default `php` from 8.4.25 to 8.5.10. The pinned
formula stays installed at its own prefix (e.g.
`/opt/homebrew/opt/php@8.4/bin/php`) if a specific version ever needs to be
reproduced.

## CI

- `.github/workflows/rust.yml` — on push and PR: locked core tests, the
  duplicate-solana-crate check, the browser bundle, and the program suite.
- `.github/workflows/dependency-drift.yml` — weekly, deliberately without the
  committed locks and without a build cache: re-resolves from scratch so an
  upstream release that breaks the manifest ranges surfaces as a red scheduled
  run. It never changes a lock; the fix is `bin/update-locks` and a commit.
- `.github/workflows/node-conformance.yml` — loads the browser bundle under
  Node and checks it against generated vectors. Guards the *loading contract*
  (`init()` taking bytes, since Node's fetch refuses `file:` URLs), not drift:
  Node runs the same wasm binary the browser does. `bin/test-node` by hand.
  See `wasm-client/SPEC.md` §3.1.
- `.github/workflows/php-conformance.yml` — runs the same vectors against
  `php-client/src/Core` on the floor `composer.json` declares and on the
  current PHP. This one *is* drift control: a second implementation can
  disagree with the crate. `bin/test-php` by hand. See SPEC §8.1.

The two conformance workflows generate vectors from the **published** crate
rather than from this tree, which is why both also watch `pay-on-chain/programs/`
-- the generator depends on it by path for genuinely Anchor-serialized account
bytes and the real error discriminants.

`php-client`'s PHPUnit suite (above) is still run by hand. It answers a
different question from the conformance job: its expected values are
hardcoded, so it names a local regression precisely and cannot notice the
crate moving.

## Conventions

- Comments in this repository explain *why*, often at length, and several
  record a mistake that has already cost a CI failure. Preserve them; when
  changing something a comment justifies, update the reasoning rather than
  deleting it.
- Version facts belong in a lock file or a CI job, not in a paragraph asking
  the reader to verify them by hand.
- `wasm-client/pkg/`, both `target/` directories, `php-client/vectors-gen/target/`,
  and `php-client/vectors-gen/vectors.json` are gitignored build/generated
  output — never edit them. `wasm-client/LICENSE-*` and `php-client/LICENSE-*`
  are intentional duplicates
  of the root licences so they ship inside the published artifacts.
- `php-client/.gitattributes` decides what the Composer **dist zip** carries:
  `pda-spike/`, `vectors-gen/`, `tests/`, `conformance/` and `phpunit.xml` are
  `export-ignore`d, so they stay in the split repository and out of every
  `composer require`. Anything new added under `php-client/` that is not
  library source needs a line there, and `bin/split-php-client` whitelists the
  package's top level so a missing line fails the split rather than shipping.
  The reasoning, and two `git archive` gotchas, are in `php-client/README.md`,
  "What the package ships".
- Publishing is deliberately unscripted (see `wasm-client/README.md`): it is
  rare, irreversible on crates.io, and needs personal credentials.
