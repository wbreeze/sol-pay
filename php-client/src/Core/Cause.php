<?php

declare(strict_types=1);

namespace SolPay\Core;

/**
 * What raised a failure, once the caller has said which program did.
 * Naming a failure means reading transaction logs, which this package
 * deliberately does not do -- see wasm-client/README.md, "Transaction logs
 * are yours to filter". Pass in the program id and code you pulled out of
 * them.
 */
final class Cause
{
    private function __construct(
        public readonly CauseKind $kind,
        public readonly ?PayError $payError = null,
        public readonly ?TokenError $tokenError = null,
        public readonly ?string $unknownProgram = null,
        public readonly ?int $unknownCode = null,
    ) {
    }

    /**
     * Name a failure raised by `$raisedBy` under `$program`'s deployment.
     * `$raisedBy` is matched against *this deployment's* address rather
     * than the compiled-in one: a site running its own deployment would
     * otherwise see every one of its own program's errors reported as
     * unknown, and lose the named failures this class exists to provide.
     */
    public static function of(Program $program, string $raisedBy, int $code): self
    {
        if ($raisedBy === $program->id) {
            $e = PayError::fromCode($code);
            if ($e !== null) {
                return new self(CauseKind::Program, payError: $e);
            }
        } elseif ($raisedBy === Ids::TOKEN_PROGRAM_ID || $raisedBy === Ids::TOKEN_2022_PROGRAM_ID) {
            $e = TokenError::fromCode($code);
            if ($e !== null) {
                return new self(CauseKind::Token, tokenError: $e);
            }
        }
        return new self(CauseKind::Unknown, unknownProgram: $raisedBy, unknownCode: $code);
    }
}
