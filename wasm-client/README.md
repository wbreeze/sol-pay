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

## Version pins

The dependency versions in `Cargo.toml` are **unverified** — crates.io was
unreachable from the machine this was written on. Run `cargo update` and check
the majors of `solana-pubkey` and `solana-instruction` before trusting them.
