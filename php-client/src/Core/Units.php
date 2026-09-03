<?php

declare(strict_types=1);

namespace SolPay\Core;

/**
 * Human amounts to mint base units and back. Every amount this package
 * takes elsewhere is in base units, and USDC has six decimals: an
 * integrator who scales twice turns an intended 50 USDC into 50,000,000 of
 * allowance, and nothing rejects it downstream. Owning the conversion
 * removes that error class; mirrors wasm-client/src/core/units.rs.
 *
 * This package's safe integer range is PHP_INT_MAX (~9.2e18), roughly half
 * of u64::MAX (~1.8e19) -- see {@see Preflight}'s class doc for the same
 * boundary. `toBaseUnits` throws rather than silently wrapping past it.
 */
final class Units
{
    /**
     * Parse a decimal amount into base units at a mint's scale. Accepts
     * "12", "12.5", "0.000001", ".5", "12.". Rejects anything with a sign,
     * an exponent, separators, or more precision than the mint can hold --
     * rounding someone's money down is not this function's business.
     */
    public static function toBaseUnits(string $amount, int $decimals): int
    {
        $amount = trim($amount);
        if ($amount === '') {
            throw UnitsException::empty();
        }

        if (str_contains($amount, '.')) {
            [$whole, $frac] = explode('.', $amount, 2);
            if (str_contains($frac, '.')) {
                throw UnitsException::notANumber();
            }
        } else {
            $whole = $amount;
            $frac = '';
        }
        if ($whole === '' && $frac === '') {
            throw UnitsException::notANumber();
        }
        if (($whole !== '' && !ctype_digit($whole)) || ($frac !== '' && !ctype_digit($frac))) {
            throw UnitsException::notANumber();
        }

        if (strlen($frac) > $decimals) {
            throw UnitsException::tooPrecise($decimals, strlen($frac));
        }

        $units = 0;
        foreach (str_split($whole.$frac) as $digit) {
            $units = $units * 10 + (int) $digit;
            if (!is_int($units)) {
                throw UnitsException::overflow();
            }
        }
        // Pad the fraction out to the mint's scale.
        for ($i = 0, $pad = $decimals - strlen($frac); $i < $pad; $i++) {
            $units *= 10;
            if (!is_int($units)) {
                throw UnitsException::overflow();
            }
        }
        return $units;
    }

    /**
     * Render base units as a decimal string, without trailing zeros.
     * `fromBaseUnits(1_500_000, 6)` is "1.5", not "1.500000", and
     * `fromBaseUnits(1_000_000, 6)` is "1". Every output round-trips back
     * through `toBaseUnits` at the same scale.
     */
    public static function fromBaseUnits(int $units, int $decimals): string
    {
        if ($decimals === 0) {
            return (string) $units;
        }
        $scale = 10 ** $decimals;
        $whole = intdiv($units, $scale);
        $frac = $units % $scale;
        if ($frac === 0) {
            return (string) $whole;
        }
        $fracStr = rtrim(str_pad((string) $frac, $decimals, '0', STR_PAD_LEFT), '0');
        return "{$whole}.{$fracStr}";
    }
}
