# sol-pay-client — API specification

Status: draft, revised 2026-08-31. Written before implementation; sections 6.2
onward describe code that does not exist yet.

**The bump slug is gone from the design as of this revision.** The program and
the client still carry it -- `Contract.slug`, `SlugIndex`, `SLUG_SEED`, a slug
argument on three instructions, `core::slug` -- and removing them is the next
change. Where this document and the code disagree about slugs, this document is
the target and the code is behind.

This specifies the client library that a site integrates. It is the companion
to `state-machine.plantuml` at the repository root, which remains the
authoritative description of the flow. Where this document and the diagram
disagree, the diagram is right and this document is a bug.

## 1. What this is

A library, not an application. sol-pay ships no user interface. The cyan nodes
in the state diagram -- `sign_up`, `account_page`, `metered_page` -- are
screens the *integrator* builds. They appear in the design only to establish
what the library owes them:

1. the data needed to render each screen, and
2. the operations its controls invoke.

Nothing else. The library does not route, render, format, or decide.

**Out of scope, decided 2026-08-31: identifying the viewer.** The library takes
a wallet address and says nothing about how the integrator obtained it. The
rationale is in §4.

## 2. The design rule

**Be rigid where the chain is rigid. Be silent where the site has a
legitimate choice.**

An integrator rejects a library that dictates their product. They keep one
that stops them shipping bugs. Everything below is sorted by that rule.

### Chain facts — the library is authoritative

Wrong here costs a failed transaction and a real fee, so the library owns
these completely and no integrator should reimplement them:

- instruction encoding, account order, and Anchor discriminators
- PDA derivation for site and contract
- account layout and decoding
- the rule that `approve` must precede `open_contract` / `renew_contract` in
  the same transaction
- the arithmetic that decides whether a meter call will succeed
- that `approve` *replaces* an allowance rather than adding to it
- the mapping from an on-chain error code to a cause the UI can branch on

### Site policy — the library has no view

- what limit to suggest or offer
- how to format an amount, or in what currency to display it
- when to show an account page, redirect, or block
- whether metering is per view, per article, or batched
- **how the server identifies the visitor** (§4)
- whether sign-in precedes sign-up
- what to do when a payment fails

If a future API would decide any of the above, it does not belong here.

**The line, stated once: describe the chain's state, do not prescribe the
site's response.**

## 3. Two consumers

A site running sol-pay executes code in two places, with different needs.

| | signs | needs | artifact |
| --- | --- | --- | --- |
| Browser | payer, via wallet adapter | `approve_checked`, `open_contract`, `renew_contract`, `close_contract`, `revoke`, decoders, preflight | npm package |
| Server | site authority | `meter_and_settle`, `initialize_site`, decoders, preflight | crates.io crate |

The server never holds the payer's key; the browser never holds the site
authority. `meter_and_settle` is the only instruction the site signs, and the
program enforces it (`has_one = authority`).

## 4. Identifying the viewer is not this library's job

This section exists because the library must not force a site to abandon the
login it already has, and because an earlier draft of this design tried to
solve identity on the site's behalf.

### 4.1 The payment core needs one input

`meter_and_settle` derives the contract from `[b"contract", site, payer]`. Its
eight accounts carry no session token of any kind. `open_contract`,
`renew_contract` and `close_contract` are the same: the payer signs, and the
payer's address is the seed.

So the payment core needs exactly one thing from the integrator: **the payer's
wallet address**. Two questions are cleanly separable, and only the second is
ours:

- **Who is this visitor?** — the site's own affair. Accounts, login, SSO,
  whatever they already run.
- **What do they owe?** — the payment core.

The integrator's obligation is therefore a single sentence: *keep a mapping
from your viewer to a wallet address, and hand us the address.* Everything the
library does starts from there.

### 4.2 What this replaces

Earlier revisions specified a "bump slug": a random token in the URL path,
indexed on chain by a `SlugIndex` PDA, that resolved a request to a contract
without the site holding any session state. It carried two justifications and
neither survived.

**Avoiding a cookie-consent banner.** Authentication session cookies are the
textbook "strictly necessary" case under the ePrivacy Directive and need no
banner, provided they are used only for authentication and are session rather
than persistent "remember me" cookies. A site that logs people in was already
clear on that count. (Not legal advice; the integrator's counsel decides.)

**Gifting a page view by sharing a link.** A slug in a URL is a bearer token
for the payer's *entire remaining balance*, so sharing one gifts the balance
rather than an article. The familiar publisher feature -- a one-time, expiring,
single-article gift link -- is a different mechanism, and if it is ever wanted
it should be built in that shape.

Removing the slug also removes a defect. Because `Contract.slug` was a plain
field in a public account and the contract PDA derives from public seeds, every
contract was enumerable -- one `getProgramAccounts` on the discriminator, or
the program's own transaction history -- and each hit yielded a working bearer
token. A third party could consume any payer's balance to the limit. That is
theft of service rather than of funds, since the money reaches the site
treasury, which makes a content scraper the likelier actor than a vandal. No
slug, nothing to steal.

What is genuinely given up: a site can no longer run sol-pay with **no**
server-side session state. Every integrator now keeps a viewer-to-wallet
mapping. That was a real property and it is being traded deliberately.

### 4.3 What remains public

Contracts stay readable by anyone: given a site and a wallet address, the
limit, used and paid figures are on chain in the clear. That is a visibility
property of putting a spend meter on a public ledger, not a defect, and no
client library can change it. Integrators whose viewers would care should be
told plainly rather than reassured.

## 5. Two published artifacts

Decided 2026-08-31.

**`sol-pay-client` on crates.io** — the `core` module as a normal Rust crate,
for a site's Rust server. Requires flipping the feature default: today
`default = ["wasm"]`, which would drag `wasm-bindgen` into a server build. It
becomes `default = []` with `wasm` opt-in.

**An npm package** — the `wasm-pack --target web` output: the `.wasm`, the JS
glue, and the generated `.d.ts`.

One source tree, one version number.

Carried over from the dependency policy: a published crate's `Cargo.lock` is
ignored by consumers. They re-resolve inside the ranges in `Cargo.toml`, so
those ranges become the real compatibility contract on the day this ships.

## 6. API surface

### 6.1 Write path — exists

Instruction builders in `core::ix`, address derivation in `core::pda`.
Unchanged by this spec, except that `core::slug` and `slug_index_address` go
away with the slug. Every builder stays public, so an integrator can compose
transactions their own way.

### 6.2 Read path — `core::state`, to be written

Nothing in the library reads chain state today, so `account_page` cannot be
rendered by a consumer at all: its "show current limit, cost per page, used and
paid amount" spans the `Contract` and `Site` accounts and we export no way to
decode either.

Layouts in field order, each preceded by an 8-byte Anchor account
discriminator (`sha256("account:<Name>")[..8]`):

| account | fields | bytes |
| --- | --- | --- |
| `Site` | authority `[u8;32]`, mint `[u8;32]`, treasury `[u8;32]`, page_price `u64`, collection_threshold `u64`, min_limit `u64`, bump `u8` | 129 |
| `Contract` | site `[u8;32]`, payer `[u8;32]`, limit `u64`, used `u64`, paid `u64`, bump `u8` | 97 |

`Contract` is shown as it will be once the slug is removed. The deployed
account still carries `slug: [u8;16]` and `slug_bump: u8` and is 114 bytes;
decoders are written against the 97-byte form and land with the program
change, not before.

```rust
impl Site     { pub fn decode(data: &[u8]) -> Result<Self, DecodeError>; }
impl Contract { pub fn decode(data: &[u8]) -> Result<Self, DecodeError>;
                pub fn unpaid(&self) -> u64;        // used - paid
                pub fn outstanding(&self) -> u64; } // limit - paid

/// SPL mint decimals, one byte at a fixed offset. `approve_checked` needs it
/// and there is nowhere else to get it without decoding a mint by hand.
pub fn mint_decimals(mint_account_data: &[u8]) -> Result<u8, DecodeError>;

/// Base units <-> a human amount, at a mint's decimals.
pub fn to_base_units(amount: &str, decimals: u8) -> Result<u64, UnitsError>;
pub fn from_base_units(units: u64, decimals: u8) -> String;
```

`decode` verifies the discriminator and length before reading. Sizes are not
written as literals: the parity test asserts them against the program's
`INIT_SPACE`, so a field added on chain fails a test here.

The unit conversions are not a convenience. Every amount in this API is in
mint base units and USDC has six decimals, so an integrator who scales twice
turns an intended 50 USDC into 50,000,000 of allowance. Nothing rejects it --
`approve` checks no balance, and the program's delegate check only compares the
allowance against the limit -- so the payer's chosen cap silently becomes their
whole balance. Owning the conversion removes the error class; validating its
output could not, because no validator knows what the payer meant.

`to_base_units` takes a decimal string rather than a float: `0.1` is not
representable in binary floating point, and a payment library that rounds is
not one an integrator can audit.

### 6.3 Preflight — `core::preflight`, to be written

The choice nodes in the diagram are arithmetic over `Contract` and `Site`, and
the minimum-limit rule appears at four places in the flow. If each integrator
reimplements them they drift from the program, and the drift surfaces as a
rejected transaction the payer paid fees for.

These return **facts, not instructions**. Nothing here says what to render.

```rust
pub fn charge(site: &Site, page_views: u32) -> Option<u64>;
pub fn can_meter(c: &Contract, s: &Site, page_views: u32) -> Result<(), Blocked>;
pub fn will_settle(c: &Contract, s: &Site, page_views: u32) -> bool;
pub fn views_remaining(c: &Contract, s: &Site) -> u64;
pub fn limit_floor(s: &Site, contract: Option<&Contract>) -> u64;
pub fn required_allowance(limit: u64) -> u64;
```

Each mirrors one on-chain check exactly:

- `can_meter` -> `used + page_price * views <= limit`, else `LimitReached`
- `will_settle` -> `used + charge - paid >= collection_threshold`
- `limit_floor` -> `max(site.min_limit, contract.unpaid())`, taking the second
  term as zero when there is no contract. That single expression is both
  renewal requirements at once -- at or above the site minimum, and covering
  usage carried forward -- and degenerates to the sign-up rule when the
  `Option` is `None`
- `required_allowance` -> the full limit; nothing is paid against a new limit
  yet, so the allowance must cover all of it

`limit_floor` is deliberately one function rather than an open-limit and a
renewal-limit pair. The question is identical on both screens -- what is the
smallest value I can accept here -- and the `Option` carries state the caller
already holds, since `find_contract` either produced a contract or did not.
Two functions would invite calling the sign-up one on the renewal screen,
which passes `site.min_limit`, sits below `unpaid`, and fails with
`LimitBelowUsage` only after the payer has approved and signed. One function
makes that unrepresentable.

It returns a plain `u64` and not a struct naming which term bound. A caller
holding both accounts can compare them and say "at least 5, because you carry
3.20 unpaid" without our help.

### 6.4 Errors — `core::error`, to be written

An integrator currently receives a numeric code and a log string, and must
match on text to tell "limit reached" (route to renewal) from "insufficient
funds" (tell the payer their wallet is short).

Those two errors come from **different programs, in different code spaces**,
and that is the part a naive typed enum would miss:

| cause | program | code |
| --- | --- | --- |
| `LimitReached` | this program | 6003 |
| `InsufficientFunds` | SPL Token, via the `transfer_checked` CPI | 1 |

Anchor numbers `#[error_code]` variants from 6000 in declaration order, so our
nine occupy 6000..6008. SPL Token's occupy a small range from 0. A bare number
never says whose it is, so the library models the space, not just our half:

```rust
pub enum PayError { /* the program's nine variants, in declaration order */ }
pub enum TokenError { /* the SPL variants sol-pay can actually provoke */ }

pub enum Cause {
    Program(PayError),
    Token(TokenError),
    Unknown { program: Pubkey, code: u32 },
}

impl PayError {
    pub fn from_code(code: u32) -> Option<Self>;
    pub fn code(&self) -> u32;
    pub fn message(&self) -> &'static str;
}

pub fn cause(program: &Pubkey, code: u32) -> Cause;
```

`Unknown` is deliberate: the runtime can surface errors from programs neither
we nor the integrator anticipated, and a library that maps those onto its own
enum is lying.

Attributing a code to a program means reading the transaction logs, which the
library does not do -- see `README.md`, "Transaction logs are yours to filter".
`cause` takes the program id the caller extracted.

#### Diagnosis, because one code is ambiguous

SPL Token appears to return `InsufficientFunds` both when the payer's balance
is too low and when the delegate's approved allowance is too low. The published
enum documentation does not distinguish them, so **this must be settled by a
LiteSVM test rather than by reading docs** -- revoke the approval, meter, and
record the code -- in the same spirit as everything else in §8.

The distinction matters because the two need opposite responses: a short
balance means "top up your wallet", a short allowance means "re-authorize".
A short allowance is genuinely reachable -- the token account belongs to the
payer, who may revoke or approve elsewhere at any time, and the program only
checks the delegate at open and renew.

Neither is inferable from the code. Both are answerable by reading the payer's
token account:

```rust
pub enum Shortfall { Balance { short: u64 }, Allowance { short: u64 }, Neither }

/// Given the payer's token account and what the next settle would move,
/// say which constraint is short. A read, not a guess.
pub fn diagnose(token_account_data: &[u8], unpaid: u64) -> Result<Shortfall, DecodeError>;
```

This is still "describe the chain's state, do not prescribe the site's
response": it reports which quantity is short and by how much, and says nothing
about what to show.

### 6.5 Ordered transactions — `core::tx`, to be written

"The approve must precede the program instruction" is currently a paragraph in
a README. It is the rule an integrator gets wrong once and then debugs for an
hour, and the program deliberately refuses to trust the client on it.

```rust
pub fn open_contract_tx(..) -> Vec<Instruction>;   // approve_checked, open_contract
pub fn renew_contract_tx(..) -> Vec<Instruction>;  // approve_checked, renew_contract
pub fn close_contract_tx(..) -> Vec<Instruction>;  // close_contract, revoke
```

Convenience, not a gate: the individual builders stay public and nothing is
reachable only through these. They exist so the correct thing is also the
shortest thing to write.

`close_contract_tx` pairs the close with a revoke. The leftover approval is
inert once the contract account is gone -- the PDA can no longer sign -- but it
stays visible in the payer's wallet until withdrawn.

### 6.6 Wallet sign-in — documented, not shipped

Decided 2026-08-31: **the library does not implement SIWS verification.** This
section specifies what an integrator may use, and names what to use.

It is one option, not a requirement, and it is deliberately absent from the
state diagram: per §4 the library is silent on how a viewer is identified. It
appears here because a site with no account system will ask, and because the
answer has sharp edges.

Sign In With Solana is a `signIn` feature in the Wallet
Standard: the site builds a structured message -- domain, address, statement,
uri, version, chainId, nonce, issuedAt, expirationTime, requestId, resources --
the wallet displays and signs it, and the server verifies the signature against
the message it issued.

Use `@solana/wallet-standard-util`'s `verifySignIn` in JavaScript, or an
existing `siws` crate in Rust. Shipping our own would mean a second
implementation of the message's ABNF format alongside whichever one the
integrator already uses, and two sources of truth for a byte-exact format is
the drift hazard this project avoids everywhere else. It would also put ed25519
into a crate that currently carries almost no dependencies.

Three things are easy to get wrong and are therefore stated here rather than
left to the reader:

- **The `signIn` feature is not universal.** Phantom's documentation notes
  support from Phantom extension 23.11.0 onward. Where the feature is absent,
  fall back to connect plus `signMessage` over the same message bytes;
  verification is identical either way.
- **The nonce is the site's to generate and to remember, and expiry must be
  checked.** A verifier that ignores `expirationTime` accepts a replayed
  sign-in forever.
- **A verified address is not a contract.** What sol-pay needs from sign-in is
  exactly one value: the payer's wallet address, which is the `payer` seed in
  `[b"contract", site, payer]`. Once verification succeeds, hand that pubkey to
  `contract_address` and the payment core takes over. Everything between the
  signature and that derive is the site's session, not ours.

Follow-on not yet decided: whether the browser package still ships a message
*constructor*. If the integrator verifies with a library that derives the
expected message from its own input type, our constructor is a redundant second
definition of the same format -- the argument above applied to the other half.
Leaning to dropping it and specifying the field set here instead.

### 6.7 JavaScript surface

Every item above gets a `wasm_bindgen` wrapper on the existing convention:
camelCase `js_name`, base58 strings for addresses, `JsError` for failures.

One constraint: `u64` amounts must cross as `BigInt`, not `number`. A JS
number loses precision above 2^53, and while USDC balances will not reach it, a
library that silently truncates is not one an integrator can audit.

## 7. What the library never does

Signing, signature verification, RPC, retries, storage, routing, rendering,
session management. It builds instructions and decodes bytes. The integrator
owns the wallet adapter, the connection, the session, and the page.

This is the boundary integrators get wrong, so it is stated here and repeated
in the README.

## 8. Drift control

Every claim this document makes about the program is pinned by a test in
`pay-on-chain/tests`, the one place both sides build together:

- instruction bytes against Anchor's generated types (exists)
- the hardcoded program id against `declare_id!` (exists)
- account discriminators against `sha256("account:<Name>")`
- decoded account sizes against `INIT_SPACE`
- each `PayError` variant against the program's discriminant
- the code SPL Token actually returns for a short balance and for a revoked or
  reduced allowance, which is what settles the ambiguity in §6.4
- preflight predicates against the program's behaviour: for a case a predicate
  calls blocked, the matching LiteSVM call must actually fail, with the same
  error

The last one is the point of the exercise. A predicate that disagrees with the
program is worse than no predicate.

## 9. Open questions

### 9.1 Program id: fixed or overridable?

The id is compiled in, so a published package is bound to one deployment. The
`Site` PDA is seeded by authority, so one deployment already serves many sites
with independent pricing -- which suggests a single canonical deployment is the
intended model and the fixed id is a feature. But an integrator wanting their
own deployment then cannot use the published package at all. An override
defaulting to the canonical id costs little and removes a reason to reject the
library.

### 9.2 Does WASM discourage adoption; should a plain-JS package exist?

Raised, deferred, not analysed.
