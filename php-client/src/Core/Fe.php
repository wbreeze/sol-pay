<?php

declare(strict_types=1);

namespace SolPay\Core;

/**
 * Field arithmetic mod p = 2^255 - 19. Pure PHP: no ext-gmp, no ext-bcmath,
 * no Composer package. Promoted from php-client/pda-spike/php/Curve25519.php,
 * which proved this arithmetic byte-for-byte against the published
 * sol-pay-client crate -- see that directory's README for the measurements.
 * This copy is the one the package actually depends on; the spike's copy
 * stays frozen as the record of that experiment.
 *
 * Representation: 15 limbs of 17 bits (15 * 17 = 255). A limb product is 34
 * bits and an accumulation of 15 of them is under 39 bits, so everything
 * stays far inside PHP's signed 64-bit int. Limb 0 absorbs the fold-back of
 * limb weight 2^255, which is congruent to 19 mod p.
 */
final class Fe
{
    public const N = 15;
    public const MASK = 0x1FFFF;      // 2^17 - 1

    /** p = 2^255 - 19 as limbs: all 0x1FFFF except limb 0, which is 0x1FFFF - 18. */
    public static function p(): array
    {
        $p = array_fill(0, self::N, self::MASK);
        $p[0] = self::MASK - 18;
        return $p;
    }

    public static function zero(): array
    {
        return array_fill(0, self::N, 0);
    }

    public static function fromInt(int $v): array
    {
        $r = self::zero();
        for ($i = 0; $i < self::N && $v !== 0; $i++) {
            $r[$i] = $v & self::MASK;
            $v >>= 17;
        }
        return $r;
    }

    /**
     * 32 little-endian bytes to a field element. The top bit is the sign bit
     * in an Ed25519 compressed point and is masked off, matching dalek's
     * FieldElement::from_bytes. A non-canonical value (y >= p) is reduced
     * rather than rejected, which is also what dalek does.
     */
    public static function fromBytes(string $b): array
    {
        $bits = 0;
        $acc = 0;
        $limbs = self::zero();
        $li = 0;
        for ($i = 0; $i < 32; $i++) {
            $byte = ord($b[$i]);
            if ($i === 31) {
                $byte &= 0x7F;              // drop the sign bit
            }
            $acc |= $byte << $bits;
            $bits += 8;
            while ($bits >= 17 && $li < self::N) {
                $limbs[$li++] = $acc & self::MASK;
                $acc >>= 17;
                $bits -= 17;
            }
        }
        if ($li < self::N) {
            $limbs[$li] = $acc & self::MASK;
        }
        return self::canonical($limbs);
    }

    /** Propagate carries so every limb is in [0, 2^17). */
    public static function carry(array $t): array
    {
        for ($pass = 0; $pass < 3; $pass++) {
            $c = 0;
            for ($i = 0; $i < self::N; $i++) {
                $t[$i] += $c;
                $c = $t[$i] >> 17;
                $t[$i] &= self::MASK;
            }
            // limb 15 has weight 2^255 == 19
            $t[0] += 19 * $c;
            if ($t[0] <= self::MASK) {
                break;
            }
        }
        return $t;
    }

    /** Fully reduce into [0, p). */
    public static function canonical(array $t): array
    {
        $t = self::carry($t);
        // conditionally subtract p, twice, to land inside [0, p)
        for ($k = 0; $k < 2; $k++) {
            $p = self::p();
            $borrow = 0;
            $d = self::zero();
            for ($i = 0; $i < self::N; $i++) {
                $v = $t[$i] - $p[$i] - $borrow;
                if ($v < 0) {
                    $v += 0x20000;
                    $borrow = 1;
                } else {
                    $borrow = 0;
                }
                $d[$i] = $v;
            }
            if ($borrow === 0) {
                $t = $d;
            }
        }
        return $t;
    }

    public static function mul(array $a, array $b): array
    {
        $t = array_fill(0, 2 * self::N, 0);
        for ($i = 0; $i < self::N; $i++) {
            $ai = $a[$i];
            if ($ai === 0) {
                continue;
            }
            for ($j = 0; $j < self::N; $j++) {
                $t[$i + $j] += $ai * $b[$j];
            }
        }
        // fold limbs 15..29 down by 15 positions with weight 19
        for ($k = 2 * self::N - 1; $k >= self::N; $k--) {
            $t[$k - self::N] += 19 * $t[$k];
            $t[$k] = 0;
        }
        $low = array_slice($t, 0, self::N);
        return self::carry($low);
    }

    public static function sq(array $a): array
    {
        return self::mul($a, $a);
    }

    public static function add(array $a, array $b): array
    {
        $r = self::zero();
        for ($i = 0; $i < self::N; $i++) {
            $r[$i] = $a[$i] + $b[$i];
        }
        return self::carry($r);
    }

    public static function sub(array $a, array $b): array
    {
        // a - b + 2p, then carry; keeps every limb non-negative
        $p = self::p();
        $r = self::zero();
        for ($i = 0; $i < self::N; $i++) {
            $r[$i] = $a[$i] - $b[$i] + 2 * $p[$i];
        }
        return self::canonical($r);
    }

    /** Big-endian hex, for tests and vector comparison. */
    public static function toHex(array $a): string
    {
        $a = self::canonical($a);
        $bytes = array_fill(0, 32, 0);
        $acc = 0;
        $bits = 0;
        $bi = 0;
        for ($i = 0; $i < self::N; $i++) {
            $acc |= $a[$i] << $bits;
            $bits += 17;
            while ($bits >= 8 && $bi < 32) {
                $bytes[$bi++] = $acc & 0xFF;
                $acc >>= 8;
                $bits -= 8;
            }
        }
        if ($bi < 32) {
            $bytes[$bi] = $acc & 0xFF;   // flush the trailing 7 bits
        }
        return strtoupper(bin2hex(pack('C*', ...array_reverse($bytes))));
    }

    public static function isZero(array $a): bool
    {
        $a = self::canonical($a);
        for ($i = 0; $i < self::N; $i++) {
            if ($a[$i] !== 0) {
                return false;
            }
        }
        return true;
    }

    public static function equals(array $a, array $b): bool
    {
        return self::canonical($a) === self::canonical($b);
    }

    /** a^e mod p, e given as a big-endian byte string. */
    public static function powBytes(array $a, string $e): array
    {
        $r = self::fromInt(1);
        $len = strlen($e);
        $started = false;
        for ($i = 0; $i < $len; $i++) {
            $byte = ord($e[$i]);
            for ($bit = 7; $bit >= 0; $bit--) {
                if ($started) {
                    $r = self::sq($r);
                }
                if (($byte >> $bit) & 1) {
                    $r = $started ? self::mul($r, $a) : $a;
                    $started = true;
                }
            }
        }
        return $started ? $r : self::fromInt(1);
    }
}
