<?php

declare(strict_types=1);

namespace SolPay\Core;

/**
 * The payer's SPL token account, as much of it as this package needs. Not
 * an Anchor account, so no discriminator: SPL writes a fixed 165-byte
 * layout, and Token-2022 appends extensions past that, which is why
 * anything at least that long decodes.
 */
final class TokenAccount
{
    private const LEN = 165;

    public function __construct(
        public readonly string $mint,
        public readonly string $owner,
        public readonly int $amount,
        /**
         * Whom the owner has approved, if anyone. SPL clears this once the
         * approved amount reaches zero, so null is ordinary, not exceptional.
         */
        public readonly ?string $delegate,
        /** How much that delegate may still move. Decremented by every delegated transfer. */
        public readonly int $delegatedAmount,
    ) {
    }

    public static function decode(string $data): self
    {
        if (strlen($data) < self::LEN) {
            throw DecodeException::wrongLength(self::LEN, strlen($data));
        }

        $r = new Reader($data, 0);
        $mint = $r->pubkey();
        $owner = $r->pubkey();
        $amount = $r->u64();
        $delegate = $r->coOptionPubkey();
        $r->skip(1);  // state
        $r->skip(12); // is_native: COption<u64>
        $delegatedAmount = $r->u64();

        return new self($mint, $owner, $amount, $delegate, $delegatedAmount);
    }
}
