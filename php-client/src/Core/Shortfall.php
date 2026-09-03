<?php

declare(strict_types=1);

namespace SolPay\Core;

/**
 * Which constraint on the payer's token account is short, and by how much.
 * Reported rather than resolved, because both can be short at once and the
 * response differs: a low balance means top up, a low allowance means
 * re-authorize.
 */
final class Shortfall
{
    private function __construct(
        /** Zero when the balance covers it. */
        public readonly int $balanceShort,
        /** Zero when the allowance covers it. */
        public readonly int $allowanceShort,
        /**
         * False once SPL has cleared the delegate -- which it does the
         * moment the allowance is spent to zero, as well as on an explicit
         * revoke.
         */
        public readonly bool $delegatePresent,
    ) {
    }

    /**
     * Read the payer's token account and say what would stop a settle of
     * $unpaid. A read, not a guess: neither shortfall is inferable from the
     * error code alone, because SPL reports both as InsufficientFunds.
     */
    public static function diagnose(TokenAccount $account, int $unpaid): self
    {
        return new self(
            balanceShort: max(0, $unpaid - $account->amount),
            allowanceShort: max(0, $unpaid - $account->delegatedAmount),
            delegatePresent: $account->delegate !== null,
        );
    }

    /** Nothing on this account would stop a transfer of the amount asked about. */
    public function isClear(): bool
    {
        return $this->balanceShort === 0 && $this->allowanceShort === 0 && $this->delegatePresent;
    }
}
