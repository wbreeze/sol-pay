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
    case ZeroPagePrice = 6002;
    case LimitReached = 6003;
    case DelegateNotSet = 6004;
    case DelegateMismatch = 6005;
    case DelegateAllowanceTooLow = 6006;
    case LimitBelowUsage = 6007;
    case MathOverflow = 6008;

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
            self::ZeroPagePrice => 'Page price must be greater than zero',
            self::LimitReached => 'Charge would carry usage past the authorized limit',
            self::DelegateNotSet => 'Payer token account names no delegate',
            self::DelegateMismatch => 'Payer token account delegates a different authority',
            self::DelegateAllowanceTooLow => 'Delegated allowance does not cover the outstanding limit',
            self::LimitBelowUsage => 'New limit does not cover usage already accrued',
            self::MathOverflow => 'Arithmetic overflow',
        };
    }
}
