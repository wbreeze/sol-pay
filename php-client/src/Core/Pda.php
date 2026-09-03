<?php

declare(strict_types=1);

namespace SolPay\Core;

/**
 * PDA derivation. `findProgramAddress` is Solana's own algorithm, working on
 * raw 32-byte strings; `siteAddress` and `contractAddress` are the named
 * wrappers a server actually calls, taking and returning base58 addresses --
 * this package's boundary type throughout. Seeds mirror
 * pay-on-chain/programs/pay-on-chain/src/constants.rs and
 * wasm-client/src/core/pda.rs.
 *
 * This class carries only what the live package needs. The comparison
 * against libsodium's stricter predicate that motivated writing
 * {@see Ed25519} lives in php-client/pda-spike/php/Pda.php, which stays as
 * the record of that experiment.
 */
final class Pda
{
    private const MARKER = 'ProgramDerivedAddress';
    private const SITE_SEED = 'site';
    private const CONTRACT_SEED = 'contract';

    /**
     * Solana's find_program_address: walk the bump seed downward from 255
     * and take the first candidate that is NOT a point on the Ed25519 curve.
     * $seeds and $programId are raw byte strings; returns [rawAddress, bump].
     */
    public static function findProgramAddress(array $seeds, string $programId): array
    {
        $prefix = implode('', $seeds);
        for ($bump = 255; $bump >= 0; $bump--) {
            $h = hash('sha256', $prefix.chr($bump).$programId.self::MARKER, true);
            if (!Ed25519::isOnCurve($h)) {
                return [$h, $bump];
            }
        }
        throw new \RuntimeException('no viable bump seed');
    }

    /** @return array{address: string, bump: int} */
    public static function siteAddress(string $authority, ?string $programId = null): array
    {
        $programId ??= Ids::PAY_ON_CHAIN_ID;
        [$addr, $bump] = self::findProgramAddress(
            [self::SITE_SEED, Base58::decode($authority)],
            Base58::decode($programId),
        );
        return ['address' => Base58::encode($addr), 'bump' => $bump];
    }

    /** @return array{address: string, bump: int} */
    public static function contractAddress(string $site, string $payer, ?string $programId = null): array
    {
        $programId ??= Ids::PAY_ON_CHAIN_ID;
        [$addr, $bump] = self::findProgramAddress(
            [self::CONTRACT_SEED, Base58::decode($site), Base58::decode($payer)],
            Base58::decode($programId),
        );
        return ['address' => Base58::encode($addr), 'bump' => $bump];
    }
}
