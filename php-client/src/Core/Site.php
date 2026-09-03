<?php

declare(strict_types=1);

namespace SolPay\Core;

/**
 * Per-site configuration: pricing, the mint, and who may meter. Decoded
 * from an Anchor account -- 8-byte discriminator, then borsh, read back by
 * hand rather than through a Borsh library, same reasoning as
 * wasm-client/src/core/state.rs (a derive macro's dependency cost isn't
 * worth it for a handful of fixed-width fields).
 *
 * Field order is load-bearing and pinned on the Rust side by
 * pay-on-chain/tests, the one place that crate and the program build
 * together. This decoder has no such test yet and must be kept in step by
 * hand until one exists.
 */
final class Site
{
    private const DISCRIMINATOR = "\x8f\xff\x34\x0f\x41\xa5\x5e\x31";

    /** 8 discriminator + 32 + 32 + 32 + 8 + 8 + 8 + 1 */
    private const LEN = 129;

    public function __construct(
        public readonly string $authority,
        public readonly string $mint,
        public readonly string $treasury,
        public readonly int $pagePrice,
        public readonly int $collectionThreshold,
        public readonly int $minLimit,
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
            authority: $r->pubkey(),
            mint: $r->pubkey(),
            treasury: $r->pubkey(),
            pagePrice: $r->u64(),
            collectionThreshold: $r->u64(),
            minLimit: $r->u64(),
            bump: $r->u8(),
        );
    }
}
