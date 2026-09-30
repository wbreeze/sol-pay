<?php

declare(strict_types=1);

namespace SolPay\Core;

/**
 * How much a fund's token account is short of the next settle. Mirrors
 * wasm-client/src/core/error.rs's `shortfall`.
 *
 * This was a struct with three fields under the delegate design, because SPL
 * returned InsufficientFunds for a short balance and a short allowance alike
 * and the two needed opposite responses. There is no allowance now, so a
 * settle refused with SPL error 1 means only that the fund holds less than
 * the unpaid balance, and one number says by how much. A number rather than
 * a verdict: the site decides what to say.
 */
final class Shortfall
{
    private function __construct()
    {
    }

    /** Zero when the fund's token account covers `$unpaid`. */
    public static function of(TokenAccount $fundTokenAccount, int $unpaid): int
    {
        return max(0, $unpaid - $fundTokenAccount->amount);
    }
}
