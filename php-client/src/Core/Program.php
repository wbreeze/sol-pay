<?php

declare(strict_types=1);

namespace SolPay\Core;

/**
 * A deployment of the metering program, and the SPL token program its site's
 * mint belongs to. Mirrors wasm-client/src/core/program.rs: two things every
 * instruction builder needs, stated once here instead of at every call site.
 * Both fields are base58 addresses.
 *
 * Rust splits this into methods-on-a-handle plus free functions defaulting
 * to the canonical deployment, which avoids repeating `Program::default()`
 * at every call site -- a friction PHP doesn't have the same way, so here
 * {@see Ix} and {@see Pda} just take a `Program` (or, for `Pda`, an optional
 * program id) explicitly. Same coverage, one implementation.
 */
final class Program
{
    public function __construct(
        public readonly string $id,
        public readonly string $tokenProgram = Ids::TOKEN_PROGRAM_ID,
    ) {
    }

    /** The deployment this package targets by default, on SPL Token. */
    public static function default(): self
    {
        return new self(Ids::PAY_ON_CHAIN_ID);
    }

    /** The same deployment, against a different token program. */
    public function withTokenProgram(string $tokenProgram): self
    {
        return new self($this->id, $tokenProgram);
    }

    /**
     * Whether a mint account belongs to this handle's token program. Pass
     * the `owner` field that comes back beside a mint's data from
     * `getAccountInfo`. A mismatch means every instruction this handle
     * builds for that mint will fail at the runtime.
     */
    public function ownsMint(string $mintAccountOwner): bool
    {
        return $mintAccountOwner === $this->tokenProgram;
    }
}
