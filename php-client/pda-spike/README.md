# PDA spike — can PHP derive Solana program addresses correctly?

Run 2026-09-03. **Answer: yes, and the shortcut everyone reaches for first is
wrong roughly half the time.**

This settles the one unproven thing in the PHP route of §12.1 of the
*demonstrator's* `SPEC.md` -- `sol-pay-demonstrator`, a separate repository;
`wasm-client/SPEC.md` in this one ends at §8. The cross-check against the
published crate (below) has since closed the one gap the initial run left
open.

## What was in question

Solana's `find_program_address` walks a bump seed downward and takes the first
candidate that is **not a point on the Ed25519 curve**. Solana's test is
`curve25519_dalek`'s decompression: does this 32-byte value decode to a curve
point at all. Nothing more.

That property is the whole guarantee. An address off the curve is one no
private key can correspond to, which is what lets a program own it and what
makes a signature for it impossible. So the predicate is not an implementation
detail of the search — it *is* the definition of a program-derived address. A
port that gets it wrong is not deriving slightly different addresses; it is
producing values that are not PDAs at all.

The obvious PHP equivalent is libsodium's
`sodium_crypto_core_ed25519_is_valid_point`. It is a **stricter** test:
[libsodium documents it](https://doc.libsodium.org/advanced/point-arithmetic) as
checking that a value "represents a point on the edwards25519 curve, in
canonical form, on the main subgroup, and that the point doesn't have a small
order." Those extra conditions are exactly what verifying a signing key wants,
and exactly wrong for finding a PDA. If the two disagree
anywhere, the derived address is silently wrong and the transaction fails, or
worse, addresses something else.

## Results

**1. The strict predicate is not even available.** Confirmed on two builds: a
stock Ubuntu 24.04 / PHP 8.4 with libsodium 1.0.18, and separately a macOS
Homebrew PHP 8.4.25 with libsodium 1.0.22. Both expose the ristretto255 core
API and **none of the ed25519 core functions**.
`sodium_crypto_core_ed25519_is_valid_point` does not exist on either, so this
is not a libsodium-version fact — it is PHP's `sodium` extension not exposing
the ed25519 core API regardless of the libsodium version underneath. What a
developer finds instead is `sodium_crypto_sign_ed25519_pk_to_curve25519`,
which is stricter in the same ways. So there is no shortcut to take; the field
arithmetic has to be written.

Worth recording alongside: the same page notes that `is_valid_point` accepted
points outside the main subgroup in **1.0.20 and earlier**. There are
therefore three predicates in circulation, not two, and the two that are not
Solana's are wrong by different amounts. This does not affect the measurement
below — neither build exposes `is_valid_point` at all — but a port that does
find the function on some future build should not assume one behaviour from it.

**2. Written, it works — 4 ms per derivation, no extensions.**
Pure PHP, 15 limbs of 17 bits, mod 2^255-19. No ext-gmp, no ext-bcmath, no
Composer package, nothing beyond `hash`. That matters for shared hosting, which
is the whole point of the PHP route.

**3. The shortcut would have been wrong 46% of the time.**

```
strict predicate in use: sodium_crypto_sign_ed25519_pk_to_curve25519 (core ed25519 API absent in this build)
samples                                    4000
on curve (Solana is_on_curve)              1998  (50.0%)   <- theory: 50%
valid point (libsodium, strict)             220  ( 5.5%)   <- theory: ~6.25%
disagreements                              1778  (44.5%)
invariant violations (valid && !on-curve)     0             <- must be 0

site PDAs differing:      185 / 400  (46.2%)
contract PDAs differing:  185 / 400  (46.2%)
```

The first line is `t2.php` reporting which function it actually called: since
`is_valid_point` is absent, the strict column is `pk_to_curve25519`, which is
what a developer on either of these builds would end up using.

Not an edge case. Nearly half of all derivations. Reproduced on a second
machine (macOS, libsodium 1.0.22) with a fresh 4,000-sample run: 46.2%
disagreement again, same as above.

**The direction matters.** The strict predicate accepts only 5.5% of values, so
a port using it rejects *fewer* candidates than Solana does and stops earlier in
the bump walk — returning a higher bump, and an address Solana rejected
precisely because it decodes to a curve point. The failure mode is not "an
address that doesn't work". It is an address that violates the one property a
program-derived address is defined by, produced by code that raises no error.

## How the implementation is known to be right

Three checks, none of which needed network access:

- **The curve constant is derived, not copied.** `d` is computed as
  `-121665/121666 mod p` using the same `mul`, `sub` and modular-inverse code
  the curve test uses, and it reproduces the published `EDWARDS_D`
  (`52036CEE…35978A3`) exactly. A single wrong bit anywhere in the field
  arithmetic would break this.
- **A one-directional invariant across 4,200 samples.** libsodium's predicate is
  strictly stronger, so *valid ⟹ on-curve* must always hold. Zero violations.
- **The measured on-curve rate is 50.0%**, which is what the mathematics
  predicts for random 32-byte values, and 200 genuine Ed25519 public keys all
  test on-curve as they must.

## Cross-checked against the sol-pay crate

The spike originally ran in a sandbox where crates.io, npm, PyPI and
Packagist were all blocked by egress policy, so the Rust reference could not
be built there and the checks above were strong evidence rather than proof of
byte-for-byte agreement with `Pubkey::find_program_address`.

That gap is now closed, run the same day on a machine with crates.io access:

From `php-client/pda-spike/`:

```
( cd ../vectors-gen && cargo run --release > vectors.json )
php php/verify.php
```

```
site       400/400 match
contract   400/400 match
ix data    match (8b11008b72e9587907000000)
ix accts   8 accounts, order/flags recorded in the vector file
```

The generator pulls `sol-pay-client` `0.1.1` from crates.io — the published
artifact, not local source — and derives its inputs as
`sha256("authority-<i>")` and `sha256("payer-<i>")`, which the PHP side
reproduces, so nothing but the expected outputs crosses between them. It also
emits one fully-built `meter_and_settle` instruction — discriminator,
argument encoding, and the account list with its signer and writable flags —
so the same run checked the instruction layer too. All 800 PDAs and the
instruction encoding matched.

## Files

| | |
| --- | --- |
| `php/Curve25519.php` | field arithmetic mod 2^255-19 and the on-curve test |
| `php/Base58.php` | base58 with no bignum dependency |
| `php/Pda.php` | `findProgramAddress`, plus the strict predicate for comparison |
| `php/t1.php` | field-arithmetic self-tests |
| `php/t2.php` | the comparison experiment above |
| `php/bench.php` | timing |
| `php/verify.php` | checks against Rust-generated vectors |

The generator that produces those vectors was `vectors-gen/` here when this
experiment ran. It has since moved up to `php-client/vectors-gen`, because
the package's own `conformance/vectors.php` and `wasm-client`'s Node
conformance check read the same file and it is no longer the spike's — see
CLAUDE.md's php-client section for what it emits and why each piece is
sourced from the crate or the program rather than transcribed.

## What this means for the demonstrator's §12.1

The PHP route's one unproven thing is now proven, cross-checked byte-for-byte
against the published `sol-pay-client` crate. It costs 280 lines of field
arithmetic (`php/Curve25519.php`) that never needs to change, and it runs in
4 ms with no extension beyond what every PHP install has.

Two findings worth carrying into a decision that were not in the spec:

- **Anyone writing a PHP Solana integration who reaches for libsodium's point
  validation gets wrong addresses half the time, silently.** That is a good
  reason for this demo to exist, and the reason it is written up in full above
  rather than left as a number.
- **Anchor discriminators are trivial in PHP** — the first 8 bytes of
  `sha256("global:<instruction_name>")` for instructions and of
  `sha256("account:<StructName>")` for accounts. Two different prefixes; one
  `hash()` call either way, no curve arithmetic, no room to diverge. For the
  record: `meter_and_settle` `8B11008B72E95879`, `open_contract`
  `7C3EC091C05A3BD3`, `renew_contract` `7DE4C69AB0EF8C90`, `close_contract`
  `25F422A85CCA506A`, `initialize_site` `553480D007E0B24F`, `Site`
  `8FFF340F41A55E31`, `Contract` `AC8A73F27943B71A`. These are computed, and
  the `verify.php` run confirms them against the crate.
