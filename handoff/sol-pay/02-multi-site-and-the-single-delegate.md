# "Can a reader only be metered by one site at a time?"

Surfaced 2026-09-02 while specifying the demonstrator. It is the first
objection a serious evaluator raises, the answer is better than the docs
suggest, and the docs do not currently contain it.

## The objection

`wasm-client/README.md` states, under "Four things to know":

> **A token account has exactly one delegate.** A payer therefore holds one
> active contract per token account.

Read by someone evaluating adoption, that becomes: *suppose this were widely
adopted and most adopters chose USDC. Would a reader be limited to visiting one
metered site at a time?*

The premise is correct and general. One `delegate` and one `delegated_amount`
per SPL token account is the account layout, `approve` replaces rather than
adds, and it is identical under Token-2022. It is not an artifact of any
particular mint. A second site's `approve` silently repoints the delegate, and
the first site's next settle then fails inside the token program — not at
`open_contract`, where the check is, but at the CPI, whenever the collection
threshold is next crossed.

## The answer, which is in the program and not in the prose

`payer_token_account` is constrained in **both** `OpenContract` and
`MeterAndSettle` by exactly two things:

```rust
constraint = payer_token_account.owner == payer.key(),
constraint = payer_token_account.mint  == site.mint,
```

There is no associated-token-account constraint. Any token account the payer
owns for the site's mint is acceptable, and a wallet may own arbitrarily many
token accounts for one mint — the ATA is merely the canonical one.

**So: one token account per site gives one delegate per site, and a reader can
hold as many concurrent contracts as they have token accounts.** The answer to
the objection is no, and the design already permits it.

Worth noting alongside: `Contract` does not record which token account was used,
so a contract is not bound to one. Any token account of the payer's, for the
site's mint, that delegates to the contract PDA will settle.

## What it costs, stated so nobody discovers it later

- **Rent.** Each token account is 165 bytes, roughly 0.002 SOL, recoverable on
  close.
- **Split balances.** Balance is per account, not per wallet, so a reader has to
  decide in advance how much to park with each site. That is a second budgeting
  decision on top of the limit, and a worse one, because it is invisible in
  most wallet interfaces.
- **Wallet support.** Wallets surface the ATA. Auxiliary token accounts for the
  same mint are second-class nearly everywhere, and creating and funding one is
  not a viable onboarding step for an ordinary reader today.
- **A second integrator obligation.** `SPEC.md` §4.1 says the payment core needs
  exactly one input, the payer's wallet address. **That holds only while
  everyone uses the ATA.** A site supporting auxiliary accounts must store
  *which* token account this reader uses, because `meter_and_settle` takes it as
  an account and the contract does not record it. §4.1's single sentence is
  currently the strongest claim in the document and it has an unstated
  precondition.

So the honest summary is: **the constraint is per token account, not per wallet;
the workaround exists on chain today and is not yet practical in a wallet.**
Both halves need saying. The first half alone is over-selling; the second half
alone is the objection.

## Two designs that would remove the friction rather than route around it

Neither is proposed here — they are the shape of the alternatives, so that the
current design is visibly a choice.

**A per-payer delegate PDA.** Seed the delegate `[b"delegate", payer]` rather
than making it the per-site contract PDA. One approval on the reader's ATA then
covers every site on the deployment, each site's exposure still bounded on
chain by its own `Contract.limit`. The trade: the allowance becomes a shared
pool, so a site that draws hard leaves less for the others, and the reader's
total exposure is the allowance rather than the sum of the limits they agreed
to. The delegate check at open can only compare the allowance against the
limit in front of it, so it would understate what is really required.

**Per-site escrow**, which the README already names: the reader tops up an
account the site draws from. It removes the delegate question entirely and
gives up the property the whole design rests on — that the money stays in the
reader's wallet until it is spent.

## Suggested change

Not a code change. Three documentation ones:

1. `README.md`, "A token account has exactly one delegate" — add that the
   constraint is per token account and that the program does not require the
   ATA, so concurrent contracts are possible at the cost of an account per site.
   As written, the paragraph reads as a design limit rather than a default.
2. `SPEC.md` §4.1 — qualify "the payment core needs exactly one input" with the
   ATA precondition, or state that a site supporting auxiliary accounts stores
   the token account alongside the address.
3. Somewhere findable — the multi-site question and its answer, because it will
   be asked by everyone who evaluates this and the answer currently requires
   reading the account constraints in `lib.rs`.
