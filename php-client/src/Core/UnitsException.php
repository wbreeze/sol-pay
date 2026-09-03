<?php

declare(strict_types=1);

namespace SolPay\Core;

final class UnitsException extends \RuntimeException
{
    private function __construct(
        public readonly UnitsErrorKind $kind,
        string $message,
        public readonly ?int $decimals = null,
        public readonly ?int $given = null,
    ) {
        parent::__construct($message);
    }

    public static function empty(): self
    {
        return new self(UnitsErrorKind::Empty, 'amount is empty');
    }

    public static function notANumber(): self
    {
        return new self(UnitsErrorKind::NotANumber, 'amount is not a decimal number');
    }

    public static function tooPrecise(int $decimals, int $given): self
    {
        return new self(
            UnitsErrorKind::TooPrecise,
            "amount has {$given} decimal places, the mint has {$decimals}",
            $decimals,
            $given,
        );
    }

    public static function overflow(): self
    {
        return new self(UnitsErrorKind::Overflow, "amount does not fit in this package's integer range");
    }
}
