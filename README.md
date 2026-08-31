# SolPay

This project will offer a pay-as-you-go capability using the Solana
block chain ecosystem.

Note: This is a work in progress. Do try to implement. Do not use as
a reference code base or example. Do not expect it to work.


## Development

Run `pnpm install` to install dependencies end get started.

Copy `.env.example` to `.env` and edit the environment variables
appropriately.

Run `pnpm run dev` or simply `./bin/serve` to start the development server
and rebuild the application on file changes.

Run `solana-test-validator` to start a testing Solana block chain on localhost.

Find the Solana on-chain programs in the `pay-on-chain` directory.

These use the [Anchor framework][anchor] for on chain smart contracts.  They
use the [Codama generator][codama] to [generate a client in JavaScript][render]
from the Anchor IDL.

[anchor]: https://www.anchor-lang.com/docs
[render]: https://github.com/codama-idl/renderers-js
[codama]: https://github.com/codama-idl/codama

To build the on-chain programs, switch to the `pay-on-chain` directory and
issue the command, `anchor build`. See [getting started with anchor][ags]
for further instructions about working with the anchor programs.

[ags]: https://www.anchor-lang.com/docs/quickstart/local#getting-started

Deploying from a fresh clone needs one extra step, `anchor keys sync`, because
the program keypair is not in the repository. See `pay-on-chain/README.md`.

Two scripts wrap the Rust side:

- `./bin/build-rust` builds the Anchor program and the WASM client. Takes
  `--program` or `--client` to do just one.
- `./bin/test-rust` runs both test suites, and passes any argument through to
  `cargo test` as a filter.

The program tests run against [LiteSVM][litesvm], an in-process SVM, so they
need no validator -- but they do load `target/deploy/pay_on_chain.so`, so the
program must be built first.

[litesvm]: https://github.com/LiteSVM/litesvm

The WASM client lives in `wasm-client`. Its core is plain Rust with no browser
dependency, wrapped in a thin `wasm-bindgen` layer; see `wasm-client/README.md`.

Both scripts pass `--locked`, so they build the versions in the committed
`Cargo.lock` files or fail rather than re-resolving. `.github/workflows/rust.yml`
runs the same locked build and both suites on push; `dependency-drift.yml`
re-resolves from scratch weekly, so an upstream release that breaks the version
ranges shows up as a red scheduled run instead of a surprise.

## Payment model

The payment model is that someone with a wallet identifies with the wallet and
navigates content. The wallet pays for what they use-- a fee per page view. The
wallet owner sets a limit amount on total charges.  They sign a contract
allowing incremental charges up to the limit.  The site charges the wallet
when enough usage has accumulated to make worthwhile a transfer transaction.
The site asks to refresh the limit when the wallet owner has reached the
spending limit they have set.

See also, the [x402 protocol][x402] from Coinbase.

[x402]: https://docs.cdp.coinbase.com/x402/welcome

## Request flow

This state diagram shows the flow of a page request when navigating
metered content. The bold lines show the happy path.

- The diagram assumes the site can map a viewer to a wallet address, and says
  nothing about how. Accounts, login, SSO -- whatever the site already runs.
  That mapping is the integrator's one obligation; everything else starts from
  the address. See `wasm-client/SPEC.md` §4.
- Every contract is derived from the site and the payer's wallet address, so
  identifying the viewer *is* finding the contract. There is no session token
  in the protocol and nothing to look up but an account.
- Only two authorizations appear in the flow: the site's authority over its
  own contracts, which is what lets it meter, and the payer's authorization of
  the spend, which is the SPL approval the whole design rests on.

![pay-as-you-go-state-machine](state-machine.png)

In the state diagram, the happy path has bold lines. It goes like this:
- viewer navigates to metered content, and the site knows their wallet address
- server derives the contract address from the site and that wallet address,
  and reads the account
- server increments the usage by the page view amount
- if the viewer has accumulated sufficient unpaid usage, the server
  invokes a transfer from the viewer's wallet
- the server delivers the metered page

Viewers the site cannot place go to the sign-up page. It includes details
about the contract and may allow the viewer to select a limit. Sign-up creates
the contract account and takes the payer's authorization of the spend, both in
one transaction -- the authorization has to come first, and the program checks
that it did.

The dialog and the server must enforce a minimum for
the limit amount that is some multiple of the page view amount.
A multiple of one does not make much sense. Forty or fifty multiple yields a
better minimum. Also, the minimum should be greater than the threshold
amount for making a transfer transaction from the viewer's wallet.

With the account set-up and authorized, the viewer returns to the happy path.

When a viewer reaches their limit, the server shows them a screen that
provides a wrapup of the usage. It offers to renew the limit, or a new limit,
or to delete the contract after a transfer transaction for unpaid usage.

