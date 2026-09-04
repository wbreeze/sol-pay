# sol-pay-client — API specification

Status: draft, revised 2026-09-01. §4.5, §5 and §6.1 through §6.5 are
implemented; §6.6 ships nothing, by decision. Published to crates.io and npm
as 0.1.0 on 2026-09-01.


This specifies the client library that a site integrates. It is the companion
to `state-machine.plantuml` at the repository root, which remains the
authoritative description of the flow. Where this document and the diagram
disagree, the diagram is right and this document is a bug.

## 1. What this is

A library, not an application. sol-pay ships no user interface. The cyan nodes
in the state diagram -- `set_meter`, `manage_meter`, `metered_page` -- are
screens the *integrator* builds. They appear in the design only to establish
what the library owes them:

1. the data needed to render each screen, and
2. the operations its controls invoke.

Nothing else. The library does not route, render, format, or decide.

**Out of scope, decided 2026-08-31: identifying the viewer.** The library takes
a wallet address and says nothing about how the integrator obtained it. The
rationale is in §4.

**Metering is an additional way to pay, not a replacement for the ones a site
already has.** A publisher with subscriptions keeps them. Metering is what it
offers the reader who will not subscribe -- the one who arrived from a link,
wants one article, and would otherwise bounce off the paywall. Nothing in this
library assumes it is the only way a viewer can reach the content, and the
integration consequences of that are in §4.4.

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
- when to show the meter, redirect, or block
- whether metering is per view, per article, or batched
- **how the server identifies the visitor** (§4)
- what to do when a payment fails

If a future API would decide any of the above, it does not belong here.

**The line, stated once: describe the chain's state, do not prescribe the
site's response.**

## 3. Two consumers

A site running sol-pay executes code in two places, with different needs.

| | signs | needs | artifact |
| --- | --- | --- | --- |
| Browser | payer, via wallet adapter | `approve_checked`, `open_contract`, `renew_contract`, `close_contract`, `revoke`, decoders, preflight | npm package |
| Server | site authority | `meter_and_settle`, `initialize_site`, decoders, preflight | crates.io crate, or a port -- §3.1 |

The server never holds the payer's key; the browser never holds the site
authority. `meter_and_settle` is the only instruction the site signs, and the
program enforces it (`has_one = authority`).

### 3.1 The server row does not say Rust

The browser row is settled by something outside anyone's choice: Wallet
Standard is browser JavaScript, so the payer signs in a browser whatever else
is true. The server row is not settled. It says what the server *needs*, not
what it is written in, and an early draft of this table said "crates.io crate"
as though those were the same thing.

A survey of the sites that would plausibly adopt this, 2026-09-02, found the
assumption expensive. PHP is the server-side language of about 70% of the sites
W3Techs can identify, and of essentially the entire CMS market; Node serves
most of the large publishers that already run paywalls; Rust is not tracked at
all, and appears nowhere in the sub-0.1% tail. No news publisher of any size
was found running a Rust web application server. **A crates.io crate on its own
therefore reaches on the order of 1% of candidate integrations, and
approximately none of the ones that already have a paywall to replace.**

That is not an argument against the crate. It is the right core and the right
place for the encoding, and everything else here is built from it. It is an
argument that the crate is not by itself a distribution strategy.

What exists so far is `php-client`: a port of this row -- the site-signed
instructions, the decoders, preflight, units and errors -- for a server with
no Rust toolchain and no WASM runtime. The payer-signed half is absent from it
deliberately, since those are signed in a browser regardless of what the server
runs. A port is the most expensive of the available answers, and §8.1 says what
it costs to keep one honest.

**The Node tier is already served, by the artifact that exists.** Measured
2026-09-04; it needed no build target and no second package. Nothing in the
wasm layer is browser-specific -- it pulls in `wasm-bindgen` and
`serde-wasm-bindgen` and converts values, with no `web-sys`, no `js-sys` and
no fetch -- so the `--target web` bundle runs unchanged under Node. There is
exactly one difference: the zero-argument `init()` resolves the `.wasm`
against `import.meta.url` and fetches it, and Node's fetch does not do `file:`
URLs, so a Node caller passes the bytes instead. Against the same vectors
`php-client` is checked with, the bundle under Node matched the published
crate on 400 `site` PDAs, 400 `contract` PDAs, the `meter_and_settle` data and
account order, and `decodeSite`. `README.md` carries the recipe.

That is a claim about `wasm-pack`'s generated glue and not about anything in
this repository, which is why it is tested rather than asserted: `bin/test-node`
and the `node conformance` workflow. It is not §8.1's kind of exposure. Node
runs the same wasm binary the browser runs and cannot diverge from it; what
could break is the loading contract, silently, in a release of a tool.

The remedy is a **sidecar** with a documented HTTP interface: one deployment
unit that reaches any language at all, for the people who would rather have
that than a package. This section carried it as explicitly undecided from the
day it was written; it is **decided 2026-09-04**, and what decided it was not
the unserved-language list below, which has not moved. It was that a PHP site
authority needs transaction assembly it cannot get honestly from any existing
package, and that the demonstrator needs somewhere for a site authority key
to live. Those are one object: a sidecar is a signing oracle, so it is
simultaneously the reach answer and the custody answer, and deciding either
of them alone would have answered the same question twice, differently.

What is decided is that it gets built, not what it is. Its trust boundary --
a Unix socket and file permissions, or mTLS -- belongs to this library rather
than to each integrator's invention, and is not designed yet; neither is
whether it lives in this repository -- and if it ever drags too many releases
along behind it, splitting it into its own code base stays available, which
is the same remedy `php-client/README.md` already records for Packagist's
repo-wide tags.

What the sidecar does not reach is PHP. A sidecar is the answer for a
language with no package; PHP has one, and one whose zero runtime
dependencies and 8.1 floor are the reason it installs where it has to, so
**PHP reaches transaction assembly through `SolPay\Tx` instead** -- decided
2026-09-04, `php-client/README.md`, "Planned: transaction assembly". The
consequence is deliberate and is not softened here: this library keeps a
second implementation of the encoding, §8.1's objection included, and §8.1
names the machinery that has to carry it.

What is left unserved by a Rust crate, a Node-loadable bundle and a PHP port
is Ruby, Java, Scala, ASP.NET and Python: together roughly a fifth of the
sites W3Techs can identify, and, unlike PHP, fragmented -- no single CMS mass
the size of WordPress to aim a port at. That shape is the argument for one
sidecar over four more ports, and it is the whole of the case either way.

## 4. What the integrator owns

Two things sit outside this library that an integrator has to get right
anyway: knowing who the viewer is, and deciding whether this request should be
metered at all. Neither is ours. This section says so explicitly, because an
earlier draft of the design tried to solve the first on the site's behalf.

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

**With one precondition, which is easy to miss.** That sentence holds while
payers use their associated token account -- the one wallets surface, and the
one any ordinary onboarding produces. It is not what the program requires.
`open_contract`, `renew_contract` and `meter_and_settle` constrain
`payer_token_account` only by its owner and its mint, so a payer may hold
several token accounts for one mint, and `Contract` does not record which one a
contract was opened against.

That latitude is what lets a payer hold concurrent contracts with more than one
site: one delegate per token account rather than one per wallet.
A site that supports it stores a second thing per viewer, the token account,
because `meter_and_settle` takes it as an account and nothing derives it. The
mapping becomes *viewer to address and token account*.

So the claim above is a claim about the default, not about the program. The
objection this answers, and what the extra account costs a payer, are in
`README.md` under "Can a payer be metered by more than one site at once?".

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

### 4.4 Coexisting with a subscription

The state diagram draws the metered path. It does not draw the decision that
precedes it -- whether this request should be metered at all -- because that
decision belongs to the site and will usually be made against a subscription
or entitlement the library knows nothing about.

Three consequences for an integrator running both:

**A contract is not a viewer type.** Someone may hold a subscription and a
contract at once: a subscriber reading outside their tier, or a metered reader
who subscribes later and whose contract is still open. Keying access control
off "has a contract" will eventually charge a subscriber. The contract answers
what a viewer has authorized, never whether they are entitled to the page.

**Metering the same view twice is the site's problem to avoid.** The program
meters whatever `page_views` the authority passes and has no idea the viewer
also has a subscription. The library's `can_meter` reports whether a charge
would succeed, not whether it should happen. Only the site knows that.

**Ending a meter takes two instructions, not one.** When a metered reader
subscribes, the site will want the meter to stop. Simply not calling
`meter_and_settle` is not enough: the payer's delegate approval stays in place,
and because a token account has exactly one delegate, that dormant approval
blocks the payer from opening a contract with any other site. Close the
contract *and* revoke -- which is what `close_and_revoke` does (§6.5).

Closing forgives the residue, so the site absorbs whatever was unpaid. That is
bounded below the collection threshold by construction, so it is small, but it
is not nothing, and it is a real cost of converting a metered reader into a
subscriber.

### 4.5 Which deployment, and which token program

Decided 2026-09-01. Neither is a constraint; both are defaults, and both are
stated once rather than at every call site.

**The program id.** The `Site` PDA is seeded by authority, so one deployment
already serves many sites with independent pricing, and a single canonical
deployment remains the intended model. But compiling the id in as the *only*
id means an integrator who wants their own deployment cannot use the published
package at all. That is a reason to reject the library, and it costs almost
nothing to remove.

**The token program.** A mint belongs to SPL Token or to Token-2022, and every
instruction that touches the payer's token account has to name the right one.
Strictly this is a property of the *mint*, not of the deployment -- one
deployment serves many sites, each site names its own mint, so two sites on one
deployment could differ. It sits on the handle anyway, because a client
instance serves one site, and repeating the same word at nine call sites to say
so is worse. A caller that really does span both holds two handles.

`core::Program` carries the pair. `Program::default()` is the canonical
deployment on SPL Token; `Program::new(id)` changes the first,
`.with_token_program(id)` the second, and they vary independently. Every
derivation, instruction and error name hangs off the handle as a method, and
the free functions in `core::pda`, `core::ix`, `core::tx` and `core::error` are
those same methods with the default filled in -- so the common case costs
nothing and each override costs one call. Across the WASM boundary the same
thing is the `PayOnChain` class (§6.7).

Consequences worth stating outright:

- **`error::cause` follows the deployment.** It decides "is this one of ours"
  by comparing the raising program against the handle's address. Had the
  override reached the instruction builders but not this comparison, a site on
  its own deployment would have seen every named failure in §6.4 quietly
  degrade to `Unknown` -- the library's error vocabulary lost to exactly the
  integrator the override was built for.
- **`approve_checked` depends on both.** It goes to the handle's token program,
  and the delegate it names is the handle deployment's contract PDA.
- **`revoke` depends on the token program alone.** It names only the token
  account and its owner, so it says nothing about which deployment held the
  allowance -- but it still has to be *sent* somewhere, so it is a method like
  the rest rather than the free-standing exception it was for one day.
- **On `meter_and_settle` the token program is an account, not the callee.**
  The instruction still goes to the metering program, which CPIs into the token
  program to move the money.

Nothing is verified about the program id: an address with no program behind it
builds perfectly good instructions that fail at the runtime, and confirming a
deployment exists needs a network this library does not have and does not want.
The token program has a cheap check, because the answer is already in hand --
`Program::owns_mint` takes the `owner` that came back beside the mint's data
from `getAccountInfo` and says whether it matches. Moving a required argument
into state removes nine chances to get it wrong and adds one; that method is
the one.

## 5. Two published artifacts

Decided 2026-08-31, packaged 2026-09-01.

**`sol-pay-client` on crates.io** — the `core` module as a normal Rust crate,
for a site's Rust server.

**An npm package** — the `wasm-pack --target web` output: the `.wasm`, the JS
glue, and the generated `.d.ts`. It serves both rows of §3, not just the
browser: a Node server loads the same bundle by handing `init()` the bytes.
See §3.1.

One source tree, one version number. `wasm-pack` derives `pkg/package.json`
from the `[package]` table in `Cargo.toml`, so name, version, description,
license and repository are kept in one place rather than two.

The feature default was the thing standing in the way and is now flipped:
`default = []` with `wasm` opt-in, so a site's Rust server can depend on this
crate without wasm-bindgen and three serde crates arriving with it. Whatever
wants the browser bundle asks for the feature by name — `bin/build-rust`
does, and so does the drift canary, which is now the only job that compiles
the browser layer against a fresh resolve.

Ordering matters once, at the first publish. Flipping a default feature after
release is a breaking change for anyone who already depends on the crate, so
it had to land first; the same is true of any further reshaping of the API
surface. The registry names are also unclaimed and unreserved — see the
README's "Publishing" for what to check before the first push.

Carried over from the dependency policy: a published crate's `Cargo.lock` is
ignored by consumers. They re-resolve inside the ranges in `Cargo.toml`, so
those ranges become the real compatibility contract on the day this ships.

**A third, later, and not yet published.** `php-client` (2026-09-03) packages
the server row of §3 for PHP, as `wbreeze/sol-pay-client` on Composer. It is
not on Packagist and nothing depends on it yet. It is nonetheless meant to
become a real artifact rather than to stay a demonstration, which is why §8.1's
conformance job exists rather than being advice for later.

## 6. API surface

### 6.1 Write path — exists

Instruction builders in `core::ix`, address derivation in `core::pda`, both
also reachable as methods on `core::Program` (§4.5). Every builder stays
public, so an integrator can compose transactions their own way.

### 6.2 Read path — `core::state`, `core::units`

`core::state` also decodes the payer's SPL token account (`TokenAccount`:
mint, owner, amount, delegate, delegated_amount), which `diagnose` in §6.4
needs and a site's UI usually wants anyway.

Nothing in the library reads chain state today, so `manage_meter` cannot be
rendered by a consumer at all: its "show current limit, cost per page, used and
paid amount" spans the `Contract` and `Site` accounts and we export no way to
decode either.

Layouts in field order, each preceded by an 8-byte Anchor account
discriminator (`sha256("account:<Name>")[..8]`):

| account | fields | bytes |
| --- | --- | --- |
| `Site` | authority `[u8;32]`, mint `[u8;32]`, treasury `[u8;32]`, page_price `u64`, collection_threshold `u64`, min_limit `u64`, bump `u8` | 129 |
| `Contract` | site `[u8;32]`, payer `[u8;32]`, limit `u64`, used `u64`, paid `u64`, bump `u8` | 97 |


```rust
impl Site     { pub fn decode(data: &[u8]) -> Result<Self, DecodeError>; }
impl Contract { pub fn decode(data: &[u8]) -> Result<Self, DecodeError>;
                pub fn unpaid(&self) -> u64;        // used - paid
                pub fn outstanding(&self) -> u64; } // limit - paid

/// SPL mint decimals, one byte at a fixed offset. `approve_checked` needs it
/// and there is nowhere else to get it without decoding a mint by hand.
pub fn mint_decimals(mint_account_data: &[u8]) -> Result<u8, DecodeError>;

// core::units
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

### 6.3 Preflight — `core::preflight`

The choice nodes in the diagram are arithmetic over `Contract` and `Site`, and
the minimum-limit rule appears at three places in the flow. If each integrator
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
  usage carried forward -- and degenerates to the opening rule when the
  `Option` is `None`
- `required_allowance` -> the full limit; nothing is paid against a new limit
  yet, so the allowance must cover all of it

`limit_floor` is deliberately one function rather than an open-limit and a
renewal-limit pair. The question is identical on both screens -- what is the
smallest value I can accept here -- and the `Option` carries state the caller
already holds, since `find_contract` either produced a contract or did not.
Two functions would invite calling the opening one on the renewal screen,
which passes `site.min_limit`, sits below `unpaid`, and fails with
`LimitBelowUsage` only after the payer has approved and signed. One function
makes that unrepresentable.

It returns a plain `u64` and not a struct naming which term bound. A caller
holding both accounts can compare them and say "at least 5, because you carry
3.20 unpaid" without our help.

### 6.4 Errors — `core::error`

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
pub struct Shortfall {
    pub balance_short: u64,     // zero when the balance covers it
    pub allowance_short: u64,   // zero when the allowance covers it
    pub delegate_present: bool, // SPL clears the delegate at zero allowance
}

/// Given the payer's decoded token account and what the next settle would
/// move, say which constraints are short. A read, not a guess.
pub fn diagnose(account: &TokenAccount, unpaid: u64) -> Shortfall;
```

A struct rather than a verdict enum, and this is the design rule applied: both
constraints can be short at once, and picking one to report would be
prescribing a response. The site decides whether "top up" or "re-authorize"
comes first. It takes a decoded `TokenAccount` rather than bytes so that
decoding stays in `core::state` and a caller who already fetched the account
does not decode it twice.

This is still "describe the chain's state, do not prescribe the site's
response": it reports which quantity is short and by how much, and says nothing
about what to show.

### 6.5 Ordered transactions — `core::tx`

"The approve must precede the program instruction" is currently a paragraph in
a README. It is the rule an integrator gets wrong once and then debugs for an
hour, and the program deliberately refuses to trust the client on it.

```rust
// core::tx, and the same three names as methods on core::Program.
pub fn approve_and_open(..)  -> [Instruction; 2];  // approve_checked, open_contract
pub fn approve_and_renew(..) -> [Instruction; 2];  // approve_checked, renew_contract
pub fn close_and_revoke(..)  -> [Instruction; 2];  // close_contract, revoke
```

Both instructions in each pair go to programs the handle already names, so
none of the three takes a program address as an argument.

The names say the pair and its order rather than repeating the name of the
instruction they wrap. The module used to supply that distinction -- `tx::open_contract`
against `ix::open_contract` -- but once both are methods on `Program` the
module namespace is gone and the two would collide outright.

Fixed-size arrays, not `Vec`: the length is part of what each one promises.

Convenience, not a gate: the individual builders stay public and nothing is
reachable only through these. They exist so the correct thing is also the
shortest thing to write.

`close_and_revoke` pairs the close with a revoke. The leftover approval is
inert once the contract account is gone -- the PDA can no longer sign -- but it
stays visible in the payer's wallet until withdrawn.

### 6.6 Wallet sign-in — documented, not shipped

Decided 2026-08-31 and finished 2026-09-01: **the library ships no sign-in code
at all** — not verification, not message construction, not a message type. This
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

**The message constructor is dropped too** (decided 2026-09-01), which is the
argument above applied to the other half. Verification and construction are the
same format read in two directions: a library that verifies derives the
expected message from its own input type, so a constructor of ours would be a
second definition of a byte-exact format, and two definitions of one format
disagree eventually. Whichever library the integrator verifies with builds the
message as well.

What replaces it is this section. The field set is named above, and the ABNF
that orders and formats those fields belongs to the SIWS specification rather
than to this document or this crate -- pointing at the authority is the whole
technique, here as with log parsing and RPC.

### 6.7 JavaScript surface

Every item above gets a `wasm_bindgen` wrapper on the existing convention:
camelCase `js_name`, base58 strings for addresses, `JsError` for failures.

The split follows §4.5. Anything that depends on the deployment or the token
program is a method on the `PayOnChain` class; everything else -- decoding,
unit conversion, preflight arithmetic, `diagnose` -- stays a free export,
because it is the same whoever deployed the program and whichever token
program the mint belongs to. Two free exports name the token programs
themselves, so configuring the class does not mean hardcoding base58.

```js
const pay = new PayOnChain();                    // canonical, SPL Token
const pay = new PayOnChain(yourProgramId);       // your own deployment
const pay = new PayOnChain().withTokenProgram(   // a Token-2022 mint
  token2022ProgramAddress(),
);
const [approve, open] = pay.approveAndOpen(...);
```

Unlike Rust, the JS side does *not* also carry free functions for the
deployment-dependent calls. A second spelling of every builder earns its place
in Rust, where it keeps an existing API intact for one line of forwarding; in
JS there is nothing yet to keep intact, and the class is where a reader will
look for configuration anyway.

One constraint: `u64` amounts must cross as `BigInt`, not `number`. A JS
number loses precision above 2^53, and while USDC balances will not reach it, a
library that silently truncates is not one an integrator can audit.

The generated `.d.ts` carries four exports that are not part of this surface:
`Pubkey`, `Hash`, `Instruction` and `Instructions`, from `solana-pubkey`,
`solana-instruction` and `solana-hash`. Those crates declare `wasm-bindgen`
and `js-sys` under `cfg(target_arch = "wasm32")`, neither optional nor
feature-gated, so their own bindings appear in any browser build that uses
them. Confirmed against the published manifests 2026-09-01; there is no
setting that removes them.

They are outside the compatibility promise of this package, and this section
is the statement of that: the exported surface is what §6.1 through §6.6
define, and an upstream release may add to, change, or remove the rest without
a version bump here meaning anything about it. Addresses cross this boundary
as base58 strings in both directions, so nothing in the documented API returns
a `Pubkey` or accepts one.

## 7. What the library never does

Signing, signature verification, sign-in message construction, RPC, retries,
storage, routing, rendering, session management. It builds instructions and
decodes bytes. The integrator owns the wallet adapter, the connection, the
session, and the page.

This is the boundary integrators get wrong, so it is stated here and repeated
in the README.

One sentence here is under amendment, and the reason is worth stating rather
than quietly fixing. "The integrator owns the connection" cost nothing for
every consumer that existed when it was written: Rust has
`solana-transaction`, the Node tier has `@solana/web3.js`, and in the browser
the wallet compiles the message and the question never arises. For a PHP
server it means hand-writing a wire encoding, because nothing in sol-pay
compiles a legacy transaction message in any language -- `core::tx` pairs
instructions in the order the program requires, which is ordering and not
wire format. The sentence did not change; the population it applies to did.
When `SolPay\Tx` lands -- decided, not yet built (§3.1) -- "it builds
instructions and decodes bytes" becomes "it builds instructions and the
message that carries them, and decodes bytes" -- and every verb above
survives it unchanged: still no signing, no signature verification, no
sign-in message construction, no RPC, no retries, no storage, no routing, no
rendering, no session management.

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

### 8.1 A second implementation is a second source of drift

`php-client` re-implements this core in another language. The tests above pin
this document's claims against the program; none of them say whether a port
agrees with the crate. That gap matters more than the usual kind, because a
divergent port does not fail cleanly -- it produces a plausible transaction
that does the wrong thing, and then someone signs it.

Conformance vectors are what close it, and they have to begin at **PDA
derivation** rather than at instruction encoding, which is where an argument
about drift naturally reaches first. The predicate `find_program_address` turns
on -- does this 32-byte value decode to a point on the Ed25519 curve -- has no
correct off-the-shelf equivalent in PHP, and the function a developer finds
first is a stricter test that yields a different address about 46% of the time
without raising anything. That is the first primitive, below everything
sol-pay-specific: a port can be wrong there while every layer above it is
right. `php-client/pda-spike/README.md` carries the measurement.

The hazard is not particular to PHP. A crypto library that offers point
validation offers the strict predicate, because strictness is what signature
verification wants; Solana wants only decompressibility. Any future port meets
the same trap, and it will look like the right function.

`php-client/vectors-gen` generates the vectors from the *published*
crate rather than from local source, and covers both layers -- 800 PDAs and one
fully-built `meter_and_settle` with its account list and flags. The
`php conformance` workflow runs them against `php-client/src/Core` on every
change to the port or to the program, on the floor `composer.json` declares as
well as on the version it is developed against; `bin/test-php` is the same
check by hand.

That job is deliberately not `composer test`. The PHPUnit suite hardcodes its
expected values as literals, which is the right shape for naming a local
regression and useless against the crate moving underneath it -- frozen
literals agree with themselves. The two run for different reasons and neither
replaces the other.

**One part of the port has no check at all, and it is the part §8 calls the
point of the exercise.** The preflight predicates are pinned against real
LiteSVM behaviour in `pay-on-chain/tests`; `php-client`'s copies of them are
not, and `Preflight`'s own class doc says so -- arithmetic duplicated from
the program and kept in step by hand. Vectors cannot close this. Preflight
has no chain-serialized bytes to compare, which is why `vectors-gen` emits
none for it, and its PHPUnit tests mirror the Rust `#[cfg(test)]` modules
test-for-test, which checks the port against the port's reading of the
program rather than against the program. The demonstrator will be the first
thing running these predicates against a real devnet, where a disagreement
surfaces as a transaction that fails after preflight said it would not --
the exact failure named above as worse than having no predicate. Closing it
is open work; `php-client/README.md`, "Drift control", carries what is known
about how.
