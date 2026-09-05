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
| *(nothing)* | `Tx` — `compile`, `wire`; see "Transaction assembly" below |

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
without a check against it. `vectors-gen/` is that check: a small Rust
binary, unpublished, that emits five things sourced from the real crate and
program rather than transcribed by hand —

- PDA derivation and one `meter_and_settle` instruction, from the published
  `sol-pay-client` crate on crates.io.
- A genuine Anchor-serialized `Site` and `Contract` account, built from
  `pay-on-chain::state::{Site,Contract}`'s own `#[account]`-derived
  `DISCRIMINATOR` and `AnchorSerialize`.
- The `PayError` code table, computed as `PayError::<variant> as u32 +
  anchor_lang::error::ERROR_CODE_OFFSET` against `pay-on-chain`'s own enum.
- The `TokenError` code table, read from `spl_token::error::TokenError`.
- Three compiled legacy transaction messages and their wire bytes, from
  `solana-message` and `solana-transaction`, checked byte-for-byte against
  `Tx::compile` and `Tx::wire` — see "The order this has to happen in" below
  for what each case covers and why they were generated first.

`tests/Core/PdaTest.php`, `IxTest.php`, `StateTest.php`, and `ErrorTest.php`
assert against those values as hardcoded literals, so a mismatch surfaces as
a specific failing assertion pointing at the stale one, not a missing file.
Regenerate and re-run after touching `state.rs`, `errors.rs`, `pda.rs`, or
`ix.rs` on the Rust side:

```
../bin/test-php    # regenerate the vectors, then check src/Core against them
composer test      # the five PHPUnit suites above, against their literals
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

**For `Preflight` it is not, and closing that took a second fixture with
different provenance.** `wasm-client/SPEC.md` §8 says why it matters: in
`pay-on-chain/tests` every Rust preflight predicate is checked against actual
LiteSVM behaviour, because a predicate that disagrees with the program is
worse than having no predicate at all. These copies had no such check.
Mirroring the Rust tests test-for-test would not have supplied one — that
checks this port against the port's reading of the program, both readings
from the same place. Nor could `vectors-gen`: preflight produces no
chain-serialized bytes, which is why it emits none for it, and a verdict
computed from the Rust *client* predicates would only prove PHP agrees with
Rust, not with the program.

What supplies it is a recording. `pay-on-chain/tests/src/test_preflight_fixture.rs`
drives the same live SVM the other tests drive and writes
`conformance/preflight-fixture.json`: the three account records as raw bytes
at each interesting instant, the predicate's verdict there, and what the
program then actually did. `conformance/preflight.php` replays it. The
boundary it crosses is one both sides already agree on — `Site::decode`,
`Contract::decode` and `TokenAccount::decode` are themselves checked
byte-for-byte on every conformance run — so there is no new schema to keep in
step.

Three properties of that arrangement are worth stating, because each one was
a choice:

- **The recording is gated by the assertions around it.** Every case requires
  the program to agree before it is kept, and the file is written once at the
  end of a passing run. A program regression therefore fails `bin/test-rust`
  and leaves the committed fixture untouched, rather than quietly rewriting
  this port's expectations to match the regression.
- **It is committed, and its provenance is not `vectors.json`'s.** The
  vectors come from the *published* crate and are regenerated every run — an
  outside opinion. This comes from the *local* program, because the question
  is whether this port agrees with the program you are about to deploy. It
  moves only when `bin/test-rust` rewrites it, which makes a moved verdict a
  reviewable diff. The two files answer different questions and should not be
  merged.
- **Coverage is what the recorded cases touch, and no more.** Pinned:
  `charge`, `canMeter` (including `over`), `willSettle`, `viewsRemaining`,
  `limitFloor`, and `Shortfall::diagnose`'s three fields. Not pinned:
  `requiredAllowance`, which is the identity function, and `Blocked::Overflow`,
  which no realistic page count reaches — and which would not mean the same
  thing on both sides anyway, since this package overflows at `PHP_INT_MAX`
  and the program at `u64`. Widening it means adding a case to the Rust
  recorder: one place, both ports.

The demonstrator is still the first thing running these predicates against a
real devnet, and that remains the sharper test — a recording pins agreement
at the states the harness reaches, not at every state a live site produces.

## Transaction assembly (`SolPay\Tx`)

**Decided and built 2026-09-04.** The reasoning against the obvious shortcut
is kept below because it is the part that would otherwise be lost. What was
decided is that this package compiles the message itself; see "Decided: this,
not the sidecar" for what that was weighed against.

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

1. **Vectors before the encoder — done.** `vectors-gen` emits the expected
   message and wire bytes for three fixed `(instructions, fee payer,
   blockhash)` cases, produced by `solana-message` and `solana-transaction`
   rather than transcribed by hand. Writing the compiler first would have
   meant hand-verifying wire bytes, which is the same trap the PDA shortcut
   was — plausible output, no error. No new surface in the shipped Rust
   client: the generator reaches for Solana's crates directly.

   The blockhash is `sha256("blockhash-0")` throughout, following the file's
   own seeding convention. Signatures are 64 fixed bytes each, seeded per
   case and position, and are not signatures over anything; they are there so
   the wire framing around the message is covered for more than one signer.
   Nothing in sol-pay signs, in any language, and this must not become the
   place that starts.

   Each vector records the message whole *and* in pieces — header counts,
   ordered account keys, compiled instruction indexes — beside the
   `source_instructions` that went into it. A byte-for-byte mismatch on the
   whole says only "differs"; the pieces say whether the ordering, the header
   or the shortvec framing is what went wrong, which is the same reason the
   `meter_and_settle` vector records flags and not just its data.

   **The three cases, and what each one is for.** One instruction signed and
   paid for by one key leaves most of compilation unexercised, so the set is
   chosen to reach every branch:

   - `authority-pays` — the shipping shape: the site authority signs
     `meter_and_settle` and pays. One signature. Nine keys, header `1/0/5`.
   - `separate-fee-payer` — a relayer pays for someone else's instruction.
     Two signatures, and the authority stays readonly, which is the only way
     to put anything in the **readonly-signer partition**. The first case
     leaves that partition empty, so without this one an encoder could omit
     it entirely and still pass.
   - `two-instructions` — `initialize_site` and `meter_and_settle` together,
     paid by a third key. `site` is writable in the first and readonly in the
     second, `treasury` the reverse, so this pins the **flag merge in both
     directions**; both instructions call the same program, so its id must
     appear **once**; and two instructions make the instruction array's
     compact-u16 count something other than 1 for the first time.

   Two rules are worth knowing before writing `compile`, because both are
   invisible until a vector disagrees with you:

   - **Within each partition the keys ascend by raw pubkey bytes, not by the
     order the instructions named them.** In `authority-pays` the instruction
     positions come out `3,5,4` and `7,2,6,–,0` while the bytes ascend
     strictly. An encoder that preserves instruction order inside a partition
     builds a different message that still looks right.
   - **The fee payer is pulled out and put first rather than sorted into
     place**, and it is forced to a writable signer even when the instruction
     marked it readonly — it pays the fee. `two-instructions` is what
     discriminates this: it has a second writable signer, and
     `sha256("fee-payer-0")` sorts *after* `sha256("authority-0")`, so an
     encoder that sorts all the writable signers together passes the other
     two cases and fails that one.

   While there was no encoder, `conformance/vectors.php` checked what it
   could: that each vector was present, internally consistent, and in
   agreement with the instructions a different crate compiled. Those checks
   are gone now, replaced by the byte-for-byte comparison they were standing
   in for — the promise made when they were written, kept rather than left
   to accumulate.

2. **Then `SolPay\Tx` — done.** `compile` and `wire` in `src/Core/Tx.php`,
   with `tests/Core/TxTest.php` asserting hardcoded literals for the same
   reason the other four suites do: the conformance run notices the crate
   moving, the literals name which local edit broke something.

   `conformance/vectors.php` now compares `Tx`'s output to all three vectors
   byte-for-byte. The shape checks that stood in while there was no encoder
   are gone, replaced by one diagnostic — a mismatch reports the byte and the
   section it falls in ("byte 4, in account key 0"), because a hex diff of
   348 bytes says only "differs".
3. **Prove it against devnet in the demonstrator — still open.** The
   conformance run proves `Tx` agrees byte-for-byte with `solana-message` and
   `solana-transaction` on three fixed cases. It does not prove a validator
   accepts what comes out, and those are different claims: no signature here
   is real, no blockhash here was ever current, and nothing has paid a fee.
   With `meter_and_settle` signed by the site authority, compilation sits on
   the metering path rather than only in first-run setup, so the demonstrator
   exercises it on every settling request — the best evidence available, and
   the first time this code meets a chain.
4. **Then amend SPEC §7's sentence**, which §7 already carries as pending.
   Held until step 3, deliberately: §7 describes what the library does, and
   "compiles the message that carries them" is a claim better made after a
   validator has accepted one.

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
autoload, license, `require`/`require-dev`) but nothing has been submitted to
Packagist. Same reasoning as `wasm-client/README.md`'s publishing section:
rare, and worth a deliberate decision rather than a side effect of finishing
the code.

**Packagist versioning needs more than a tag.** Unlike `cargo publish`/`npm
publish`, which package whatever the manifest says at the moment you run them,
Packagist derives a package's version *from a git tag* — and a tag in this
repository is repo-wide, so it would apply to `pay-on-chain` and `wasm-client`
too, meaning nothing to either of them. Prefixing the tag to scope it
(`php-client-v0.1.0`) looks like the obvious fix and was tested directly
against Composer's own VCS driver (the same code Packagist runs): it does not
work. Composer's tag parser requires the tag to *be* the version string, with
only an optional leading `v` — `v0.1.0` alone resolves to `0.1.0`;
`php-client-v0.1.0` and `php-client/v0.1.0` are both invisible to it, not even
parsed incorrectly, just never listed as a version at all. A second,
independent problem showed up in the same test: Composer's VCS repository type
requires `composer.json` at the repository root, and fails outright (`No valid
composer.json was found in any branch or tag`) when it lives in a subdirectory
the way this one does.

Both problems have the same fix, and it is the standard one in the PHP
ecosystem (Symfony's components, Laravel's `illuminate/*`): a **subtree
split** — mirror `php-client/` into its own repository and point Packagist at
that instead of this one. The split repo gets `composer.json` at its root for
free, and a tag history that cannot collide with the other two artifacts'
versioning.

### Publishing a version

`bin/split-php-client` does the local half and stops. It refuses if
`php-client/` has uncommitted changes, runs `bin/test-php` and `composer
test`, synthesizes the split, checks the result is actually publishable, and
prints the remaining commands. It does not push, tag, or publish: those leave
the machine, need personal credentials, and are hard to take back — the same
reasoning that leaves `cargo publish` unscripted.

**One-time setup.** Create an empty public repository for the split — it holds
only this package. Add it as a remote here (`git remote add split <url>`).
Then submit *that* repository's URL at
<https://packagist.org/packages/submit>, and enable the GitHub hook Packagist
offers so later tags are picked up without resubmitting. Nothing is ever
committed in the split repo by hand; see below.

**Per release, from the root of this repository:**

```
bin/split-php-client                       # verifies, then updates php-client-release
git push split php-client-release:master
```

**Then, in a clone of the split repository — not here:**

```
git tag v0.1.0
git push origin v0.1.0
```

The tag belongs there and only there. A tag in this repository would claim to
version `pay-on-chain` and `wasm-client` as well, which is the whole problem
the split exists to avoid. `composer.json` carries no `version` field on
purpose — Composer takes it from the tag, and a hardcoded one conflicts with
it — and `bin/split-php-client` fails if one appears.

**Why this is not a one-time task.** The split repository is a projection of a
moving source, so every release re-runs the split to pick up what changed.
That is cheap: `git subtree split` is deterministic, so the same commits
synthesize to the same SHAs, the branch *extends* rather than being rewritten,
and the push is an ordinary fast-forward. The script checks that property
explicitly instead of trusting it, and refuses rather than suggesting a
force-push if it ever fails.

The one way to break it: **never commit in the split repository.** It is
write-only from here. A commit made there diverges from the synthesized
history, and the next split stops fast-forwarding.

### Why the licences are copied, not linked

`LICENSE-MIT` and `LICENSE-APACHE` here are **intentional duplicates** of the
ones at the repository root, for the same reason `wasm-client/` carries its
own: the licence has to be *inside* the published artifact, and this directory
becomes the root of one.

Copies rather than symlinks, and that is not a style preference. Git stores a
symlink as a blob whose content is the target string, and `git subtree split`
moves that blob verbatim — so `php-client/LICENSE-MIT -> ../LICENSE-MIT`
arrives at the split root still pointing at `../LICENSE-MIT`, now outside the
repository entirely. Measured: it dangles in a fresh clone, and it survives
`git archive` *as a symlink* (the zip entry carries the symlink bit and 14
bytes of path, not the licence text), so it dangles on Composer's dist path
too. On a checkout without symlink support it is worse — a plain file whose
entire content is `../LICENSE-MIT`, which looks real and is not.
`bin/split-php-client` fails on a symlink there rather than shipping one.

### What the package ships

Decided 2026-09-05. The split takes everything under `php-client/`, which
includes `pda-spike/` — a dated experiment kept as a record — and
`vectors-gen/`, a Rust crate that cannot run for a Composer consumer at all.
Neither breaks anything, and both are small in git, but a `composer require`
should not hand somebody a spike and a Rust generator alongside the library.

The suites go too, which is the conventional exclusion in this ecosystem —
`tests/` and `phpunit.xml`, and `conformance/` with them. That last one is not
a judgement call: `conformance/vectors.php` reads `vectors-gen/vectors.json`,
which is gitignored and has never been in a package, so the script could not
run from a dist zip before this either.

Two remedies were available and they are not the same remedy. Excluding all of
it **from the split** would take it out of the published repository too, and
it should be there: the split repo is the package's public source, and how the
field arithmetic was settled, how the vectors are made, and how anyone checks
the claim are all part of that source. Excluding it **from `git archive`**
takes it out of the dist zip only. That is `php-client/.gitattributes`:

```
/pda-spike      export-ignore
/vectors-gen    export-ignore
/tests          export-ignore
/conformance    export-ignore
/phpunit.xml    export-ignore
/.gitattributes export-ignore
```

`git archive` is what GitHub serves and what Packagist hands Composer as the
dist zip, which is how essentially every `composer require` of this package
will arrive. Cloning the split repository, or `composer install
--prefer-source`, still gets everything. That is the line worth drawing: the
record stays with the source, and the install gets the library.

`composer.lock` deliberately stays. It is committed here for a reproducible
dev install, Composer ignores a dependency's lock file outright, and excluding
it would buy nothing. `composer.json`'s `autoload-dev` still names the now
absent `tests/`, which is correct and not an oversight — dev autoload rules of
a dependency are never used, and `composer validate --strict` and
`composer dump-autoload --no-dev` were both run against the dist shape to
confirm it. What ships is the licences, the README, `composer.json`,
`composer.lock` and `src/Core`.

Two mechanical notes, both measured rather than assumed. `export-ignore` on a
*directory* excludes the whole subtree, even though `git check-attr` on a file
inside it reports the attribute unspecified — check-attr answers per path and
does not inherit, `git archive` prunes at the directory entry. And this file
is itself inside the prefix, so it splits with everything else and lands at
the split repository's root, where the leading slashes resolve; each path
anchors to the directory holding the file, which is `php-client` here and the
package root there — the same two directories either way.

`bin/split-php-client` extracts the split with `git archive`, so what it
verifies *is* the dist zip. Its check is a **whitelist** of the top level, not
a list of things to keep out, and that is deliberate: the exclusion is a few
lines in one file that nothing else in the repository would notice if they
were deleted, and the failure worth guarding against is the *next* thing added
under `php-client/` with no line written for it — which a blocklist cannot
see. Adding to the package is then a deliberate edit to the script, the same
reasoning that leaves publishing unscripted. It still prints the top level on
every run.

## Licence

Dual licensed under either of Apache License, Version 2.0, or the MIT
license, at your option — see the repository root `LICENSE.md` for why both.
