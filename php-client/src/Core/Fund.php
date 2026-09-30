<?php

declare(strict_types=1);

namespace SolPay\Core;

/**
 * A reader's money in one mint, held by the program (SPEC §4.7). The balance
 * is not here: it is the balance of the fund's token account, which
 * {@see TokenAccount::decode} reads, at the address
 * {@see Pda::fundTokenAccount} derives. See {@see Site} for the decoding
 * discipline.
 */
final class Fund
{
    /** sha256("account:Fund")[0:8] */
    private const DISCRIMINATOR = "\x3e\x80\xb7\xd0\x5b\x1f\xd4\xd1";

    /** 8 discriminator + 32 + 32 + 1 + 4 + 1 */
    private const LEN = 78;

    public function __construct(
        public readonly string $reader,
        public readonly string $mint,
        public readonly int $index,
        /** Meters currently open against this fund. */
        public readonly int $meters,
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
            reader: $r->pubkey(),
            mint: $r->pubkey(),
            index: $r->u8(),
            meters: $r->u32(),
            bump: $r->u8(),
        );
    }
}
