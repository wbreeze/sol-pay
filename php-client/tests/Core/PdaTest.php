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

    public function testContractAddressMatchesTheRustCrate(): void
    {
        $site = Pda::siteAddress(self::seed('authority-0'))['address'];
        $contract = Pda::contractAddress($site, self::seed('payer-0'));

        self::assertSame('6974nWSDYwkuz4tXmAoqqECR2sP58HL4Ca7wkTdkpsYy', $contract['address']);
        self::assertSame(255, $contract['bump']);
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
