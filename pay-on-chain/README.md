Anchor environment for Solana pay as you go
on-chain programs.

This was set-up following instructions for
[Anchor docs quick start][aqs]
with the Rust test template.

`anchor init --test-template rust pay-on-chain`

[aqs]: https://www.anchor-lang.com/docs/quickstart/local

The JavaScript scaffolding `anchor init` also generates -- `package.json`,
`tsconfig.json`, `migrations/` -- has been removed. Nothing in this repository
is written in JavaScript, and `Anchor.toml` runs `cargo test`.

## Dev

From the repository root:

```
bin/build-rust --program   # anchor build
bin/test-rust              # the program suite and the client suite
```

**No validator is involved.** The tests run against [LiteSVM][litesvm], an
in-process SVM that loads the built `.so` directly. `Anchor.toml` sets
`[scripts] test = "cargo test"`, so `anchor test` reaches the same assertions,
but it first starts a local validator and deploys to it -- and then nothing
uses either. `bin/test-rust` is the shorter path and the one CI runs.

`solana-test-validator` is wanted for one thing only: deploying the program
locally and driving it by hand. If the clone is fresh, read "Program id"
below before doing that.

[litesvm]: https://github.com/LiteSVM/litesvm

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

### The trap without a fresh clone

`bin/clean --program` removes `target/` and takes that gitignored keypair with
it. The next `anchor build` mints a replacement without saying so, because it
generates the file whenever it is absent and never reads it otherwise -- the id
compiled into the `.so` comes from `declare_id!`, not from the keypair. What
the build leaves behind is therefore a placeholder sitting exactly where
`--program-id` reaches for out of habit. It deploys perfectly well, to the
wrong address, where Anchor's entrypoint rejects every instruction the program
exists to serve. This has happened once, on devnet, on 2026-09-05.

`bin/build-rust --program` now warns when the keypair in `target/deploy/` does
not match `declare_id!`, which is the earliest moment anything can notice.
Copying your saved key there silences it; `bin/clean` will delete that copy
too, which is exactly why the copy outside the repository is the one that
matters.

### Deploying to the declared address

From a clone that holds the saved key, pointing at it where it actually lives:

```
solana program deploy target/deploy/pay_on_chain.so \
  --program-id /path/to/your/pay_on_chain-keypair.json \
  --url devnet
```

Nothing here scripts that, for the reason `wasm-client/README.md` gives about
publishing: it is rare, irreversible, needs personal credentials, and the path
in the middle is yours and belongs in no file of ours. The wallet in
`solana config get` is only fee payer and upgrade authority -- it has no say in
the address, which comes from the `--program-id` file alone.

The program is live on devnet at the declared address as of 2026-09-05, so the
claim-the-address risk above now applies to mainnet and to any cluster it has
not reached, no longer to devnet. Upgrades there are signed by the upgrade
authority, and need no copy of the program keypair at all.


What is live there is the delegate design. The fund design in this tree goes
out as an upgrade to the same address, under the same upgrade authority. A
`Site` keeps its layout across the change; a meter opened under the delegate
design does not, and nothing migrates it -- there are none worth keeping.

### Upgrading the deployed program

An upgrade replaces the bytes at the declared address and keeps the address,
so it needs no program keypair -- only the upgrade authority, which is the
wallet in `solana config get` unless you pass `--upgrade-authority`. Build
first, and confirm the build carries the id you mean to upgrade:

```
bin/build-rust --program
solana address -k target/deploy/pay_on_chain-keypair.json   # should print F8UDAGgx...; the build warns if not
```

**Check the room before you deploy.** The program's bytes live in a program
data account whose size was fixed at the first deploy, to fit that build. An
upgrade that is larger than it does not fit. Compare the two:

```
solana program show F8UDAGgxVTm8Vmh4RmskpMBCFqhRvuTqbDxDCj8UMedL --url devnet
ls -l target/deploy/pay_on_chain.so
```

`Data Length` in the first is the room; the file size in the second is what
has to fit in it. The fund design added instructions, so expect the build to
have grown past what the delegate design was deployed with.

Current Solana CLIs extend the account for you during an upgrade (the flag
that turns it off, `--no-auto-extend`, shows in `solana program deploy
--help` when yours does). One that does not fails the upgrade with an
account-data-too-small error, and the fix is to extend by hand, by at least
the difference, and deploy again:

```
solana program extend F8UDAGgxVTm8Vmh4RmskpMBCFqhRvuTqbDxDCj8UMedL <bytes> --url devnet
```

Extending costs rent on the added bytes, paid by the wallet, and cannot be
undone short of closing the program.

Then the upgrade itself, naming the program by address rather than by
keypair file:

```
solana program deploy target/deploy/pay_on_chain.so \
  --program-id F8UDAGgxVTm8Vmh4RmskpMBCFqhRvuTqbDxDCj8UMedL \
  --url devnet
```

**A failed upgrade leaves a buffer behind.** The CLI writes the new bytes to
a buffer account first and swaps them in at the end, so an upgrade that fails
partway -- a dropped connection, too little SOL, the size check above -- leaves
a funded buffer holding most of the program's rent. `solana program show
--buffers --url devnet` lists them and `solana program close --buffers --url
devnet` returns the SOL. Running the deploy again does not reuse one.

Afterwards, `solana program show` again: `Last Deployed In Slot` should have
moved and `Data Length` should cover the build. That is all it proves -- that
bytes changed hands, not that the fund design works on devnet. Nothing in
this repository exercises it there; the demonstrator is what will.
