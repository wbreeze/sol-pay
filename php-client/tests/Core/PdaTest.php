<?php

declare(strict_types=1);

namespace SolPay\Tests\Core;

use PHPUnit\Framework\TestCase;
use SolPay\Core\Base58;
use SolPay\Core\Ids;
use SolPay\Core\Pda;

/**
 * Cross-checked against php-client/vectors-gen/vectors.json, generated
 * from the published sol-pay-client 0.1.1 crate by
 * php-client/vectors-gen. Inputs are sha256("authority-0") and
 * sha256("payer-0"), the same derivation vectors-gen uses for its first
 * sample, so a fresh regeneration (see pda-spike/README.md) can be diffed
 * against these constants by hand.
 *
 * The fund, fund token account and meter literals were recomputed on
 * 2026-09-29 for the fund design (SPEC.md §4.7): by this package and by an
 * independent derivation, and confirmed against the crate by bin/test-php.
 */
final class PdaTest extends TestCase
{
    private static function seed(string $tag): string
    {
        return Base58::encode(hash('sha256', $tag, true));
    }

    public function testSiteAddressMatchesTheRustCrate(): void
    {
        $site = Pda::siteAddress(self::seed('authority-0'), Ids::PAY_ON_CHAIN_ID);

        self::assertSame('HLwKN3khwF5WdLfbN2XsbQz8tYET3iH1eQ3HbRagG2BZ', $site['address']);
        self::assertSame(255, $site['bump']);
    }

    public function testFundAndItsTokenAccountMatchTheRustCrate(): void
    {
        $fund = Pda::fundAddress(self::seed('payer-0'), self::seed('mint-0'), 0);

        self::assertSame('Ci29hxcazhP6wMYrtyP89obQES1PrdXNsXcLpNfCjnUL', $fund['address']);
        self::assertSame(255, $fund['bump']);
        self::assertSame(
            'HMWvX7LhM4iJq4vJXxxSrTz5S6sHfQTXSgrHTyf5Tx7U',
            Pda::fundTokenAccount($fund['address'], self::seed('mint-0')),
        );
    }

    public function testMeterAddressMatchesTheRustCrate(): void
    {
        $site = Pda::siteAddress(self::seed('authority-0'))['address'];
        $fund = Pda::fundAddress(self::seed('payer-0'), self::seed('mint-0'), 0)['address'];
        $meter = Pda::meterAddress($site, $fund);

        self::assertSame('7BiAHXnVXJi9qxDdmzLfqhdb9FdnmVNkgo6RiU1rtto6', $meter['address']);
        self::assertSame(255, $meter['bump']);
    }

    public function testTheIndexIsOneByte(): void
    {
        $a = Pda::fundAddress(self::seed('payer-0'), self::seed('mint-0'), 0)['address'];
        $b = Pda::fundAddress(self::seed('payer-0'), self::seed('mint-0'), 255)['address'];
        self::assertNotSame($a, $b);

        $this->expectException(\InvalidArgumentException::class);
        Pda::fundAddress(self::seed('payer-0'), self::seed('mint-0'), 256);
    }

    public function testDefaultsToTheCanonicalDeployment(): void
    {
        $authority = self::seed('authority-0');

        self::assertSame(
            Pda::siteAddress($authority, Ids::PAY_ON_CHAIN_ID),
            Pda::siteAddress($authority),
        );
    }

    public function testADifferentDeploymentDerivesADifferentAddress(): void
    {
        $authority = self::seed('authority-0');
        $other = Base58::encode(str_repeat("\x09", 32));

        self::assertNotSame(
            Pda::siteAddress($authority)['address'],
            Pda::siteAddress($authority, $other)['address'],
        );
    }
}
