# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

Pay-as-you-go metering for site content on Solana. Two Rust crates, one PHP
proof of concept, no front end:

- `pay-on-chain/` — the Anchor program plus its LiteSVM test crate.
- `wasm-client/` — `sol-pay-client`, instruction builders published both to
  crates.io (native Rust core) and to npm as a browser bundle.
- `php-client/` — not published, not yet a client. `php-client/pda-spike`
  proves a PHP server with no Rust toolchain and no WASM runtime can still
  derive this program's PDAs and build its instructions correctly. Not wired
  into `bin/`, CI, or the workspace — see below.

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

### php-client / pda-spike

`php-client/pda-spike/php/` is a from-scratch PHP port of the field
arithmetic behind `find_program_address` and Anchor's instruction encoding —
not a binding, since a typical PHP host has no FFI path into the Rust core.
It exists because SPEC §2 treats PDA derivation and instruction encoding as
chain facts the library should own so integrators never reimplement them, and
a Rust-only crate structurally can't reach PHP-only servers.

Its drift control mirrors `pay-on-chain/tests`': `vectors-gen/` is a small
Rust binary that pulls the published `sol-pay-client` crate from crates.io and
prints PDAs and one built instruction as JSON; `php/verify.php` reproduces the
same inputs and checks its own output byte-for-byte against that JSON. Rerun
it after any change to `wasm-client/src/core/pda.rs` or `ix.rs`:

```
cd php-client/pda-spike/vectors-gen && cargo run --release > ../php/vectors.json
cd ../php && php verify.php vectors.json
```

`php-client/pda-spike/README.md` also carries a second finding worth knowing
before touching this code: PHP's `sodium` extension exposes no ed25519 core
API on any build tested so far, so the natural-looking shortcut
(`sodium_crypto_core_ed25519_is_valid_point`) doesn't exist and a stricter
substitute would silently derive wrong addresses on roughly half of all
inputs — which is why the on-curve test is hand-written rather than delegated
to libsodium.

## CI

- `.github/workflows/rust.yml` — on push and PR: locked core tests, the
  duplicate-solana-crate check, the browser bundle, and the program suite.
- `.github/workflows/dependency-drift.yml` — weekly, deliberately without the
  committed locks and without a build cache: re-resolves from scratch so an
  upstream release that breaks the manifest ranges surfaces as a red scheduled
  run. It never changes a lock; the fix is `bin/update-locks` and a commit.

Neither workflow touches `php-client/`. Its conformance check (above) is
manual until it becomes more than a proof of concept.

## Conventions

- Comments in this repository explain *why*, often at length, and several
  record a mistake that has already cost a CI failure. Preserve them; when
  changing something a comment justifies, update the reasoning rather than
  deleting it.
- Version facts belong in a lock file or a CI job, not in a paragraph asking
  the reader to verify them by hand.
- `wasm-client/pkg/`, both `target/` directories, `php-client/pda-spike/vectors-gen/target/`,
  and `php-client/pda-spike/php/vectors.json` are gitignored build/generated
  output — never edit them. `wasm-client/LICENSE-*` are intentional duplicates
  of the root licences so they ship inside the published artifacts.
- Publishing is deliberately unscripted (see `wasm-client/README.md`): it is
  rare, irreversible on crates.io, and needs personal credentials.
