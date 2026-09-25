# Feedback from the demonstrator

**2026-09-24.** Five items owed to this repository by `sol-pay-demonstrator`
(Newsprint), which SPEC §1 makes the reference integration. Each was checked
against this tree today rather than repeated from a note, and one of them did
not survive that check.

Nothing here changes code. Items 1 and 4 are API decisions.

## 1. `diagnose` answers a question no site has

`wasm-client/src/core/error.rs:201` and
`php-client/src/Core/Shortfall.php:39`:

```rust
delegate_present: account.delegate.is_some(),
```

A token account holds **one** delegate, and `approve` replaces whatever was
there. So after a second site's `approve`, `delegate.is_some()` is true while
the first site can no longer settle at all. The field reports presence; what a
site needs to know is identity.

**`is_clear()` inherits the fault, and that is the sharp end.** It folds
`delegate_present` in (`error.rs:190`, `Shortfall.php:46`) and is documented
as *"nothing on this account would stop a transfer of the amount asked
about."* For a reader whose delegate is another site's contract, it returns
**true**, and the transfer then fails inside SPL's `transfer_checked` with
`OwnerMismatch` (4) rather than `InsufficientFunds` (1).

The doc comment above the field names two ways the delegate goes away — spent
to zero, and an explicit revoke. Replacement by another `approve` is a third,
and it is the only one that leaves a delegate in place.

**Evidence.** Newsprint shipped this bug, by trusting the field. It was
repaired on 2026-09-23 by comparing the field with the site's own contract PDA
— `PayerState::delegateIsContract()`, covered by `PayerStateTest` and
`DelegateWiringTest` — and three screens now act on the answer: the set-meter
screen warns before the wallet dialog, a close sends `close_contract` alone
when the delegate is not the site's, and a refusal says whether the field is
empty or names somebody else.

**The cost of leaving it.** Every integrator has to write that comparison, and
one who does not ships this bug. The symptom is the worst kind: a screen
telling a reader to re-authorize when the approval is fine and the money is
fine.

**Two shapes of fix, and the choice is the library's**: `diagnose` takes the
expected delegate and reports whether the delegate is the caller's, or the
field keeps its meaning and takes a name that says so — `delegate_set` — with
`is_clear` documented as not answering the settle question on its own.

## 2. The README's transaction boundary — closed 2026-09-24

Raised because the payment model called the threshold worthwhile against "a
transfer transaction". `be16745` (2026-09-22) had already corrected that
paragraph, so **the item was discharged before it was delivered**, and
`wasm-client/SPEC.md` §4.6 carries the argument.

Three further places in the README still described a transfer as a transaction
of its own. All three were corrected on 2026-09-24: the happy-path bullets, the
minimum-limit paragraph — where the program in fact *requires*
`min_limit > collection_threshold` and refuses with `MinimumBelowThreshold` —
and the wrap-up paragraph.

The third was wrong about more than the boundary. It had the wrap-up screen
collecting the residue at close, where `close_contract` emits
`Closed { forgiven }` and moves nothing. Discarding the residue is a decision:
collecting at a close is the least likely settle to succeed, so a close
carrying a transfer could fail and leave the payer unable to leave. The README
now records the decision and its reason, and
`pay-on-chain/tests/src/test_metering.rs` already asserted the behaviour —
*"residue below the threshold is forgiven, not collected"*.

**Nothing owed here.** This item also suggested labelling the
treasury-contention argument as unmeasured; §4.6 already does.

## 3. Three sentences for a browser integration note

Measured on the author's machine between 2026-09-07 and 2026-09-10, against
Firefox 155 with Phantom and against Brave, through a diagnostics page that
filters nothing. Nothing in this repository covers any of it; SPEC §5's
mention of registry names is about crates.io and npm.

> Filter wallets by **chain and features, never by name** — one extension
> registers one wallet per network and they share a name.
>
> `wallet.accounts` is empty until the current page connects. After any
> navigation, reconnect (silently first) before assuming you have the account
> you signed in with.
>
> **`window.solana.isPhantom` is not evidence that Phantom is there.** A browser
> with its own wallet can set it while registering only itself, so a site that
> sniffs the injected object will call into the wrong wallet. Ask the Wallet
> Standard registry; the injected object lies for compatibility's sake.

Each of the three cost a session. The first made the site tell a reader
holding a working Phantom that their wallet did not support Sign In With
Solana: Phantom registered three wallets, all named `Phantom`, and
de-duplicating by name left the Sui one, which has no `solana:signIn`.

Every browser integrator meets all three, which is why they belong to the
library rather than to one site's notes.

## 4. A read half for events

The program emits three events
(`pay-on-chain/programs/pay-on-chain/src/lib.rs`, lines 315–335):

```
Metered { contract, page_views, used, paid, transferred }
Renewed { contract, limit, carried }
Closed  { contract, forgiven }
```

**Neither client decodes them.** There is no discriminator handling and no
`Program data:` parsing anywhere in `wasm-client/src` or `php-client/src`. So
an integrator wanting to show what the program said it did has to discover
Anchor's convention — one log line carrying `sha256("event:<Name>")[..8]`,
then Borsh — and write the layouts by hand.

**Available as a donation**: `Newsprint\Chain\ProgramEvent`, about eighty
lines. It derives all three discriminators rather than hard-coding them, so an
upstream rename shows up as a failing assertion instead of a silent mismatch.
It is proven against a real landed devnet transaction (`FKb3eeBw…q5fXi`,
2026-09-09, read back with `getTransaction` and seen in a browser HAR) and was
mutated four ways against synthetic Anchor logs.

It is the read half of a write path this library already owns.

## 5. The item that did not survive checking

The note owed said: *`php-client`'s readers take more than they need — a
reader that accepts decoded state in order to derive an address it could
derive from configuration teaches integrators to serialise two round trips.*

**Checked today: no such reader exists here.** `Pda::contractAddress(string
$site, string $payer, ?string $programId)` takes addresses. `Preflight`'s
methods take `Site` and `Contract` because they are predicates over those
values, which is not the same thing. The class that over-took was Newsprint's
own `PayerReader::read(string $wallet, SiteState $state)`, replaced on
2026-09-10. **The item was owed to the wrong repository.**

What does survive is a documentation gap rather than a defect. Nothing in the
read path says that a site's two reads are **independent**: the contract PDA
derives from the site *address* and the payer's token account from the *mint*,
both of which a site knows from its own configuration, so one
`getMultipleAccounts` serves a whole metered request. Newsprint ran those two
reads in sequence for five days because a function signature implied a
dependency that was never in the data. One sentence in the read-path section
would spare the next integrator the same five days.
