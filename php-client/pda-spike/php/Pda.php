<?php
declare(strict_types=1);

require_once __DIR__ . '/Curve25519.php';
require_once __DIR__ . '/Base58.php';

final class Pda
{
    private const MARKER = 'ProgramDerivedAddress';

    /**
     * Solana's find_program_address. Walk the bump seed downward from 255 and
     * take the first candidate that is NOT a point on the Ed25519 curve.
     *
     * $onCurve lets the caller substitute predicates so the two candidate
     * implementations can be compared. Production code passes null and gets
     * the decompression test.
     */
    public static function findProgramAddress(array $seeds, string $programId, ?callable $onCurve = null): array
    {
        $onCurve ??= [Ed25519::class, 'isOnCurve'];
        $prefix = implode('', $seeds);
        for ($bump = 255; $bump >= 0; $bump--) {
            $h = hash('sha256', $prefix . chr($bump) . $programId . self::MARKER, true);
            if (!$onCurve($h)) {
                return [$h, $bump];
            }
        }
        throw new RuntimeException('no viable bump seed');
    }

    /**
     * The predicates a PHP port would reach for instead of writing field
     * arithmetic. Both are STRICTER than Solana's is_on_curve, and both are
     * therefore wrong for PDA derivation.
     *
     * crypto_core_ed25519_is_valid_point additionally requires canonical
     * encoding, non-small-order, and prime-order subgroup membership. It is
     * also frequently absent: PHP only compiles the ed25519 core API when
     * libsodium exposes it, and a stock Ubuntu 24.04 / PHP 8.4 build does not.
     *
     * pk_to_curve25519 is what remains, and it rejects small-order and
     * non-canonical points too.
     */
    public static function sodiumIsValidPoint(string $p): bool
    {
        if (function_exists('sodium_crypto_core_ed25519_is_valid_point')) {
            return sodium_crypto_core_ed25519_is_valid_point($p);
        }
        try {
            sodium_crypto_sign_ed25519_pk_to_curve25519($p);
            return true;
        } catch (SodiumException) {
            return false;
        }
    }

    public static function strictPredicateName(): string
    {
        return function_exists('sodium_crypto_core_ed25519_is_valid_point')
            ? 'sodium_crypto_core_ed25519_is_valid_point'
            : 'sodium_crypto_sign_ed25519_pk_to_curve25519 (core ed25519 API absent in this build)';
    }
}
