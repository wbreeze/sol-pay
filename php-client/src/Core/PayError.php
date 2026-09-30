<?php

declare(strict_types=1);

namespace SolPay\Core;

/**
 * This program's errors. Backed by their actual Anchor code -- 6000 plus
 * declaration order, per pay-on-chain/programs/pay-on-chain/src/errors.rs
 * -- so `code()` and `fromCode()` need no offset arithmetic.
 *
 * A bare code never says which program raised it: SPL Token numbers from 0
 * and shares low numbers with this program (see {@see TokenError}). Naming
 * one means knowing which program's logs it came from; see {@see Cause}.
 */
enum PayError: int
{
    case LimitBelowMinimum = 6000;
    case MinimumBelowThreshold = 6001;
    case ZeroItemPrice = 6002;
    case LimitReached = 6003;
    case LimitBelowUsage = 6004;
    case MathOverflow = 6005;
    case MintMismatch = 6006;
    case Expired = 6007;
    case ExpiryInPast = 6008;
    case Unauthorized = 6009;
    case FundNotEmpty = 6010;
    case FundHasMeters = 6011;

    public static function fromCode(int $code): ?self
    {
        return self::tryFrom($code);
    }

    public function code(): int
    {
        return $this->value;
    }

    public function message(): string
    {
        return match ($this) {
            self::LimitBelowMinimum => 'Limit is below the site minimum',
            self::MinimumBelowThreshold => 'Site minimum limit must exceed the collection threshold',
            self::ZeroItemPrice => 'Item price must be greater than zero',
            self::LimitReached => 'Charge would carry usage past the authorized limit',
            self::LimitBelowUsage => 'New limit does not cover usage already accrued',
            self::MathOverflow => 'Arithmetic overflow',
            self::MintMismatch => 'The site and the fund are in different mints',
            self::Expired => 'The meter is past its expiry',
            self::ExpiryInPast => 'The expiry has already passed',
            self::Unauthorized => "Signer is neither the fund's reader nor the meter's key",
            self::FundNotEmpty => 'The fund still holds a balance',
            self::FundHasMeters => 'The fund still has meters open',
        };
    }
}
