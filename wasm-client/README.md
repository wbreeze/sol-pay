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

## Two things to know

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
