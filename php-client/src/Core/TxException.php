<?php

declare(strict_types=1);

namespace SolPay\Core;

final class TxException extends \RuntimeException
{
    private function __construct(
        public readonly TxErrorKind $kind,
        string $message,
    ) {
        parent::__construct($message);
    }

    public static function badAddress(string $address, int $bytes): self
    {
        return new self(
            TxErrorKind::BadAddress,
            "address {$address} decodes to {$bytes} bytes, not 32",
        );
    }

    public static function tooManyAccounts(int $count): self
    {
        return new self(
            TxErrorKind::TooManyAccounts,
            "{$count} distinct accounts; a compiled instruction indexes them with single bytes, so 255 is the limit",
        );
    }

    public static function signatureCount(int $given, int $required): self
    {
        return new self(
            TxErrorKind::SignatureCount,
            "{$given} signature(s) for a message whose header requires {$required}",
        );
    }

    public static function badSignature(int $index, int $bytes): self
    {
        return new self(
            TxErrorKind::BadSignature,
            "signature {$index} is {$bytes} bytes, not 64",
        );
    }

    public static function malformedMessage(): self
    {
        return new self(
            TxErrorKind::MalformedMessage,
            'message is too short to carry the header it declares',
        );
    }
}
