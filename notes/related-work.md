# Related work

Surveyed 2026-09-28. What else charges per unit of use for content or calls,
and how it settles. Companion to [`x402.md`](x402.md), which takes one entry
from this survey at length; x402 appears below only in its place in the
landscape.

**Provenance, stated once.** This was gathered by delegated web search across
four bands, then two claims were checked by hand. It is docs-page reading, not
code reading, and it inherits that repository rule: nothing here is a claim
about any of these systems' behaviour, only about what their documents say.
Entries carry a marker where a claim is unverified. Everything not marked was
read off the linked page on the date above.

## The box

A survey without its exclusion criteria cannot be audited, so:

**In.** Anything charging per unit of *use* rather than per period, for digital
content, API calls or inference, that either (a) removes the payer from the
per-charge path, or (b) competes at the HTTP level. Must have a located
specification, EIP, or open source implementation.

**Out.** General payment processors, subscription billing, tipping, token
gating, DePIN, payment L2s, and anything evidenced only by a funding
announcement. Excluded under that rule during the run: Tollbit, PayAI, and
several agent-wallet startups -- product pages, no protocol.

**Bands.** HTTP-level protocols; on-chain absent-payer spend primitives; the
publisher pay-per-article commercial record; streaming rails and API/inference
metering.

**Blind spots, named so a later pass knows where to start.** Axate, Piano,
Poool, Steady, Wallkit, Pico, and any Japanese or Korean per-article model came
back unresolved -- searched, no clear verdict, excluded rather than guessed.
Two IETF drafts were found and not read: `draft-ryan-httpauth-payment-01` and
`draft-hope-bailie-http-payments-00`. The search budget was about fourteen
calls per band and three of four bands reached it, so absence below is weak
evidence of absence.

## The axis that sorts everything

Every candidate falls into one of five settlement shapes. The distinction that
matters is not the chain, the token or the protocol layer. It is **who signs at
charge time** and **when value moves**.

| shape | signs each charge | who does this |
| --- | --- | --- |
| transfer per charge | payer, every time | x402 `exact`, L402 |
| transfer per charge | merchant, payer absent | Coinbase Spend Permissions, `solana-program/subscriptions`, ERC-7715 |
| continuous by elapsed time | nobody, after opening | Superfluid, Sablier, Streamflow, Zebec |
| draw down a prepaid ledger | provider, off chain | Skyfire, Nevermined, Blendle, Google Contributor |
| **accrue on chain, settle at a threshold** | **site authority, payer absent** | **this program**, and one fiat precedent |

Nothing else found occupies the last row. The absent-payer group gets the hard
part right -- a cap the merchant draws against without the payer present -- and
then transfers on every charge. The streaming group pre-funds a known total and
releases it by clock rather than by use, which is a different product and not
an alternative for metered page views.

## The three findings

### 1. The nearest mechanism is on this chain, and it may collide

`solana-program/subscriptions`, program id
`De1egAFMkMWZSN5rYXRj9CAdheBamobVNubTsi9avR44`
(https://solana.com/docs/payments/subscriptions/overview). Three authorization
modes: fixed delegation up to a total with optional expiry, recurring
delegation resetting per period, and merchant-published billing terms that
approved collectors charge against. Absent-payer pull under a cap, same chain,
same intent.

Two differences, both to be confirmed against the program rather than its docs,
whose prose is thin on exactly these points:

- It appears to transfer on every charge, with no accrual and no threshold.
- The payer approves a **single program-controlled Subscription Authority**
  rather than a per-merchant delegate, with separate authorization records
  checked against it.

**If the second is right there is an interop collision, and it is the reasoning
of README's "Can a reader be metered by more than one site at once?" arriving
from a new direction.** One token account has one delegate. A reader who has
approved that Subscription Authority on a token account cannot also have a
sol-pay meter PDA delegated on it. The remedy is the one already documented
-- a second token account -- but the question changes from coexisting with a
second sol-pay site to coexisting with Solana's own primitive, which more
readers will meet first. This is reasoning from a documented constraint, not a
tested result.

*Unverified:* a mainnet date of 2026-06-02 and an attribution to Moonsong Labs
with the Solana Foundation surfaced in search and could not be confirmed from
the docs page. *Corrected during the survey:* it is not a native runtime
program, as a first pass reported; it is a deployed program under the
`solana-program` organization.

### 2. The collection threshold has exactly one precedent, and it is in fiat

LaterPay, now Supertab (https://www.supertab.co/), ran a deferred tab: small
purchases accumulate and the reader's payment method is charged only once the
running total crosses a threshold. Structurally §4.6.

The useful part is the divergence, not the resemblance. Their reason is fee
amortization -- card fees are per transaction, so a ten-cent charge cannot
stand alone. §4.6 **rejects fee amortization as the reason here**, because the
transfer is a CPI inside the instruction that raises `used`, and gives three
different ones: treasury write contention, keeping most calls unable to fail,
and per-transfer Token-2022 costs. Two designs reaching the same structure from
unrelated constraints is the strongest evidence the structure is sound that a
survey can produce.

*Unverified:* the threshold amount, the currency, and whether the deferred tab
survived the LaterPay-to-Supertab transition. The marketing site says nothing
mechanical; the answer would be at `connect-docs.supertab.co`.

Worth recording beside it: **Supertab is the only one of nine publisher
ventures surveyed that is still operating.**

### 3. The graveyard died of demand, and it carries a number about ours

Blendle: over a million registered readers in the Netherlands, of whom roughly
**150,000 ever made a paid micropayment**, at $0.25--$0.49 per article. The
micropayment model closed in the Netherlands in 2019 and in Germany and the US
in 2023, the stated cause being a very limited base actually converting, with
readers preferring unlimited bundles (Nieman Lab, August 2023).

The demonstrator's `content/MeteredPayEconomics.md` needs about one page view
in five paid for, at about a dime. Blendle's fraction is
registrations-to-ever-transacted, a different denominator, so it does not
refute that figure. **It is nevertheless the closest evidenced number anyone
has produced for the assumption the economics rests on, and it points the wrong
way.** Recorded here rather than argued with.

Two more from the same band:

- **Satoshipay** abandoned the Bitcoin blockchain in 2017 because on-chain fees
  and confirmation times made micropayments uneconomic (CoinDesk, July 2017).
  That is the argument for a cheap chain, made by someone who lost to its
  absence.
- **Coil** and **Scroll** both ended by reorg and acquisition rather than by
  admitted demand failure; **Flattr** and **Google Contributor** shut with no
  stated cause at all, Contributor twice. A shutdown without a postmortem is
  not evidence the model failed, and should not be read as evidence it worked
  either.

## What the survey does not answer

Nobody found is metering web page views for human readers with an absent payer.
The absent-payer primitives are built for subscriptions and for agents; the
HTTP protocols assume the client is software holding a key. That is either an
unoccupied position or a market that is not there, and a survey cannot tell
those apart. It is the same open question `MeteredPayEconomics.md` already
names, now with the observation that no one else has bet on the answer either.

## Entries, by band

**HTTP-level.** x402 (see [`x402.md`](x402.md)) · L402/Lightning Labs,
https://github.com/lightninglabs/L402, per-request macaroon plus paid invoice,
payer active every time · Interledger Open Payments,
https://github.com/interledger/open-payments, GNAP grant then client-initiated
payments without further payer signing, off-chain ILP rails, SDKs including PHP
· Web Monetization, https://webmonetization.org/specification/, browser-mediated
streaming, self-described work in progress and not on the W3C standards track ·
AP2, https://github.com/google-agentic-commerce/AP2, signed mandates over any
instrument, no settlement mechanism of its own · ACP,
https://github.com/agentic-commerce-protocol/agentic-commerce-protocol, agent
checkout over card rails · A402, https://arxiv.org/abs/2603.01179, TEE-mediated
atomic service channels with aggregated settlement, paper only, no
implementation found.

**Absent-payer spend primitives.** Coinbase Spend Permissions,
https://github.com/coinbase/spend-permissions, recurring cap, merchant-initiated
`spend`, transfers immediately · ERC-7715,
https://eips.ethereum.org/EIPS/eip-7715, the general permission-grant framework
this pattern is an instance of, still Draft · ZeroDev session keys, rate limits
on call frequency rather than on amount · Superfluid ACL, operator-initiated
flows, streaming rather than metering · `solana-program/subscriptions`, above.

**Streaming.** Superfluid, Sablier, Streamflow, Zebec. All pre-fund a known
total and release it by elapsed time. Streamflow and Zebec are Solana-native
and non-custodial, so they are the closest in shape and still answer a
different question.

**Metering and billing infrastructure.** Skyfire, https://docs.skyfire.xyz/,
prepaid agent budget drawn per call against a custodial ledger · Nevermined,
https://nevermined.ai/, prepaid credit plans metered per call, settles through
x402. Both put a third party in the path.

**Publisher record.** Blendle (dead, 2019/2023) · Coil (dead 2023, handed to the
Interledger Foundation) · Scroll (acquired 2021, folded into Twitter Blue
within thirty days) · Google Contributor (dead 2017, twice, no stated cause) ·
Flattr (dead November 2023, no stated cause) · Satoshipay (status unresolved,
last evidenced event a 2017 chain pivot) · Brave/BAT (operating; no publisher
or reader scale figure evidenced) · Supertab/LaterPay (operating).
