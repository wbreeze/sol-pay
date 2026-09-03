<?php

declare(strict_types=1);

namespace SolPay\Core;

/**
 * Walks a fixed account layout. Internal to this package's decoders, not
 * part of the public API. Pubkeys come out base58-encoded -- this package's
 * boundary type throughout -- so callers never see raw address bytes.
 */
final class Reader
{
    private int $at;

    /** `$at` is where the fields start: 8 for an Anchor account, 0 for an SPL one. */
    public function __construct(
        private readonly string $bytes,
        int $at,
    ) {
        $this->at = $at;
    }

    /** A 4-byte-tagged optional pubkey, the way SPL Token writes `COption<Pubkey>`. */
    public function coOptionPubkey(): ?string
    {
        $tag = unpack('V', substr($this->bytes, $this->at, 4))[1];
        $this->at += 4;
        $key = $this->pubkey();
        return $tag === 1 ? $key : null;
    }

    public function skip(int $n): void
    {
        $this->at += $n;
    }

    public function pubkey(): string
    {
        $raw = substr($this->bytes, $this->at, 32);
        $this->at += 32;
        return Base58::encode($raw);
    }

    /**
     * A little-endian u64 as a PHP int. PHP ints are signed 64-bit, so a
     * genuine on-chain value above PHP_INT_MAX (~9.2e18) would silently
     * become a float here. Every field this package reads with it --
     * prices, limits, usage -- lives at ordinary token-amount scale, far
     * below that ceiling.
     */
    public function u64(): int
    {
        $v = unpack('P', substr($this->bytes, $this->at, 8))[1];
        $this->at += 8;
        return $v;
    }

    public function u8(): int
    {
        $v = ord($this->bytes[$this->at]);
        $this->at += 1;
        return $v;
    }
}
