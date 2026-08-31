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

- The diagram draws one way for the server to tell whose contract a request
  belongs to: a "bump slug", or simply "slug" -- a random token in the URI path
  that resolves to the contract with one derive and one account read.
- It is only one way. Payment itself never uses the slug: `meter_and_settle`
  derives the contract from the site and the payer's wallet address. A site
  with its own accounts, login, or SSO stores a wallet address on the user
  record and needs no slug at all. See `wasm-client/SPEC.md` §4.
- What the slug buys is a site with **no server-side session state**. It is not
  primarily a way to dodge a cookie banner: an authentication session cookie is
  generally exempt from consent as strictly necessary, so a site that logs
  people in is already clear on that count.
- The slug is a secret, not a shareable link. Gifting a page view by sharing a
  URL is **out of scope**: a slug is a bearer token for the payer's whole
  remaining balance, so sharing one gifts the balance rather than an article.
  A gift-link feature, if it is ever wanted, belongs in the shape publishers
  actually use -- one time, expiring, one article.

![pay-as-you-go-state-machine](state-machine.png)

In the state diagram, the happy path has bold lines. It goes like this:
- viewer uses navigation to metered content with their bump slug
- server finds contract account using the slug
- server increments the usage by the page view amount
- if the viewer has accumulated sufficient unpaid usage, the server
  invokes a transfer from the viewer's wallet
- the server delivers the metered page after inserting the viewer's
  slug into any links to metered content

Viewers who come without a bump slug go to the sign-in or sign-up page.
The page view includes details about the contract and may allow the viewer
to select a limit.
- Sign-in invokes a wallet authorization and contract lookup using the
  wallet address.
- Sign-up invokes contract account creation and wallet authorization (signing)
  of the contract.

The dialog and the server must enforce a minimum for
the limit amount that is some multiple of the page view amount.
A multiple of one does not make much sense. Forty or fifty multiple yields a
better minimum. Also, the minimum should be greater than the threshold
amount for making a transfer transaction from the viewer's wallet.

With the account set-up and authorized, the viewer returns to the happy path.

When a viewer reaches their limit, the server shows them a screen that
provides a wrapup of the usage. It offers to renew the limit, or a new limit,
or to delete the contract after a transfer transaction for unpaid usage.

