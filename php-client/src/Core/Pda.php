<?php

declare(strict_types=1);

namespace SolPay\Core;

/**
 * PDA derivation. `findProgramAddress` is Solana's own algorithm, working on
 * raw 32-byte strings; `siteAddress`, `fundAddress`, `fundTokenAccount` and
 * `meterAddress` are the named wrappers a server actually calls, taking and
 * returning base58 addresses -- this package's boundary type throughout.
 * Seeds mirror pay-on-chain/programs/pay-on-chain/src/constants.rs and
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
    private const FUND_SEED = 'fund';
    private const METER_SEED = 'meter';

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
        return self::derive(
            [self::SITE_SEED, Base58::decode($authority)],
            $programId ?? Ids::PAY_ON_CHAIN_ID,
        );
    }

    /**
     * A reader's fund in one mint. `$index` (0..255) tells several funds in
     * the same mint apart; nothing treats zero specially (SPEC §4.7).
     *
     * @return array{address: string, bump: int}
     */
    public static function fundAddress(string $reader, string $mint, int $index, ?string $programId = null): array
    {
        if ($index < 0 || $index > 255) {
            throw new \InvalidArgumentException("fund index must be 0..255, got $index");
        }
        return self::derive(
            [self::FUND_SEED, Base58::decode($reader), Base58::decode($mint), chr($index)],
            $programId ?? Ids::PAY_ON_CHAIN_ID,
        );
    }

    /**
     * The fund's token account: the associated token account of the fund
     * PDA, under the site's token program. This is where a deposit goes.
     */
    public static function fundTokenAccount(
        string $fund,
        string $mint,
        string $tokenProgram = Ids::TOKEN_PROGRAM_ID,
    ): string {
        return self::derive(
            [Base58::decode($fund), Base58::decode($tokenProgram), Base58::decode($mint)],
            Ids::ASSOCIATED_TOKEN_PROGRAM_ID,
        )['address'];
    }

    /**
     * One meter per site per fund.
     *
     * @return array{address: string, bump: int}
     */
    public static function meterAddress(string $site, string $fund, ?string $programId = null): array
    {
        return self::derive(
            [self::METER_SEED, Base58::decode($site), Base58::decode($fund)],
            $programId ?? Ids::PAY_ON_CHAIN_ID,
        );
    }

    /** @return array{address: string, bump: int} */
    private static function derive(array $seeds, string $programId): array
    {
        [$addr, $bump] = self::findProgramAddress($seeds, Base58::decode($programId));
        return ['address' => Base58::encode($addr), 'bump' => $bump];
    }
}
