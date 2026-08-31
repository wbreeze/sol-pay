# sol-pay-client

Instruction builders for the `pay-on-chain` metering program. The crate is
split so the useful part is not tied to a browser:

- `src/core/` — addresses, slugs, instruction construction. Plain Rust, no
  wasm, no I/O. A Leptos/Yew front end, a native tool, or a test can use it.
- `src/lib.rs` — a thin `wasm-bindgen` layer that converts to and from
  JavaScript. Enabled by the default `wasm` feature; turn it off
  (`--no-default-features`) to use the core from native Rust.

## Building

```
rustup target add wasm32-unknown-unknown
cargo test --no-default-features        # core tests, native
wasm-pack build --target web            # browser bundle in ./pkg
```

## Shape of the output

Instructions come out matching `@solana/kit`'s `IInstruction`, so they drop
straight into a transaction message:

```js
import init, { openContract, approveChecked, slugFromBytes } from './pkg/sol_pay_client.js';
await init();

const bytes = crypto.getRandomValues(new Uint8Array(16));
const slug = slugFromBytes(bytes);

// approve must come first: it is what makes the payer chargeable later.
const ixs = [
  approveChecked(tokenProgram, payerAta, mint, payer, site, limit, 6),
  openContract(site, payer, payerAta, slug, limit),
];
```

Signing is deliberately not here. Wallet Standard is browser JavaScript, so
the wallet adapter assembles and signs; this crate decides *what* gets signed.

Randomness is also not here. Slug bytes come from the caller
(`crypto.getRandomValues` in a browser), which keeps the crate free of a
`getrandom` backend for `wasm32-unknown-unknown`.

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
