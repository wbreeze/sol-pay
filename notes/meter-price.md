# To investigate: should a meter record its price?

**2026-10-07.** Raised from `sol-pay-demonstrator` (Newsprint) on 2026-10-02
and deferred there. Recorded here because the question belongs to the
program. Nothing in this note changes code.

## How the price works today

Checked against this tree on 2026-10-07.

- The price is `Site.item_price` (`pay-on-chain/programs/pay-on-chain/src/state.rs`).
  `initialize_site` writes the price, and no instruction changes a site
  account afterwards.
- `meter_and_settle` reads `site.item_price` on every call. A `Meter` holds a
  key, an expiry, a limit, `used` and `paid`. A meter holds no price.
- A `Site` is seeded by `["site", authority]`. So a new price is a new
  authority key and a second site account. Readers move to the second site as
  their meters on the first expire.

## Question 1: a price on the meter

The author's view, 2026-10-02: the price on the site account should **not**
become mutable. The price would change underneath a reader's open meter, and
the reader agreed to a limit at the old price.

If the price were ever to change in place, the meter would have to carry its
own copy, written by `open_meter` and rewritten by `renew_meter`. The meter
would then charge at the price the reader accepted until the reader renews.

To weigh:

- the meter account grows by eight bytes, and the reader pays that rent;
- `meter_and_settle` would read the price from the meter, and the site's
  price would become the price of the *next* meter;
- `renew_meter` becomes the moment a reader accepts a new price, so a client
  would have to show the new price before the wallet signs;
- preflight arithmetic in both clients (`preflight.rs`, `Preflight.php`)
  takes the price from the site today;
- an instruction that changes a site's price would be the first instruction
  that changes a site account at all.

## Question 2: A-B pricing

A site may want to try two prices at once. Today that is two site accounts
under two authority keys, and the server signs with the key that matches the
meter. Whether the program should make two prices easier is open. A price on
the meter (question 1) would allow it with one site account, since each meter
would carry the price it was opened at.

## What the demonstrator has not run

The demonstrator's article *What the limit promises* describes the rolling
move to a second site account from the program's rules. Nobody has run a
price change on devnet.
