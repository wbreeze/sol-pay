<?php

declare(strict_types=1);

namespace SolPay\Core;

/**
 * The Ed25519 "is this y-coordinate on the curve" test that Solana's
 * `Pubkey::is_on_curve` performs -- what `find_program_address` actually
 * needs, and, per php-client/pda-spike/README.md, *not* the same question
 * libsodium's `sodium_crypto_core_ed25519_is_valid_point` answers. That
 * predicate is stricter (canonical encoding, non-small-order, prime-order
 * subgroup membership) and disagrees with `is_on_curve` on roughly half of
 * all 32-byte inputs, which is why this is hand-written rather than
 * delegated to libsodium.
 */
final class Ed25519
{
    /** Big-endian 32-byte encoding of 2^254 - 10. */
    private static function sub2Pow254Minus10(): string
    {
        $b = array_fill(0, 32, 0xFF);
        $b[0] = 0x3F;                 // top byte: 2^254 - 1 has 0x3F as its most significant byte
        // subtract 9 more (we currently hold 2^254 - 1)
        $borrow = 9;
        for ($i = 31; $i >= 0 && $borrow > 0; $i--) {
            $v = $b[$i] - $borrow;
            if ($v < 0) {
                $v += 256;
                $borrow = 1;
            } else {
                $borrow = 0;
            }
            $b[$i] = $v;
        }
        return pack('C*', ...$b);
    }

    /** d = -121665/121666 mod p, as limbs. Computed, not hardcoded. */
    public static function d(): array
    {
        static $d = null;
        if ($d !== null) {
            return $d;
        }
        $num = Fe::sub(Fe::zero(), Fe::fromInt(121665));   // -121665
        $den = Fe::fromInt(121666);
        $d = Fe::mul($num, self::invert($den));
        return $d;
    }

    /** a^(p-2) mod p, by Fermat. */
    public static function invert(array $a): array
    {
        // p - 2 = 2^255 - 21
        $b = array_fill(0, 32, 0xFF);
        $b[0] = 0x7F;
        $borrow = 20;
        for ($i = 31; $i >= 0 && $borrow > 0; $i--) {
            $v = $b[$i] - $borrow;
            if ($v < 0) {
                $v += 256;
                $borrow = 1;
            } else {
                $borrow = 0;
            }
            $b[$i] = $v;
        }
        return Fe::powBytes($a, pack('C*', ...$b));
    }

    /**
     * Solana's Pubkey::is_on_curve: does this 32-byte value decompress to an
     * Edwards point? That is exactly "is u/v a square", where u = y^2 - 1 and
     * v = d*y^2 + 1. Since u/v = u*v / v^2 and v^2 is a square, u/v is a
     * square iff u*v is. One Legendre symbol, one modular exponentiation.
     *
     * Note what is NOT required: prime-order subgroup membership, small-order
     * rejection, or canonical encoding. Those are what libsodium's
     * crypto_core_ed25519_is_valid_point adds on top, and adding them is
     * exactly what makes that predicate the wrong one for this job.
     */
    public static function isOnCurve(string $pointBytes): bool
    {
        $y = Fe::fromBytes($pointBytes);
        $y2 = Fe::sq($y);
        $u = Fe::sub($y2, Fe::fromInt(1));
        $v = Fe::add(Fe::mul(self::d(), $y2), Fe::fromInt(1));

        if (Fe::isZero($v)) {
            return false;             // u/v undefined; dalek fails to decompress
        }
        $uv = Fe::mul($u, $v);
        if (Fe::isZero($uv)) {
            return true;              // x = 0, a valid point
        }
        $chi = Fe::powBytes($uv, self::halfPMinusOneBytes());
        // chi is +1 for a square, p-1 for a non-square
        return Fe::equals($chi, Fe::fromInt(1));
    }

    private static function halfPMinusOneBytes(): string
    {
        static $e = null;
        if ($e === null) {
            $e = self::sub2Pow254Minus10();
        }
        return $e;
    }
}
