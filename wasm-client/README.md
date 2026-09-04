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
cargo run --example open_contract                 # the write path, printed
wasm-pack build --target web -- --features wasm   # browser bundle in ./pkg
```

Or `bin/test-rust` and `bin/build-rust --client` from the repository root,
which add `--locked` and, for the program, everything the LiteSVM harness
needs.

`examples/open_contract.rs` is the browser's half of an integration in about
sixty lines: derive the addresses, convert the amount, build the two
instructions a payer signs, stop. It is also a check on this crate's public
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

**A token account has exactly one delegate.** `approve` replaces rather than
adds. That is the SPL account layout, identical under Token-2022, and not
something this program chose. A payer therefore holds one active contract per
token account, and a second site's `approve` silently repoints the delegate --
the first site's next settle then fails inside the token program, at the
transfer, rather than at `open_contract` where the delegate is checked. The
limit is per token account and not per wallet, and that difference is the whole
answer to the question below.

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

## Can a payer be metered by more than one site at once?

Yes. It is the first thing anyone evaluating this asks, and the answer lives in
the account constraints rather than in the prose, so it is worth stating here.

Nothing in this program requires the associated token account. `open_contract`,
`renew_contract` and `meter_and_settle` each constrain `payer_token_account` by
exactly two things:

```rust
constraint = payer_token_account.owner == payer.key(),
constraint = payer_token_account.mint  == site.mint,
```

Any token account the payer owns for the site's mint is acceptable, and a
wallet may own arbitrarily many token accounts for one mint -- the ATA is
merely the canonical one. So one token account per site gives one delegate per
site, and a payer can hold as many concurrent contracts as they have token
accounts. `Contract` does not record which account was used, either, so a
contract is not bound to one: any account of the payer's, for the site's mint,
that delegates to the contract PDA will settle.

What that costs, stated so nobody discovers it later:

- **Rent.** A plain SPL token account is 165 bytes, roughly 0.002 SOL,
  recoverable on close. Token-2022 accounts carrying extensions are larger.
- **Split balances.** Balance is per account, not per wallet, so the payer
  decides in advance how much to park with each site. That is a second
  budgeting decision on top of the limit, and a worse one, because most wallet
  interfaces do not show it.
- **Wallet support.** Wallets surface the ATA. Auxiliary token accounts for the
  same mint are second-class nearly everywhere, and creating and funding one is
  not a viable onboarding step for an ordinary reader today.
- **A second thing for the site to store.** The payer's wallet address is the
  only input this library needs *while everyone uses the ATA*. A site that
  supports auxiliary accounts must also store which token account each payer
  uses, because `meter_and_settle` takes it as an account and the contract does
  not record it.

So the honest summary is that the constraint is per token account rather than
per wallet, the workaround exists on chain today, and it is not yet practical
in an ordinary reader's wallet. Both halves need saying: the first alone
over-sells, the second alone is the objection.

Removing the friction instead of routing around it would mean changing the
design. The two shapes that would are worth naming, so that the current one is
visibly a choice rather than an oversight. A **per-payer delegate PDA**, seeded
`[b"delegate", payer]` instead of being the per-site contract PDA, would let a
single approval on the payer's ATA cover every site on the deployment, each
site still bounded on chain by its own `Contract.limit` -- at the cost of a
shared allowance, so a site that draws hard leaves less for the others, and the
payer's total exposure becomes the allowance rather than the sum of the limits
they agreed to. Or a **per-site escrow** the payer tops up, which removes the
delegate question entirely and gives up the property the whole design rests on:
that the money stays in the payer's wallet until it is spent.

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
