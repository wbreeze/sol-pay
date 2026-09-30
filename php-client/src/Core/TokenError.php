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
     * Code 1. From a settle it means one thing now that there is no
     * allowance: the fund's token account holds less than the unpaid
     * balance. {@see Shortfall::of} says by how much.
     */
    case InsufficientFunds = 1;

    /** Code 3. The token account is for a different mint than the site's. */
    case MintMismatch = 3;

    /** Code 4. The signing authority does not own the source account. */
    case OwnerMismatch = 4;

    /** Code 17. */
    case AccountFrozen = 17;

    /** Code 18. Usually a client passing the wrong decimals to deposit or withdraw. */
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
            self::InsufficientFunds => 'Insufficient funds',
            self::MintMismatch => 'Token account is for a different mint',
            self::OwnerMismatch => 'Wrong owner',
            self::AccountFrozen => 'Token account is frozen',
            self::MintDecimalsMismatch => 'Decimals do not match the mint',
        };
    }
}
