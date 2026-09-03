<?php

declare(strict_types=1);

namespace SolPay\Core;

/**
 * Well-known program addresses, as base58 strings -- the boundary type this
 * package uses throughout. Mirrors wasm-client/src/core/ids.rs exactly;
 * these are consensus-stable.
 */
final class Ids
{
    /** The metering program. Must match `declare_id!` in the on-chain crate. */
    public const PAY_ON_CHAIN_ID = 'F8UDAGgxVTm8Vmh4RmskpMBCFqhRvuTqbDxDCj8UMedL';

    public const SYSTEM_PROGRAM_ID = '11111111111111111111111111111111';

    public const TOKEN_PROGRAM_ID = 'TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA';

    public const TOKEN_2022_PROGRAM_ID = 'TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb';
}
