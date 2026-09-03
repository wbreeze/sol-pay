<?php

declare(strict_types=1);

namespace SolPay\Core;

/**
 * Will this succeed, and what does the payer still have room for? Every
 * method here mirrors one check the program makes, so a site can ask before
 * it spends a transaction fee finding out. They report facts, not
 * instructions: nothing here decides what to render, redirect to, or block.
 *
 * The arithmetic is duplicated from the program on purpose -- this package
 * cannot call into it -- mirroring wasm-client/src/core/preflight.rs, whose
 * predicates are checked against real LiteSVM behaviour in
 * pay-on-chain/tests. This PHP copy has no such check yet and must be kept
 * in step by hand until one exists.
 *
 * Overflow here means "does not fit in PHP_INT_MAX" (~9.2e18), not
 * "does not fit in u64" (~1.8e19) -- PHP has no unsigned 64-bit integer, so
 * this package's safe range is roughly half of the program's. Ordinary
 * token amounts never come close to either ceiling.
 */
final class Preflight
{
    /** What `pageViews` costs at this site's price. Null if it overflows. */
    public static function charge(Site $site, int $pageViews): ?int
    {
        if ($site->pagePrice === 0 || $pageViews === 0) {
            return 0;
        }
        $product = $site->pagePrice * $pageViews;
        if (!is_int($product)) {
            return null;
        }
        return $product;
    }

    /** Mirrors `require!(new_used <= limit, LimitReached)`. */
    public static function canMeter(Contract $contract, Site $site, int $pageViews): ?Blocked
    {
        $charge = self::charge($site, $pageViews);
        if ($charge === null) {
            return Blocked::overflow();
        }
        $newUsed = $contract->used + $charge;
        if (!is_int($newUsed)) {
            return Blocked::overflow();
        }
        if ($newUsed > $contract->limit) {
            return Blocked::limitReached($newUsed - $contract->limit);
        }
        return null;
    }

    /**
     * Whether this call would also move money, rather than only accruing
     * usage. Worth knowing because a settling call touches the treasury and
     * the payer's token account, so it is the one that can fail on a low
     * balance.
     */
    public static function willSettle(Contract $contract, Site $site, int $pageViews): bool
    {
        $charge = self::charge($site, $pageViews);
        if ($charge === null) {
            return false;
        }
        $newUsed = $contract->used + $charge;
        if (!is_int($newUsed)) {
            return false;
        }
        return max(0, $newUsed - $contract->paid) >= $site->collectionThreshold;
    }

    /** How many more views fit under the limit. */
    public static function viewsRemaining(Contract $contract, Site $site): int
    {
        if ($site->pagePrice === 0) {
            return 0;
        }
        return intdiv(max(0, $contract->limit - $contract->used), $site->pagePrice);
    }

    /**
     * The smallest limit this payer may authorize right now. One function,
     * not an open-limit and a renewal-limit pair: the question is identical
     * on both screens -- what is the smallest value I can accept here -- and
     * $contract carries state the caller already holds, since looking up
     * the contract either produced one or did not.
     *
     * Renewal has two requirements at once: at or above the site minimum,
     * and covering usage carried forward. `max` is both, and it degenerates
     * to the opening rule when there is no contract.
     */
    public static function limitFloor(Site $site, ?Contract $contract): int
    {
        $carried = $contract?->unpaid() ?? 0;
        return max($site->minLimit, $carried);
    }

    /**
     * What the SPL approval must cover for a contract at $limit: the whole
     * limit. Nothing is paid against a new limit yet, and the program checks
     * the allowance against it at open and at renew.
     */
    public static function requiredAllowance(int $limit): int
    {
        return $limit;
    }
}
