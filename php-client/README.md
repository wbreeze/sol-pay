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

Requires PHP ^8.1. Not wired into `bin/build-rust` or `bin/test-rust` — this
package has no Rust toolchain dependency and no reason to share theirs; see
the repository root `CLAUDE.md` for why that isolation is deliberate rather
than an oversight. It has its own entry points instead: `bin/test-php` and
the `php conformance` workflow, both under "Drift control" below.

That `^8.1` is the library's floor, not the tooling's. PHPUnit 11 pulls a
`sebastian/*` tree requiring php >=8.2, so a plain `composer install` cannot
resolve on 8.1 at all. `composer install --no-dev` can, because this package
has **no runtime dependencies** — that is how CI tests the floor, and how a
consumer on 8.1 would install it, since consumers do not take dev
dependencies either.

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
../bin/test-php    # regenerate the vectors, then check src/Core against them
composer test      # the four PHPUnit suites above, against their literals
```

Those are two different questions and neither replaces the other. Frozen
literals name a local regression precisely and cannot notice the crate
moving; freshly generated vectors notice the crate moving and would not tell
you which of your own edits broke something.

`conformance/vectors.php` is what `bin/test-php` runs, and it checks **this
package** rather than the spike: both PDA families with their bumps, the
`meter_and_settle` data and every account's pubkey and flags, `Site::decode`,
`Contract::decode`, and both error tables. The `php conformance` workflow
runs it on every push that touches this directory or the program, on PHP 8.1
and 8.5. `pda-spike/php/verify.php` still exists and still works, but it
checks the spike's standalone `Pda`/`Base58` — not what ships.

`Preflight` and `Units` are pure arithmetic with no on-chain bytes to
cross-check; their tests mirror `wasm-client`'s own `#[cfg(test)]` modules
test-for-test instead. For `Units` that is the right level of rigor — there
is no encoding there to get wrong.

**For `Preflight` it is not, and this is the one part of the package kept in
step by hand.** `Preflight`'s class doc says so, and `wasm-client/SPEC.md` §8
says why it matters: in `pay-on-chain/tests` every Rust preflight predicate
is checked against actual LiteSVM behaviour, because a predicate that
disagrees with the program is worse than having no predicate at all. These
copies have no such check. Mirroring the Rust tests test-for-test does not
supply one — it checks this port against the port's reading of the program,
and both readings come from the same place. Vectors cannot supply one
either: preflight produces no chain-serialized bytes, which is why
`vectors-gen` emits none for it.

The demonstrator will be the first thing running these predicates against a
real devnet, and a disagreement will surface there as a transaction that
fails after preflight said it would not — the failure the LiteSVM check on
the Rust side exists to prevent, and which SPEC §8 calls the point of that
exercise. **This is open work.** Its shape is not
settled and the cheap option is not obviously the right one: a PHP-side
harness driving the same LiteSVM cases the Rust tests drive would be a
fixture format crossing a language boundary, which is a larger thing than it
sounds, while having `vectors-gen` emit the program's *verdict* for a set of
preflight inputs — blocked or not, and which cause — would at least check
the predicates against something the program produced rather than against a
second reading of it.

## Planned: transaction assembly (`SolPay\Tx`)

**Decided 2026-09-04, not built.** Recorded here rather than left in a
scratch note because the gap is real, and because the reasoning against the
obvious shortcut is the part that would be lost. What was decided is that
this package compiles the message itself; see "Decided: this, not the
sidecar" below for what that was weighed against.

A PHP site authority holding an `Instruction` from `SolPay\Core\Ix` cannot
send it. Between the instruction and the wire there is a legacy transaction
message to compile: compact-u16 (shortvec) length prefixes, account-key
deduplication and ordering by signer/writable rank, the three header counts,
the program-id index, the recent blockhash, and the signature array. This
package stops at the instruction, and nothing in sol-pay goes further in any
language — `wasm-client/src/core/tx.rs` pairs payer-signed instructions in
the order the program requires, which is ordering constraints and not wire
format. SPEC §7 records the same finding from the specification's side: "the
integrator owns the connection" cost nothing for Rust or Node, and the
sentence did not change, the population it applies to did.

### Why not an existing PHP Solana SDK

There is prior art. Checking it is what settles the question rather than
raising it — checked 2026-09-04:

- **`tightenco/solana-php-sdk`**, the original — last release v0.3.2, March
  2022; repository archived; Packagist lists it **abandoned**. 14,895
  installs. Requires `php ^7.4 || ~8.0`, which does not even permit this
  package's 8.1 floor. Pulls `guzzlehttp/guzzle`, `illuminate/http` and
  `illuminate/support`.
- **`attestto/solana-php-sdk`**, the living fork of `verze-app`, itself of
  `tightenco` — 42,464 installs, `php ~8.2`, Guzzle plus a sodium compat shim
  plus dotenv. **Untagged.** Packagist lists five versions for it and every
  one is a branch: `dev-master` (2024-11-08),
  `dev-chore/dependabot-grouping` (2026-08-06), and three feature branches
  from 2024. Packagist imports tags automatically, so five branches and no
  tag is the absence itself and not a summary of it — there has never been a
  release, and requiring this package means requiring a moving branch. That
  2026 branch does mean the repository is not dormant, only untagged; "last
  commit 2024-11-08" is true of `master` and not of the repository.
- Behind those, a fork tail: `cryptothree`, `lyenon`, `iroge`, `jools`,
  `safebits`, `pantaovay`, `josephopanel`, `efrost-deltaplan`. Nine or more
  forks of one abandoned package is what an unmaintained dependency looks
  like from the outside.

Three consequences, in order of how much they matter:

1. **None of it is checked against anything.** These are hand-written ports
   of a wire format with no conformance vectors and no upstream to notice
   drift — which is the exact exposure "Drift control" above exists to
   answer, taken on as a dependency instead. An unverified message
   serializer fails the way the libsodium PDA shortcut would have: it builds
   a plausible transaction that does the wrong thing, and then someone signs
   it.
2. **The dependency cost is disproportionate.** This package has zero runtime
   dependencies and "Building and testing" above makes that load-bearing —
   `composer install --no-dev` on 8.1 is how a consumer on the floor installs
   it. Reaching a message serializer through Guzzle and `illuminate/*` gives
   that up for a few hundred lines of encoding.
3. **The floors do not line up.** Depending on the maintained fork would push
   this package's floor from 8.1 to 8.2, and `php-conformance.yml` tests 8.1
   on purpose.

### What it would be

Two pure functions of their arguments, which is the property `src/Core` is
built on:

```
compile(Instruction[], feePayer, recentBlockhash) -> message bytes
wire(message, signatures)                         -> transaction bytes
```

The blockhash is passed in, never fetched. The signature is produced by the
caller — `sodium_crypto_sign_detached`, the *signature* API, present in
ext-sodium since 7.2; only the ed25519 **core point** API is missing, which
is the separate finding in `pda-spike/README.md`. So every verb in SPEC §7
survives: still no signing, no signature verification, no sign-in message
construction, no RPC, no retries, no storage, no routing, no rendering, no
session management.

SPEC §2's design rule puts this on the library's side rather than the site's:
message compilation is encoding, ordering and exact byte layout; there is no
legitimate site choice anywhere in it; and wrong costs a failed transaction
and a real fee, which is §2's own criterion. It has no view on what limit to
suggest, when to meter, how the site identifies a visitor, or what to do when
a payment fails.

Sizing: shortvec, key ordering and header derivation are on the order of 150
lines with no dependencies, against the ~250 for the `Fe`/`Ed25519` field
arithmetic already accepted here.

### The order this has to happen in

1. **Vectors before the encoder.** Extend `pda-spike/vectors-gen` to emit the
   expected message and wire bytes for one fixed `(instruction, fee payer,
   blockhash)` triple, produced by `solana-message` and `solana-transaction`
   rather than transcribed by hand, and check them from
   `conformance/vectors.php`. Writing the compiler first means hand-verifying
   wire bytes, which is the same trap the PDA shortcut was — plausible
   output, no error. No new surface in the shipped Rust client: the generator
   reaches for Solana's crates directly.
2. **Then `SolPay\Tx`**, with a `tests/Core/TxTest.php` whose expected values
   are hardcoded literals, for the same reason the other four suites are —
   the conformance run notices the crate moving, the literals name which
   local edit broke something.
3. **Prove it in the demonstrator against devnet, then promote it.** That is
   the path `pda-spike/php/` took into `src/Core`, which makes this a
   precedent here rather than a new practice. With `meter_and_settle` signed
   by the site authority, compilation sits on the metering path rather than
   only in first-run setup, so it is exercised on every settling request —
   the best evidence available for whether it deserves promotion.
4. **Then amend SPEC §7's sentence**, which §7 already carries as pending.

### The scope boundary, stated so it is not a surprise later

This package would be shipping a small piece of general Solana plumbing, and
people will ask for v0 messages, address lookup tables, compute-budget
instructions and durable nonces. The honest boundary is **legacy message
only**, and it holds: a priority-fee or compute-budget instruction is just
another `Instruction` in the array, so it costs nothing to support and
nothing to refuse. v0 and lookup tables are a real decision, and for
`meter_and_settle` — eight accounts, one signer — the answer is no.

### Decided: this, not the sidecar

**Decided 2026-09-04.** SPEC §3.1's sidecar was decided the same day, and it
absorbs assembly and signing for every unserved language at once — including
this one. PHP does not take that route. A sidecar is the answer for a
language with no package; PHP has a package, and one whose zero runtime
dependencies and 8.1 floor are the whole reason it installs where it has to.
Asking a PHP shop for a deployment unit and a trust boundary in place of a
`composer require` gives that up, and the fragmentation that carries the
sidecar — Ruby, Java, Scala, ASP.NET, Python, no CMS mass to aim a port at —
was never an argument about PHP.

What that buys is bought with drift, and the bill is not disputed here.
SPEC §8.1's objection stands exactly as written: this package is a second
implementation, `SolPay\Tx` widens it from derive/encode/decode to
derive/encode/decode/compile, and the sidecar would have avoided the widening
because Rust gets message compilation from Solana's own crates for nothing.
The cost is **accepted, not answered**. That is what makes step 1 above
load-bearing rather than merely prudent: vectors first is the price of this
decision, and an encoder written before them would take on the drift without
buying the check.

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

**Packagist versioning needs more than a tag when the day comes.** Unlike
`cargo publish`/`npm publish`, which package whatever the manifest says at
the moment you run them, Packagist derives a package's version *from a git
tag* — and a tag in this repository is repo-wide, so it would apply to
`pay-on-chain` and `wasm-client` too, meaning nothing to either of them.
Prefixing the tag to scope it (`php-client-v0.1.0`) looks like the obvious
fix and was tested directly against Composer's own VCS driver (the same code
Packagist runs): it does not work. Composer's tag parser requires the tag to
*be* the version string, with only an optional leading `v` — `v0.1.0` alone
resolves to `0.1.0`; `php-client-v0.1.0` and `php-client/v0.1.0` are both
invisible to it, not even parsed incorrectly, just never listed as a version
at all. A second, independent problem showed up in the same test: Composer's
VCS repository type requires `composer.json` at the repository root, and
fails outright (`No valid composer.json was found in any branch or tag`)
when it lives in a subdirectory the way this one does.

Both problems have the same fix, and it's the standard one for this in the
PHP ecosystem (Symfony's components, Laravel's `illuminate/*` packages): a
**subtree split** — mirror the `php-client/` subtree into its own dedicated
repository (`git subtree split -P php-client`, or a CI action like
`symplify/monorepo-split-github-action`) and point Packagist at that repo
instead of this one. The split repo gets `composer.json` at its root for
free, and its own tag history that can never collide with the other two
artifacts' versioning. Worth building only once publishing is actually
decided — no point standing up that pipeline for a package nobody can
`composer require` yet.

## Licence

Dual licensed under either of Apache License, Version 2.0, or the MIT
license, at your option — see the repository root `LICENSE.md` for why both.
