<?php

declare(strict_types=1);

namespace SolPay\Core;

/**
 * Why a metering call would be refused. Returned, never thrown --
 * {@see Preflight} reports facts, not errors, so a blocked call is an
 * ordinary value, not an exception.
 */
final class Blocked
{
    private function __construct(
        public readonly BlockedKind $kind,
        public readonly ?int $over = null,
    ) {
    }

    public static function limitReached(int $over): self
    {
        return new self(BlockedKind::LimitReached, $over);
    }

    public static function overflow(): self
    {
        return new self(BlockedKind::Overflow);
    }

    public function __toString(): string
    {
        return match ($this->kind) {
            BlockedKind::LimitReached => "charge exceeds the authorized limit by {$this->over}",
            BlockedKind::Overflow => 'charge does not fit in this package\'s arithmetic',
        };
    }
}
