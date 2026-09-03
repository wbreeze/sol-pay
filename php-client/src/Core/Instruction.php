<?php

declare(strict_types=1);

namespace SolPay\Core;

/**
 * A built instruction: the program it calls, its accounts, and its raw
 * data. `data` is a binary string -- Borsh-encoded bytes, not text -- and
 * `programId` is a base58 address, same as every account in `accounts`.
 */
final class Instruction
{
    /** @param AccountMeta[] $accounts */
    public function __construct(
        public readonly string $programId,
        public readonly array $accounts,
        public readonly string $data,
    ) {
    }
}
