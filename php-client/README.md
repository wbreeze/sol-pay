# sol-pay-client (PHP)

A server-side PHP port of `wasm-client`'s core, for a site whose server has no
Rust toolchain and no WASM runtime. Packaged as `wbreeze/sol-pay-client` on
Composer; not published anywhere. See `wasm-client/SPEC.md` for the design
this mirrors, and `wasm-client/README.md` for the same crate in Rust/JS form.

## What is here, and what is deliberately not

`wasm-client/SPEC.md` §3 splits an integration into two consumers: a browser,
which signs as the payer via a wallet adapter, and a server, which signs as
the site authority. This package covers the server row only:

| `wasm-client/src/core` | `src/Core` |
| --- | --- |
| `pda.rs` | `Pda` — `siteAddress`, `contractAddress` |
| `ix.rs` (server-signed subset) | `Ix` — `initializeSite`, `meterAndSettle` |
| `state.rs` | `Site`, `Contract`, `TokenAccount`, `Mint`, `Reader` (internal) |
| `preflight.rs` | `Preflight`, `Blocked` |
| `error.rs` | `PayError`, `TokenError`, `Cause`, `Shortfall` |
| `units.rs` | `Units` |
| `program.rs` | `Program` |
| `ids.rs` | `Ids` |

The payer-signed instructions — `open_contract`, `renew_contract`,
`close_contract`, `approve_checked`, `revoke` — and `tx.rs`'s ordered pairing
of them are absent on purpose. Wallet Standard is browser JavaScript, so
those are signed in the browser regardless of what language the server runs;
a PHP port gains nothing by having them. Signing, RPC, and storage are out of
scope here for the same reason `wasm-client` leaves them out — see SPEC §7.

## Conventions

Every public pubkey is a base58 string, in and out — matching
`wasm-client`'s own JS boundary (`wasm-client/README.md`, "addresses cross
its boundary as base58 strings, never as a `Pubkey` object"). There is no
`Pubkey` type here at all; `Base58::decode`/`encode` are the only place raw
address bytes exist, and only internally.

PHP has no unsigned 64-bit integer. This package's safe integer ceiling is
`PHP_INT_MAX` (~9.2e18), not `u64::MAX` (~1.8e19) — `Reader::u64()`,
`Preflight`, and `Units` each document this where it matters. Ordinary token
amounts, prices, and limits never come close to either ceiling; the
difference only matters at the extreme.

Where Rust splits an API into methods on a deployment handle plus free
functions defaulting to the canonical one — avoiding `Program::default()` at
every call site — this package just takes a `Program` explicitly (`Ix`,
`Cause::of`) or an optional program id (`Pda`). Same coverage, one
implementation: PHP doesn't have the call-site friction that split exists to
avoid in Rust.

## Building and testing

```
composer install
composer test          # vendor/bin/phpunit --testdox works the same way
```

Requires PHP ^8.1. Not wired into `bin/build-rust`, `bin/test-rust`, or any
CI workflow — this package has no Rust toolchain dependency and no reason to
share theirs; see the repository root `CLAUDE.md` for why that isolation is
deliberate rather than an oversight.

## Drift control

Nothing here can call into the Rust core, so nothing here is provably right
without a check against it. `php-client/pda-spike/vectors-gen` is that
check: a small Rust binary, unpublished, that emits four things sourced from
the real crate and program rather than transcribed by hand —

- PDA derivation and one `meter_and_settle` instruction, from the published
  `sol-pay-client` crate on crates.io.
- A genuine Anchor-serialized `Site` and `Contract` account, built from
  `pay-on-chain::state::{Site,Contract}`'s own `#[account]`-derived
  `DISCRIMINATOR` and `AnchorSerialize`.
- The `PayError` code table, computed as `PayError::<variant> as u32 +
  anchor_lang::error::ERROR_CODE_OFFSET` against `pay-on-chain`'s own enum.
- The `TokenError` code table, read from `spl_token::error::TokenError`.

`tests/Core/PdaTest.php`, `IxTest.php`, `StateTest.php`, and `ErrorTest.php`
assert against those values as hardcoded literals, so a mismatch surfaces as
a specific failing assertion pointing at the stale one, not a missing file.
Regenerate and re-run after touching `state.rs`, `errors.rs`, `pda.rs`, or
`ix.rs` on the Rust side:

```
cd pda-spike/vectors-gen && cargo run --release > ../php/vectors.json
cd ../php && php verify.php vectors.json   # the spike's own check: Pda/Ix only
cd ../.. && composer test                  # the four PHPUnit suites above
```

`Preflight` and `Units` are pure arithmetic with no on-chain bytes to
cross-check; their tests mirror `wasm-client`'s own `#[cfg(test)]` modules
test-for-test instead, which is the right level of rigor for functions with
no encoding to get wrong.

## Relationship to pda-spike

`pda-spike/php/Curve25519.php` and `Pda.php` were where this package's field
arithmetic and PDA derivation were first proven — see
`pda-spike/README.md` for that experiment's own record, including a finding
worth knowing before touching `Fe`/`Ed25519`: PHP's `sodium` extension
exposes no ed25519 core API on any build tested so far, so the
natural-looking shortcut (`sodium_crypto_core_ed25519_is_valid_point`)
doesn't exist, and a stricter substitute would silently derive wrong
addresses on roughly half of all inputs.

`src/Core/Fe.php` and `Ed25519.php` are promoted copies, namespaced and
pruned to what this package actually needs — the spike's own copies, and its
libsodium-comparison code, stay frozen as the record of that dated run. Fix
a bug in the arithmetic here; don't expect it to also need fixing there,
since the spike is a record, not a dependency.

## Publishing

Packaged, not published — `composer.json` is publish-ready (name, PSR-4
autoload, license, `require`/`require-dev`) but nothing has run `composer
publish`. Same reasoning as `wasm-client/README.md`'s publishing section:
rare, and worth a deliberate decision rather than a side effect of finishing
the code.

## Licence

Dual licensed under either of Apache License, Version 2.0, or the MIT
license, at your option — see the repository root `LICENSE.md` for why both.
