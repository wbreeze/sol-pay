<?php

declare(strict_types=1);

namespace SolPay\Tests\Core;

use PHPUnit\Framework\TestCase;
use SolPay\Core\Base58;
use SolPay\Core\Ids;
use SolPay\Core\Ix;
use SolPay\Core\Pda;
use SolPay\Core\Program;

final class IxTest extends TestCase
{
    private static function seed(string $tag): string
    {
        return Base58::encode(hash('sha256', $tag, true));
    }

    private static function key(int $b): string
    {
        return Base58::encode(str_repeat(chr($b), 32));
    }

    private static function discriminator(string $name): string
    {
        return substr(hash('sha256', "global:$name", true), 0, 8);
    }

    /**
     * Same seeds as vectors-gen's single fully-built instruction, so this is
     * the case already checked by php-client/pda-spike/php/verify.php
     * against sol-pay-client 0.1.1 -- see vectors.json's "meter_and_settle"
     * entry for the reference bytes and account list this asserts against.
     */
    public function testMeterAndSettleMatchesTheRustCrateByteForByte(): void
    {
        $authority = self::seed('authority-0');
        $site = Pda::siteAddress($authority)['address'];
        $payer = self::seed('payer-0');
        $payerAta = self::seed('payer-ata-0');
        $treasury = self::seed('treasury-0');
        $mint = self::seed('mint-0');

        $ix = Ix::meterAndSettle(Program::default(), $site, $authority, $payer, $payerAta, $treasury, $mint, 7);

        self::assertSame(Ids::PAY_ON_CHAIN_ID, $ix->programId);
        self::assertSame('8b11008b72e9587907000000', bin2hex($ix->data));
        self::assertCount(8, $ix->accounts);

        self::assertSame($site, $ix->accounts[0]->pubkey);
        self::assertFalse($ix->accounts[0]->isSigner);
        self::assertFalse($ix->accounts[0]->isWritable);
        self::assertSame($authority, $ix->accounts[1]->pubkey);
        self::assertTrue($ix->accounts[1]->isSigner, 'authority signs');
        self::assertFalse($ix->accounts[1]->isWritable);
        self::assertFalse($ix->accounts[2]->isSigner);
        self::assertTrue($ix->accounts[3]->isWritable, 'contract is written');
        self::assertTrue($ix->accounts[4]->isWritable, 'payer token account is written');
        self::assertTrue($ix->accounts[5]->isWritable, 'treasury is written');
        self::assertFalse($ix->accounts[6]->isWritable);
        self::assertSame(Ids::TOKEN_PROGRAM_ID, $ix->accounts[7]->pubkey);
    }

    public function testDiscriminatorsMatchInstructionNames(): void
    {
        // Recomputed rather than trusted, same discipline as ix.rs's own
        // discriminators_match_instruction_names test.
        $init = Ix::initializeSite(Program::default(), self::key(1), self::key(2), self::key(3), 10, 100, 50);
        self::assertSame(self::discriminator('initialize_site'), substr($init->data, 0, 8));

        $meter = Ix::meterAndSettle(
            Program::default(),
            self::key(1),
            self::key(2),
            self::key(3),
            self::key(4),
            self::key(5),
            self::key(6),
            3,
        );
        self::assertSame(self::discriminator('meter_and_settle'), substr($meter->data, 0, 8));
    }

    public function testInitializeSiteArgsAreThreeLittleEndianU64s(): void
    {
        $ix = Ix::initializeSite(Program::default(), self::key(1), self::key(2), self::key(3), 10_000, 250_000, 500_000);

        $args = substr($ix->data, 8);
        self::assertSame(pack('P', 10_000).pack('P', 250_000).pack('P', 500_000), $args);
    }

    public function testInstructionsCarryTheDeploymentThatBuiltThem(): void
    {
        $mine = new Program(self::key(9));
        $ix = Ix::initializeSite($mine, self::key(1), self::key(2), self::key(3), 10, 100, 50);

        self::assertSame($mine->id, $ix->programId);
    }

    public function testTheTokenProgramFollowsTheHandle(): void
    {
        $t22 = Program::default()->withTokenProgram(Ids::TOKEN_2022_PROGRAM_ID);
        $ix = Ix::meterAndSettle($t22, self::key(1), self::key(2), self::key(3), self::key(4), self::key(5), self::key(6), 3);

        self::assertSame(Ids::PAY_ON_CHAIN_ID, $ix->programId, 'still our program');
        self::assertSame(Ids::TOKEN_2022_PROGRAM_ID, $ix->accounts[7]->pubkey);
    }
}
