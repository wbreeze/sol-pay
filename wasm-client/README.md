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
cargo test                                        # core tests, native
wasm-pack build --target web -- --features wasm   # browser bundle in ./pkg
```

Or `bin/test-rust` and `bin/build-rust --client` from the repository root,
which add `--locked` and, for the program, everything the LiteSVM harness
needs.

## Shape of the output

Instructions come out matching `@solana/kit`'s `IInstruction`, so they drop
straight into a transaction message:

```js
import init, { PayOnChain } from './pkg/sol_pay_client.js';
await init();

const pay = new PayOnChain();

// approve must come first: it is what makes the payer chargeable later.
const ixs = [
  pay.approveChecked(payerAta, mint, payer, site, limit, 6),
  pay.openContract(site, payer, payerAta, limit),
];

// or, the same pair in the right order:
const same = pay.approveAndOpen(payerAta, mint, payer, site, limit, 6);
```

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
unit conversion, preflight and `diagnose` stay free exports, because they are
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

The payer's wallet address is the only thing the library needs to identify a
contract. Where that address came from -- a login, an SSO session, a wallet
sign-in -- is the site's business, and this crate has no opinion about it. See
`SPEC.md` §4.

Sign In With Solana in particular: this crate neither verifies a sign-in
message nor builds one. Both halves are the same byte-exact format, and
whichever library you verify with builds it too -- a second definition here
would only disagree with yours eventually. `SPEC.md` §6.6 names what to use
and the three things that are easy to get wrong.

Signing is deliberately not here. Wallet Standard is browser JavaScript, so
the wallet adapter assembles and signs; this crate decides *what* gets signed.

Nor is randomness, or any other source of ambient state: every function in the
crate is a pure function of its arguments.

## Four things to know

**The approve must precede the program instruction.** `open_contract` and
`renew_contract` both verify on chain that the payer's token account names the
contract PDA as delegate for the full limit. They fail rather than trust the
client to have done it.

**A token account has exactly one delegate.** A payer therefore holds one
active contract per token account. The site is part of the contract PDA seeds,
so several sites are possible in principle, but a payer wanting contracts with
two sites at once needs a separate token account for each. If multi-site
turns out to matter, the alternative is a per-site escrow the payer tops up,
which trades that limit for funds leaving the wallet early.

**The limit is trust, not pacing.** `meter_and_settle` takes a `page_views`
count and is signed by the site authority alone. The payer is not present and
does not approve each charge. Nothing bounds that count except
`used + charge <= limit`, so a site can draw straight to the limit in a single
instruction whenever it likes.

The limit is therefore the payer's exposure to the site, not a budget that
paces their reading. A site explaining the limit to a payer should say so
plainly, and should expect the honest number to be small.

**Transaction logs are yours to filter.** This crate does not read or parse
transaction logs, and diagnosing a failed metering call means looking at them:
the numeric error code alone does not say which program raised it, so
`LimitReached` from this program and `InsufficientFunds` from the SPL Token
program are told apart by their context in the logs, not by their numbers.

Handling that is the integrator's job, and it comes with an exposure worth
naming. A transaction's logs and its account list carry the payer's wallet
address, and the program's `emit!` events carry amounts -- `used`, `paid`,
`transferred` -- as base64 `Program data:` lines that anyone can decode. None
of it is secret; it is all on chain already. But raking raw logs into
application logs, an error tracker, or an analytics pipeline copies payer
wallet addresses and spending history into systems that were never scoped to
hold them, and it does so on a site that may well have adopted this design to
avoid exactly that kind of baggage.

Extract the error code, discard the rest, and think before forwarding raw
transaction logs to a third-party service.

## Publishing

Two artifacts, one source tree, one version number: bump `version` in
`Cargo.toml` and both follow.

```
cargo publish --dry-run                          # crates.io: the core
wasm-pack build --target web -- --features wasm  # regenerate pkg/ first
wasm-pack pack                                   # npm: inspect the tarball
```

Then the real thing, `cargo publish` and `wasm-pack publish`, in that order —
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
