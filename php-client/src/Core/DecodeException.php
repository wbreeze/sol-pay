<?php

declare(strict_types=1);

namespace SolPay\Core;

/** Thrown by every `decode()` in this package. Carries the same facts as wasm-client's `DecodeError`. */
final class DecodeException extends \RuntimeException
{
    private function __construct(
        public readonly DecodeErrorKind $kind,
        string $message,
        public readonly ?int $expectedLength = null,
        public readonly ?int $actualLength = null,
    ) {
        parent::__construct($message);
    }

    public static function wrongDiscriminator(): self
    {
        return new self(DecodeErrorKind::WrongDiscriminator, 'account discriminator does not match');
    }

    public static function wrongLength(int $expected, int $got): self
    {
        return new self(
            DecodeErrorKind::WrongLength,
            "account should be {$expected} bytes, got {$got}",
            $expected,
            $got,
        );
    }
}
