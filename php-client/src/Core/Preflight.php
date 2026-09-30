<?php

declare(strict_types=1);

namespace SolPay\Core;

/**
 * Will this succeed, and what does the reader still have room for? Every
 * method here mirrors one check the program makes, so a site can ask before
 * it spends a transaction fee finding out. They report facts, not
 * instructions: nothing here decides what to render, redirect to, or block.
 *
 * The arithmetic is duplicated from the program on purpose -- this package
 * cannot call into it -- mirroring wasm-client/src/core/preflight.rs, whose
 * predicates are checked against real LiteSVM behaviour in
 * pay-on-chain/tests. This copy is checked against the same program, one
 * step removed: pay-on-chain/tests/src/test_preflight_fixture.rs records the
 * account bytes at each interesting instant together with what the program
 * then did, and php-client/conformance/preflight.php replays that recording
 * against the methods below. Coverage is exactly the states those recorded
 * cases reach -- see that script's header for what is and is not pinned.
 *
 * Overflow here means "does not fit in PHP_INT_MAX" (~9.2e18), not
 * "does not fit in u64" (~1.8e19) -- PHP has no unsigned 64-bit integer, so
 * this package's safe range is roughly half of the program's. Ordinary
 * token amounts never come close to either ceiling.
 */
final class Preflight
{
    /** What `items` costs at this site's price. Null if it overflows. */
    public static function charge(Site $site, int $items): ?int
    {
        if ($site->itemPrice === 0 || $items === 0) {
            return 0;
        }
        $product = $site->itemPrice * $items;
        if (!is_int($product)) {
            return null;
        }
        return $product;
    }

    /**
     * Mirrors the program's two refusals, in the program's order: Expired
     * when `$now > expiry`, then LimitReached when `used + charge > limit`.
     *
     * `$now` is Unix seconds as the server trusts them. This package has no
     * clock, as it has no RPC; the program reads the cluster's, so a server
     * whose clock is off disagrees near the expiry by exactly that much.
     */
    public static function canMeter(Meter $meter, Site $site, int $items, int $now): ?Blocked
    {
        if ($meter->expired($now)) {
            return Blocked::expired();
        }
        $charge = self::charge($site, $items);
        if ($charge === null) {
            return Blocked::overflow();
        }
        $newUsed = $meter->used + $charge;
        if (!is_int($newUsed)) {
            return Blocked::overflow();
        }
        if ($newUsed > $meter->limit) {
            return Blocked::limitReached($newUsed - $meter->limit);
        }
        return null;
    }

    /**
     * Whether this call would also move money, rather than only accruing
     * usage. Worth knowing because a settling call touches the treasury and
     * the fund's token account, so it is the one that can fail on a low
     * balance.
     */
    public static function willSettle(Meter $meter, Site $site, int $items): bool
    {
        $charge = self::charge($site, $items);
        if ($charge === null) {
            return false;
        }
        $newUsed = $meter->used + $charge;
        if (!is_int($newUsed)) {
            return false;
        }
        return max(0, $newUsed - $meter->paid) >= $site->collectionThreshold;
    }

    /** How many more items fit under the limit. */
    public static function itemsRemaining(Meter $meter, Site $site): int
    {
        if ($site->itemPrice === 0) {
            return 0;
        }
        return intdiv(max(0, $meter->limit - $meter->used), $site->itemPrice);
    }

    /**
     * The smallest limit this reader may authorize right now. One function,
     * not an open-limit and a renewal-limit pair: the question is identical
     * on both screens -- what is the smallest value I can accept here -- and
     * $meter carries state the caller already holds, since looking up
     * the meter either produced one or did not.
     *
     * Renewal has two requirements at once: at or above the site minimum,
     * and covering usage carried forward. `max` is both, and it degenerates
     * to the opening rule when there is no meter.
     */
    public static function limitFloor(Site $site, ?Meter $meter): int
    {
        $carried = $meter?->unpaid() ?? 0;
        return max($site->minLimit, $carried);
    }
}
