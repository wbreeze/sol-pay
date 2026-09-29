<?php

declare(strict_types=1);

namespace SolPay\Core;

/** One reader's spending meter with one site. See {@see Site} for the decoding discipline. */
final class Meter
{
    private const DISCRIMINATOR = "\x05\x73\xe3\xf0\x3f\xa6\xce\xb6";

    /** 8 discriminator + 32 + 32 + 8 + 8 + 8 + 1 */
    private const LEN = 97;

    public function __construct(
        public readonly string $site,
        public readonly string $reader,
        public readonly int $limit,
        public readonly int $used,
        public readonly int $paid,
        public readonly int $bump,
    ) {
    }

    public static function decode(string $data): self
    {
        if (strlen($data) !== self::LEN) {
            throw DecodeException::wrongLength(self::LEN, strlen($data));
        }
        if (substr($data, 0, 8) !== self::DISCRIMINATOR) {
            throw DecodeException::wrongDiscriminator();
        }

        $r = new ByteReader($data, 8);
        return new self(
            site: $r->pubkey(),
            reader: $r->pubkey(),
            limit: $r->u64(),
            used: $r->u64(),
            paid: $r->u64(),
            bump: $r->u8(),
        );
    }

    /** Usage accrued but not yet transferred. Mirrors the program. */
    public function unpaid(): int
    {
        return max(0, $this->used - $this->paid);
    }

    /** What the delegate allowance still has to cover under the current limit. */
    public function outstanding(): int
    {
        return max(0, $this->limit - $this->paid);
    }
}
