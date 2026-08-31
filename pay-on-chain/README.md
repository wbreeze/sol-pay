Anchor environment for Solana pay as you go
on-chain programs.

This was set-up following instructions for
[Anchor docs quick start][aqs]
with the Rust test template.

`anchor init --test-template rust pay-on-chain`

[aqs]: https://www.anchor-lang.com/docs/quickstart/local

## Dev

- After checkout, in this directory, run `anchor build`
- In a separate terminal, from this directory, run `solana-test-validator`
- Run `anchor test --skip-local-validator`

Alteratively, you can allow `anchor test` to start and stop the validator:
```
anchor build
anchor test
```

## Program id

`declare_id!` in `programs/pay-on-chain/src/lib.rs` names the address this
program expects to be deployed at, and `Anchor.toml` repeats it. It is not
decoration: Anchor's entrypoint rejects an instruction whose program id does
not match it, every PDA in the system is derived from it, and the WASM client
hardcodes the same value (a test in `tests/` fails if the two drift).

The value is the public key of `target/deploy/pay_on_chain-keypair.json`,
which `anchor build` generates on its first run. `target` is gitignored, so
**a fresh clone builds a different keypair while `lib.rs` still declares the
original address.** Before deploying from a fresh clone -- to a local
validator or anywhere else -- run:

```
anchor keys sync
```

which rewrites `declare_id!` and `Anchor.toml` to match the keypair you
actually hold. The test suite does not need this: the LiteSVM harness loads
the built `.so` at the declared id rather than at the keypair's address, so
`bin/test-rust` passes either way. Only a real deploy notices.

Keep a copy of `pay_on_chain-keypair.json` somewhere outside the repository.
It is the only way to deploy to that address, and it stays out of git because
anyone holding it could claim the address on a cluster where the program has
not been deployed yet. Upgrades do not need it -- those are signed by the
upgrade authority.

