# sol-pay-client

Instruction builders for the `pay-on-chain` metering program. The crate is
split so the useful part is not tied to a browser:

- `src/core/` — addresses, instruction construction. Plain Rust, no
  wasm, no I/O. A Leptos/Yew front end, a native tool, or a test can use it.
  `core::Program` names the deployment and the token program everything is
  built for.
- `src/lib.rs` — a thin `wasm-bindgen` layer that converts to and from
  JavaScript. Behind the opt-in `wasm` feature, so depending on this crate
  from native Rust costs nothing extra; ask for `--features wasm` to get it.

## Building

```
rustup target add wasm32-unknown-unknown
cargo test
cargo run --example open_meter
wasm-pack build --target web -- --features wasm
```

- `cargo test`: core tests, native
- `cargo run --example open_meter`: the write path, printed
- `wasm-pack build --target web -- --features wasm`: browser bundle in ./pkg

Or `bin/test-rust` and `bin/build-rust --client` from the repository root,
which add `--locked` and, for the program, everything the LiteSVM harness
needs.

`examples/open_meter.rs` is the browser's half of an integration in about
a hundred and thirty lines: derive the addresses, convert the amounts, build the three
instructions a site's server composes for a reader's wallet to sign, stop. It is also a check on this crate's public
surface. An example links the library as an external crate, so it reaches only
what an integrator can reach, and `cargo test` builds it -- so a change that
breaks a real call site fails the test run rather than waiting for someone to
integrate against a release.

There is no matching example for the server's half. This crate decodes account
bytes and never produces them, so showing the read path honestly needs real
accounts from a cluster rather than a fixture that can quietly go stale.

## Shape of the output

Instructions come out matching `@solana/kit`'s `IInstruction`, so they drop
straight into a transaction message:

```js
import init, { PayOnChain } from './pkg/sol_pay_client.js';
await init();

const pay = new PayOnChain();

// The transaction a reader's wallet signs, composed by the site's server
// for a Solana Pay transaction request. The fund must exist before money
// lands in it, so open_fund comes first; openFundAndDeposit is that pair.
const fund = pay.deriveFundAddress(reader, mint, index);
const ixs = [
  ...pay.openFundAndDeposit(reader, mint, index, readerAta, deposit, 6),
  pay.openMeter(site, reader, fund, browserKey, limit, expiry),
];
```

That match is an agreement neither package declares -- this crate depends on no
JavaScript at all, and you pick kit for yourself -- so it is checked rather than
assumed. The published `package.json` carries the kit range this release was
checked against as an **optional peer dependency**, which is a statement about
consumption and not an install: nothing is pulled, and you are free to ignore
it. If you vendor kit separately, that field is how you ask whether your copy
and this one were ever tested together.

What earns it is `conformance/kit.mjs` (`bin/test-kit`, and the `kit agreement`
workflow): kit's own `AccountRole` constants against the bit pattern this crate
encodes, and kit's legacy message compilation against `solana-message`'s --
header, account set, every signer and writable bit, and each instruction's
account list resolved back to addresses.

Compared that way and not byte-for-byte, deliberately: **intra-partition
account order is not canonical**. `solana-message` orders by raw pubkey bytes,
kit by the base58 string, and the two disagree on where the SPL Token program
id lands. Both messages are valid and mean the same thing. If you compile the
same transaction on a server and in the browser, do not expect the bytes to
match. SPEC.md §8.2 has the measurement.

### Four exports that are not ours

`sol_pay_client.d.ts` also declares `Pubkey`, `Hash`, `Instruction` and
`Instructions`. Those are not part of this library. They come from
`solana-pubkey`, `solana-instruction` and `solana-hash`, which declare
`wasm-bindgen` and `js-sys` under `cfg(target_arch = "wasm32")` — not optional,
not behind any feature — and export their own types whenever they are built
for the browser. Nothing this crate enables causes it and nothing it could
disable would stop it, short of not using those crates at all.

**Treat them as absent.** They are outside this package's compatibility
promise: an upstream release can change or remove them without any change
here, and the version number will not warn you. Everything this library
actually offers is the `PayOnChain` class and the free functions documented
above, and addresses cross its boundary as base58 strings, never as a `Pubkey`
object.

## Using the bundle from a Node server

`SPEC.md` §3 splits an integration into a browser and a server, and the server
row does not have to be Rust. The same npm package the browser takes runs on a
Node server -- nothing in the wasm layer is browser-specific, so there is no
second build target and no second package to install.

One difference, and it is the whole of it: `init()` with no argument resolves
`sol_pay_client_bg.wasm` against `import.meta.url` and fetches it, and Node's
fetch does not do `file:` URLs. Hand it the bytes instead.

```js
import { readFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import init, { PayOnChain } from 'sol-pay-client';

const require = createRequire(import.meta.url);
const wasmPath = require.resolve('sol-pay-client/sol_pay_client_bg.wasm');
await init({ module_or_path: await readFile(wasmPath) });

const pay = new PayOnChain();
const ix = pay.meterAndSettle(site, authority, fund, treasury, mint, items);
```

Do that once at startup rather than per request. `init` returns early if the
module is already there, but the file read does not.

The object form matters: passing the bytes positionally still works and warns
that it is deprecated.

A server reaches for nearly everything: `meterAndSettle` and
`initializeSite`, which the site authority signs; the setup builders, which it
composes for the reader's wallet to sign; `verifyKey`, for the key proof; and
the decoders, preflight and `cause`. The page needs only `closeMeter`, to sign
with its browser key when the reader signs out.

`bin/test-node` is the check that keeps this section honest, and the
`node conformance` workflow runs it on the LTS Node lines. It is not drift
control: Node runs the same wasm binary the browser runs, so it cannot
disagree with the crate. What it guards is the bytes-init contract above,
which belongs to `wasm-pack`'s generated glue rather than to anything here.

## Which deployment, and which token program

Two things every builder needs, neither of which changes between calls: the
metering program's address, and the SPL token program the site's mint belongs
to. Both are defaults rather than constraints, and both are stated once.

```rust
use sol_pay_client::core::{ids, Program};

let pay = Program::default();          // canonical deployment, SPL Token
let pay = Program::new(my_program_id); // my own deployment
let pay = Program::default().with_token_program(ids::TOKEN_2022_PROGRAM_ID);

let (site, _) = pay.site_address(&authority);
```

```js
const pay = new PayOnChain();
const pay = new PayOnChain(myProgramId);
const pay = new PayOnChain().withTokenProgram(token2022ProgramAddress());
```

The two vary independently, and most integrators will need neither. The `Site`
PDA is seeded by authority, so one deployment already serves many sites with
independent pricing; the override exists so that wanting your own deployment is
not a reason to be unable to use the package.

In Rust the free functions in `core::pda`, `core::ix`, `core::tx` and
`core::error` are the same calls against the canonical deployment on SPL Token.
In JavaScript the calls that depend on either live only on the class; decoding,
unit conversion, preflight and `verifyKey` stay free exports, because they are
the same whoever deployed the program.

**Get the token program wrong and every instruction for that mint fails at the
runtime.** It is not a preference: a mint account is *owned* by one token
program or the other. Since `getAccountInfo` hands you that owner beside the
mint data you are already decoding, checking costs nothing:

```js
if (!pay.ownsMint(mintAccount.owner)) { /* wrong token program */ }
```

The program id has no equivalent check. Confirming a deployment exists needs a
network, and this crate does not have one.

A meter is identified by its site and fund, and the key proof says which
browser holds it: the page signs a nonce from the server with its browser key,
and `verifyKey` checks the signature against the key the meter names. The
nonce is the site's to issue, use once and expire quickly: a proof accepted
twice lets whoever copied it read on the reader's fund. Anything
more the site knows about the reader -- a login, an SSO session -- is the
site's business, and this crate has no opinion about it. See `SPEC.md` §4 and
§6.6.

Signing is deliberately not here. The reader's wallet signs in the wallet, by
a Solana Pay scan, and the page signs with its own key through WebCrypto; this
crate decides *what* gets signed, and checks a key proof, which needs no
secret.

Nor is randomness, or any other source of ambient state: every function in the
crate is a pure function of its arguments.

## Four things to know

**`open_fund` must precede the deposit.** The fund's token account is created
by `open_fund`, and a transfer to an account that does not exist fails. The
two go in one transaction, in that order; `openFundAndDeposit` (`core::tx`)
returns the pair already ordered. `open_meter` needs the fund to exist, not
to hold money, so it can follow either way.

**The limit is trust, not pacing.** `meter_and_settle` takes an `items`
count and is signed by the site authority alone. The reader is not present and
does not approve each charge. Nothing bounds that count except
`used + charge <= limit`, so a site can draw straight to the limit in a single
instruction whenever it likes, until the expiry.

The limit is therefore the reader's exposure to the site, not a budget that
paces their reading. A site explaining the limit to a reader should say so
plainly, and should expect the honest number to be small.

**The browser key moves no money.** It signs the key proof and `close_meter`,
and the program refuses it for everything else. The page keeps it
(`SPEC.md` §4.8 recommends a non-extractable WebCrypto key in IndexedDB); the
server checks proofs with `verifyKey` against the `key` the meter names, and
submits a key-signed `closeMeter` as fee payer, since the key holds no SOL.

**Transaction logs are yours to filter.** This crate does not read or parse
transaction logs, and diagnosing a failed metering call means looking at them:
the numeric error code alone does not say which program raised it, so
`LimitReached` from this program and `InsufficientFunds` from the SPL Token
program are told apart by their context in the logs, not by their numbers.

Handling that is the integrator's job, and it comes with an exposure worth
naming. A transaction's account list carries the fund's address, and the fund
account names the reader's wallet; the program's `emit!` events carry amounts
-- `used`, `paid`, `transferred` -- as base64 `Program data:` lines that anyone
can decode. None of it is secret; it is all on chain already. But raking raw
logs into application logs, an error tracker, or an analytics pipeline copies
reader addresses and spending history into systems that were never scoped to
hold them, and it does so on a site that may well have adopted this design to
avoid exactly that kind of baggage.

Extract the error code, discard the rest, and think before forwarding raw
transaction logs to a third-party service.

## Can a reader be metered by more than one site at once?

Yes. It is the first thing anyone evaluating this asks, and the answer lives in
the seeds rather than in the prose.

A meter's address is `[b"meter", site, fund]`. One fund therefore serves any
number of sites, with at most one meter at each, and every settle draws from
the same token account. The reader budgets once, by what they put in the fund,
and bounds each site separately, by the limit and expiry of its meter. The
fund's `meters` count says how many are open, and `close_fund` refuses while
any are.

What that costs, stated so nobody discovers it later:

- **Shared money.** The limits are per site, the balance is not. A site that
  draws to its limit leaves less for the others, and a settle at one site can
  fail because another drew first. The reader's total exposure is the smaller
  of the fund's balance and the sum of the limits.
- **One mint per fund.** A fund meets only sites that price in its mint;
  `open_meter` fails with `MintMismatch` otherwise. A reader of sites in two
  coins holds two funds.
- **Rent.** A fund is a program account plus a token account, and each meter
  is a program account, all paid by the reader and returned on close.

A reader who wants separate budgets, or two devices at one site at the same
time, holds a second fund in the same mint under another index (`SPEC.md`
§4.7, §4.8). Which fund a transaction uses is always the reader's choice,
stated on the page; the server never defaults one.

## Publishing

Two artifacts, one source tree, one version number: bump `version` in
`Cargo.toml` and both follow.

**The bump moves three lock files, not one.** This crate is a path dependency
of `pay-on-chain/tests` and of `php-client/vectors-gen`, so
`pay-on-chain/Cargo.lock` and `php-client/vectors-gen/Cargo.lock` record its
version as well as `wasm-client/Cargo.lock` does, and every script here builds
with `--locked`, which refuses a lock that disagrees with a manifest. Move just
that one entry in each, from inside each directory so the pinned toolchain
answers:

```
(cd wasm-client              && cargo update -p sol-pay-client)
(cd pay-on-chain             && cargo update -p sol-pay-client)
(cd php-client/vectors-gen   && cargo update -p sol-pay-client)
```

A path dependency has nothing to resolve, so each is a one-line diff. Not
`bin/update-locks`, which moves everything its ranges allow and would bury the
bump in unrelated churn. Then run the suites; `bin/test-rust` and
`bin/test-php` both build against the new locks.

From the repository root:

```
(cd wasm-client && cargo publish --dry-run)
bin/build-rust --client
grep -A2 peerDependencies wasm-client/pkg/package.json
(cd wasm-client && wasm-pack pack)
```

- `(cd wasm-client && cargo publish --dry-run)`: crates.io: the core
- `bin/build-rust --client`: regenerate pkg/ -- not a bare wasm-pack build
- `(cd wasm-client && wasm-pack pack)`: npm: inspect the tarball

**Build `pkg/` with `bin/build-rust --client`, never with `wasm-pack build`
by hand.** The script does two things a bare build does not: it passes
`--locked`, and it writes the optional `@solana/kit` peer range into
`pkg/package.json`, copied from `conformance/package.json`. A tarball built
without it publishes with no peer declaration at all, and nothing fails --
the kit agreement job tests the range, not the published manifest. The
`grep` above is the check.

Then the real thing, `cargo publish` and `wasm-pack publish` (both from
`wasm-client/`; the second publishes the `pkg/` already built, and does not
rebuild it), in that order —
the crate is the one another Rust crate can depend on, so it is the one worth
having land first if only one of them does.

`wasm-pack` writes `pkg/package.json` from the `[package]` fields above, so
the npm package takes its name, version, description, license and repository
from `Cargo.toml` and there is no second place to keep them in step. `pkg/` is
gitignored build output: it is regenerated by the build, never edited.

`LICENSE-MIT` and `LICENSE-APACHE` are duplicated into this directory on
purpose. `cargo package` and `wasm-pack` both see only files under the crate
root, so a licence that lives only at the repository root ships in neither
artifact.

Two things to check the first time, neither of which this repository can
answer for itself:

- **The name `sol-pay-client` has to be free on both registries.** They are
  separate namespaces with separate races. `cargo search sol-pay-client` and
  `npm view sol-pay-client` before the first publish.
- **A crates.io publish is permanent.** Versions can be yanked but never
  replaced or deleted, so the dry run is not a formality.

Publishing is deliberately not scripted. It is rare, irreversible, and needs
credentials that belong to a person rather than to a repository.

## Versions

The dependency versions in `Cargo.toml` are the ones this crate was built and
tested against; `Cargo.lock` is committed, so a clone reproduces exactly that
resolution. Nothing here needs checking before use.

| crate | resolved |
| --- | --- |
| `solana-pubkey` | 2.4.0 |
| `solana-instruction` | 2.3.3 |
| `borsh` | 1.8.1 |
| `bs58` | 0.5.1 |
| `wasm-bindgen` | 0.2.127 |
| `serde-wasm-bindgen` | 0.6.5 |
| `serde_bytes` | 0.11.19 |
| `ed25519-dalek` | 2.2.0 |

`bin/build-rust` and `bin/test-rust` pass `--locked`, so a build that would
have to re-resolve fails instead of quietly drifting. The toolchain is pinned
in `rust-toolchain.toml`.

The one constraint worth knowing is that the solana crates must stay on the
same generation as `anchor-lang` 0.32.1, `litesvm` and `spl-token` — all 2.x.
That is not a rule anyone has to remember: two generations in one tree means
two copies of `solana-pubkey`, which fails to compile. `cargo tree -d` names
the duplicate if it ever happens.

To move off these versions deliberately, run `cargo update`, run
`bin/test-rust`, and commit the new lock. CI does that on a schedule, so an
upstream release that breaks the build shows up as a red run rather than as a
surprise during someone else's work.

## Licence

Dual licensed under MIT or Apache-2.0, at your option — the Rust ecosystem
default. Copies are in this directory, so they ship inside both published
artifacts; the statement of intent is at the repository root in `LICENSE.md`.
