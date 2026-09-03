<?php

declare(strict_types=1);

namespace SolPay\Core;

/**
 * The SPL Token errors this flow can actually provoke. Not the whole enum:
 * naming codes sol-pay cannot cause would invite guessing.
 */
enum TokenError: int
{
    /**
     * Code 1. Raised both when the payer's balance is too low *and* when the
     * delegated allowance is too low, which is why {@see Shortfall} exists.
     */
    case InsufficientFunds = 1;

    /** Code 3. The token account is for a different mint than the site's. */
    case MintMismatch = 3;

    /**
     * Code 4. Includes the case where the delegate was cleared: SPL drops
     * the delegate once its allowance reaches zero, and a cleared delegate
     * is no longer an authority at all.
     */
    case OwnerMismatch = 4;

    /** Code 17. */
    case AccountFrozen = 17;

    /** Code 18. Usually a client passing the wrong decimals to approve_checked. */
    case MintDecimalsMismatch = 18;

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
            self::InsufficientFunds => 'Insufficient funds or delegated allowance',
            self::MintMismatch => 'Token account is for a different mint',
            self::OwnerMismatch => 'Wrong owner, or the delegate is no longer set',
            self::AccountFrozen => 'Token account is frozen',
            self::MintDecimalsMismatch => 'Decimals do not match the mint',
        };
    }
}
