<?php

declare(strict_types=1);

namespace SolPay\Core;

/**
 * Key proof (SPEC §6.6): is the browser in front of a site the one a meter
 * names? Mirrors wasm-client/src/core/proof.rs.
 *
 * The site's server composes some bytes, the page signs them with the key it
 * generated, and the server checks the signature against the key the meter
 * names -- {@see Meter::$key}, fetched from the chain. There is no message
 * format and no address to recover.
 *
 * libsodium's `crypto_sign_verify_detached`, which ext-sodium has carried
 * since PHP 7.2. It is the signature API, not the ed25519 *point* API whose
 * absence is why {@see Ed25519} is hand-written; see README, "Transaction
 * assembly". It refuses small-order keys and non-canonical signatures, as the
 * Rust crate's `verify_strict` does, so the two ports agree on what passes.
 *
 * **A valid signature is not a live meter.** After this says yes the site
 * still owes three checks of its own: that the meter's `key` is the key it
 * verified against, that the meter is not `expired($now)`, and that the
 * meter's `site` is this site. And the bytes must carry a nonce and a time
 * the server issued and remembers, or a replayed proof passes forever.
 */
final class Proof
{
    private function __construct()
    {
    }

    /**
     * True when `$signature` (64 raw bytes) is a valid Ed25519 signature by
     * `$key` (base58, as {@see Meter} decodes it) over `$message`. Nothing
     * else: no format, no expiry, no nonce -- those are the site's.
     */
    public static function verifyKey(string $key, string $message, string $signature): bool
    {
        $raw = Base58::decode($key);
        if (strlen($raw) !== SODIUM_CRYPTO_SIGN_PUBLICKEYBYTES || strlen($signature) !== SODIUM_CRYPTO_SIGN_BYTES) {
            return false;
        }
        try {
            return sodium_crypto_sign_verify_detached($signature, $message, $raw);
        } catch (\SodiumException) {
            return false;
        }
    }
}
