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

