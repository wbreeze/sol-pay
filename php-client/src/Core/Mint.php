<?php

declare(strict_types=1);

namespace SolPay\Core;

/**
 * The mint's decimals, one byte at a fixed offset. Needed to interpret raw
 * token amounts at their real scale, and there is nowhere else to get it
 * without decoding a mint by hand. Accepts anything at least mint-sized, so
 * a Token-2022 mint carrying extensions decodes too.
 */
final class Mint
{
    private const MIN_LEN = 82;
    private const DECIMALS_OFFSET = 44;

    public static function decimals(string $mintAccountData): int
    {
        if (strlen($mintAccountData) < self::MIN_LEN) {
            throw DecodeException::wrongLength(self::MIN_LEN, strlen($mintAccountData));
        }
        return ord($mintAccountData[self::DECIMALS_OFFSET]);
    }
}
