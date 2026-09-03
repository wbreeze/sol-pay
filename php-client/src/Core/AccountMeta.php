<?php

declare(strict_types=1);

namespace SolPay\Core;

/** One account in an instruction: a base58 pubkey, and whether it signs or is written. */
final class AccountMeta
{
    public function __construct(
        public readonly string $pubkey,
        public readonly bool $isSigner,
        public readonly bool $isWritable,
    ) {
    }
}
