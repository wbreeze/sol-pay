# SolPay

Pay-as-you-go for web content, using the Solana blockchain. A reader's wallet
pays a small fee per use, the money stays in the reader's wallet until
it is spent. Nothing in the protocol records who read what.

This repository delivers `sol-pay` — a metering program on Solana and a client
library.  A site uses the `sol-pay` client libraries to build the metering
program instructions and read its accounts.

The program is deployed on devnet at
[`F8UDAGgxVTm8Vmh4RmskpMBCFqhRvuTqbDxDCj8UMedL`][explorer], which a devnet
reset can empty without anything here changing. Look at it on Solana Explorer,
or ask a devnet node yourself:

```
solana program show F8UDAGgxVTm8Vmh4RmskpMBCFqhRvuTqbDxDCj8UMedL --url devnet
```

[explorer]: https://explorer.solana.com/address/F8UDAGgxVTm8Vmh4RmskpMBCFqhRvuTqbDxDCj8UMedL?cluster=devnet

## What is here

Two Rust crates, and one PHP port:

- [`pay-on-chain`](pay-on-chain) — the metering program, built with the
  [Anchor framework][anchor], and its LiteSVM test suite.
- [`wasm-client`](wasm-client) — the client library a site integrates,
  published as a crate and as a browser bundle. The same bundle runs on a Node
  server, so the server half of an integration does not have to be Rust
  either. Its API specification,
  [`wasm-client/SPEC.md`](wasm-client/SPEC.md), is the document to read before
  the code.
- [`php-client`](php-client) — a server-side PHP client covering the
  site-signed half of `wasm-client`'s API: PDA derivation, instruction
  building, account decoding, preflight, and error mapping, for a PHP server
  with no Rust toolchain and no WASM runtime. Published to Packagist from a
  subtree split; see [`php-client/README.md`](php-client/README.md), and
  [`wasm-client/SPEC.md` §3.1][spec31] for why a port exists at all.

[anchor]: https://www.anchor-lang.com/docs
[spec31]: wasm-client/SPEC.md#31-the-server-row-does-not-say-rust

## Payment model

The payment model is that a caller identifies with a wallet.  The wallet pays
for what they use-- a fee per use. The wallet signs a contract allowing
incremental charges up to a limit.  The site may ask to refresh the limit when
reaching it.

The metering program accumulates charges. It makes the pre-authorized transfer
from the wallet when the unpaid total reaches a collection threshold.
See [SPEC §4.6, "Why there is a collection threshold."][collect]

[collect]: wasm-client/SPEC.md#46-why-there-is-a-collection-threshold

A site running sol-pay executes code in two places, and they sign different
things. The browser holds the reader's wallet and never sees the site
authority; the server holds the site authority and never sees the reader's
key.

| | signs | artifact |
| --- | --- | --- |
| Browser | the reader, through a wallet adapter | the npm package |
| Server | the site authority | the crate, or the PHP port |

### Request flow

This state diagram shows the flow of a page request when navigating
metered content.

![pay-as-you-go-state-machine](state-machine.png)

The bold lines show the happy path.  It goes like this:

- viewer navigates to metered content, and the site knows their wallet address
- server derives the contract address from the site and wallet addresses,
  and reads the account
- server makes one metering call, which raises the usage by the page view
  amount
- that same call moves money only when the unpaid total has reached a
  collection threshold. The transfer is a cross-program invocation inside the
  metering instruction rather than a transaction of its own
- the server delivers the metered content

The diagram assumes the site can map a viewer to a wallet address, and says
nothing about how. Accounts, login, SSO — whatever the site already runs.
That mapping is the integrator's one obligation; everything else starts from
the address. See [`wasm-client/SPEC.md` §4][spec4].

[spec4]: wasm-client/SPEC.md#4-what-the-integrator-owns

Every contract is derived from the site and the payer's wallet address, so
identifying the viewer *is* finding the contract. There is no session token
in the protocol and nothing to look up but an account.

Only two authorizations appear in the flow: the site's authority over its
own contracts, which is what lets it meter, and the payer's authorization of
the spend, which is the SPL approval the whole design rests on.

Viewers without a contract go to the set-meter page. It includes details
about the cost and lets the viewer choose a limit. Setting the meter creates
the contract account and takes the payer's authorization of the spend, both in
one transaction. The authorization has to come first. The program checks
that it did.

The dialog and the server must enforce a minimum for
the limit amount that is some multiple of the page view amount.
A multiple of one does not make much sense. Forty or fifty multiple yields a
better minimum. The program itself requires the minimum to exceed the
collection threshold and refuses `initialize_site` otherwise
(`MinimumBelowThreshold`): a minimum at or below the threshold would let a
payer sign up for less than a single collection, so the first settle could
never fire within their limit.

With the account set-up and authorized, the viewer returns to the happy path.

When a viewer reaches their limit, the server shows them a screen that
provides a wrapup of the usage. It offers to renew the limit, at the same
amount or a new one, or to close the contract.

Closing forgives whatever is unpaid. That is a decision rather than an
omission. A transfer can always be refused -- a short balance, an approval
revoked or replaced, a frozen account -- and a close is where those are most
likely. A close carrying a transfer could fail and leave the payer unable
to leave. `close_contract` therefore emits `Closed { forgiven }` and moves no
money.

## What the integrator owns

This is a library, not an application. The cyan nodes in the state diagram --
`set_meter`, `manage_meter`, `metered_page` -- are screens the *integrator*
builds. They appear in the design only to establish what the library owes
them: the data needed to render each screen, and the operations its controls
invoke. Nothing else. The library does not route, render, format, or decide.

The obligation that comes with that is a single sentence: **keep a mapping
from your viewer to a wallet address, and hand us the address.** The payment
core needs exactly one input. `meter_and_settle` derives the contract from
`[b"contract", site, payer]`. Its accounts carry no session token of
any kind. Who the visitor is stays the site's own affair -- accounts, login,
SSO, whatever it already runs.

This library is authoritative about instruction encoding, PDA derivation,
account layout, the rule that `approve` must precede `open_contract` in the
same transaction, and the arithmetic that decides whether a meter call will
succeed. It has no view on what limit to suggest, how to format an amount, when
to show the meter, or what to do when a payment fails.

Metering may be an additional way to pay. A publisher with subscriptions may
opt to keep them; metering is what it offers the reader who will
not subscribe. [SPEC §4.4][spec44] has integration consequences for that
scenario.

[spec44]: wasm-client/SPEC.md#44-coexisting-with-a-subscription

## The demonstrator

[`wbreeze/sol-pay-demonstrator`][demo] is a working site that meters a set of
articles on devnet, built on the PHP port. It is a reference integration:
everything this repository declines to supply — RPC, the wallet adapter, the
session, the viewer-to-wallet map, the decision to meter a request, error
attribution, log hygiene — is there, in one place, in the smallest honest
form. Three of its screens are the three this library names.

[demo]: https://github.com/wbreeze/sol-pay-demonstrator

## Installing

The client library is published as `sol-pay-client`, once per language. The
three are one library: the same instructions, the same encoding, for the same
program.

```
cargo add sol-pay-client                 # a Rust server, or a Rust front end
npm install sol-pay-client               # the browser, and Node servers
composer require wbreeze/sol-pay-client  # a PHP server
```

[crates.io][crates] · [npm][npm] · [Packagist][packagist]

[crates]: https://crates.io/crates/sol-pay-client
[npm]: https://www.npmjs.com/package/sol-pay-client
[packagist]: https://packagist.org/packages/wbreeze/sol-pay-client

## Development

Every script in [`bin/`](bin) carries a header comment saying what it does,
what it proves and what it does not. Read that before the code.

Three artifacts. The program and the Rust client build with the same
toolchain; the PHP port shares nothing with either.

**The program.** [`./bin/build-rust --program`](bin/build-rust) builds it with
Anchor. [`./bin/test-rust`](bin/test-rust) runs its suite, and the client's,
passing any argument through to `cargo test` as a filter. No validator is
involved: the tests load the built `.so` into [LiteSVM][litesvm], an
in-process SVM. A local deploy is the one thing here that wants
`solana-test-validator`, and from a fresh clone it wants `anchor keys sync`
first, because the program keypair is not in the repository.
[`pay-on-chain/README.md`](pay-on-chain/README.md) has both.

**The Rust client and the browser bundle** are one source tree,
[`wasm-client`](wasm-client). The core is plain Rust under a thin
`wasm-bindgen` layer, so `./bin/test-rust` covers it natively;
[`./bin/build-rust --client`](bin/build-rust) produces the bundle in
`wasm-client/pkg`. [`wasm-client/README.md`, "Building"][wcbuild] gives the
bare `cargo` and `wasm-pack` equivalents.

**The PHP port**, [`php-client`](php-client), is `composer install` and
`composer test` from its own directory. Its floor is PHP `^8.1` while its test
tooling wants 8.2 — a trap with a standard way out, in
[`php-client/README.md`, "Building and testing"][pcbuild].

`build-rust` and `test-rust` pass `--locked`: they build what the committed
`Cargo.lock` files name, or they fail. Three further scripts test how the
library is *consumed* rather than how it is built, below.

[litesvm]: https://github.com/LiteSVM/litesvm
[wcbuild]: wasm-client/README.md#building
[pcbuild]: php-client/README.md#building-and-testing

### Cleanup:

*[`./bin/clean`](bin/clean)* removes build output. `--program` takes
  `pay-on-chain/target` and `.anchor`; `--client` takes `wasm-client/target`
  and `pkg`; no flag takes both. The script also removes
  `php-client/vectors-gen/target` and its `vectors.json`, which belong to
  neither flag because the consumption checks below share them. The `target`
  is a full build of the published crate and is the largest thing the script
  removes. It leaves `test-ledger` alone — the local validator's chain state
  is not build output — and `php-client/vendor`, which is an installed
  dependency tree that `composer install` restores rather than any build here.

### Consumption checks

Three scripts, for the three ways something that is not this library uses it.
[`./bin/test-node`](bin/test-node) guards a *loading contract* — Node runs the
same wasm the browser runs, so it cannot disagree with the crate
([SPEC §3.1][spec31]). [`./bin/test-php`](bin/test-php) is a *second
implementation* that can, and a divergent port signs the wrong thing
([§8.1][spec81]). [`./bin/test-kit`](bin/test-kit) is an agreement with an
*outside package neither side pins*, declared nowhere an installer can see it
([§8.2][spec82]). `bin/test-kit`'s header sets the three side by side; read it
before changing any of them.

[spec81]: wasm-client/SPEC.md#81-a-second-implementation-is-a-second-source-of-drift
[spec82]: wasm-client/SPEC.md#82-an-agreement-neither-side-declares

Each builds or generates what it finds missing, and names the tool it wants
when one is absent. `test-kit` reaches the network by design: the point is
the real kit from npm.

### CI

*[`node-conformance.yml`](.github/workflows/node-conformance.yml) and
[`php-conformance.yml`](.github/workflows/php-conformance.yml)* run `test-node`
and `test-php` on push to GitHub.  They execute across more than one runtime.

*[`kit-agreement.yml`](.github/workflows/kit-agreement.yml)* runs `test-kit` on
push to GitHub and on a weekly schedule.

*[`program-tests.yml`](.github/workflows/program-tests.yml)* runs the Rust
program tests using the locked build and both suites on push to GitHub.

*[`dependency-drift.yml`](.github/workflows/dependency-drift.yml)* re-resolves
the Rust Cargo versions from scratch
weekly, so an upstream release that breaks the version ranges shows up as a red
scheduled run instead of a surprise.

*[`./bin/update-locks`](bin/update-locks)* moves `Cargo.lock` files to the
newest versions inside the ranges the manifests already allow, prints what
moved, and then runs whichever suite builds from them. (It does not ask you
to.) It takes `--program`, `--client` or `--vectors`. The third is
[`php-client/vectors-gen`](php-client/vectors-gen),
proved by `test-php` rather than `test-rust` because nothing in the Rust suites
builds it. This is the deliberate answer to a dependency-drift report from CI;
it never crosses a major boundary, so adopting a newer major stays a manifest
edit and a decision.

### Publishing

Publishing is deliberately not scripted for any of the three. It is rare,
irreversible, and needs credentials that belong to a person rather than to a
repository. [`wasm-client/README.md`, "Publishing"][wcpub] covers `cargo
publish` and `wasm-pack publish`, which share one version number out of
[`Cargo.toml`](wasm-client/Cargo.toml).

[wcpub]: wasm-client/README.md#publishing

PHP needs one more step, because Packagist derives a version from a git tag
and a tag here would claim to version the other two artifacts as well.
[`./bin/split-php-client`](bin/split-php-client) produces the read-only
repository Packagist publishes `php-client` from, and proves it is publishable
— Composer needs `composer.json` at a repository root and a tag that *is* the
version. It verifies and stops: pushing, tagging and publishing leave the
machine and stay manual, the same as `cargo publish`. See
[`php-client/README.md`, "Publishing"][pcpub].

[pcpub]: php-client/README.md#publishing

## Licence

Dual licensed under either of

- Apache License, Version 2.0 ([`LICENSE-APACHE`](LICENSE-APACHE))
- MIT license ([`LICENSE-MIT`](LICENSE-MIT))

at your option. See [`LICENSE.md`](LICENSE.md) for why both.
