<?php

declare(strict_types=1);

namespace SolPay\Core;

/** One payer's spending contract with one site. See {@see Site} for the decoding discipline. */
final class Contract
{
    private const DISCRIMINATOR = "\xac\x8a\x73\xf2\x79\x43\xb7\x1a";

    /** 8 discriminator + 32 + 32 + 8 + 8 + 8 + 1 */
    private const LEN = 97;

    public function __construct(
        public readonly string $site,
        public readonly string $payer,
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

        $r = new Reader($data, 8);
        return new self(
            site: $r->pubkey(),
            payer: $r->pubkey(),
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
