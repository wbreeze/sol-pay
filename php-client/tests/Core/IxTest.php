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
     * Same seeds as vectors-gen's fully-built meter_and_settle, so this is
     * the case conformance/vectors.php checks against the Rust crate -- see
     * vectors.json's "meter_and_settle" entry for the reference bytes and
     * account list this asserts against.
     */
    public function testMeterAndSettleMatchesTheRustCrateByteForByte(): void
    {
        $authority = self::seed('authority-0');
        $site = Pda::siteAddress($authority)['address'];
        $mint = self::seed('mint-0');
        $fund = Pda::fundAddress(self::seed('payer-0'), $mint, 0)['address'];
        $treasury = self::seed('treasury-0');

        $ix = Ix::meterAndSettle(Program::default(), $site, $authority, $fund, $treasury, $mint, 7);

        self::assertSame(Ids::PAY_ON_CHAIN_ID, $ix->programId);
        self::assertSame('8b11008b72e9587907000000', bin2hex($ix->data));
        self::assertCount(8, $ix->accounts);

        self::assertSame($site, $ix->accounts[0]->pubkey);
        self::assertFalse($ix->accounts[0]->isSigner);
        self::assertFalse($ix->accounts[0]->isWritable);
        self::assertSame($authority, $ix->accounts[1]->pubkey);
        self::assertTrue($ix->accounts[1]->isSigner, 'authority signs');
        self::assertFalse($ix->accounts[1]->isWritable);
        self::assertSame($fund, $ix->accounts[2]->pubkey);
        self::assertFalse($ix->accounts[2]->isWritable, 'the fund is only read');
        self::assertTrue($ix->accounts[3]->isWritable, 'meter is written');
        self::assertSame(Pda::fundTokenAccount($fund, $mint), $ix->accounts[4]->pubkey);
        self::assertTrue($ix->accounts[4]->isWritable, "the fund's token account pays");
        self::assertTrue($ix->accounts[5]->isWritable, 'treasury is written');
        self::assertFalse($ix->accounts[6]->isWritable);
        self::assertSame(Ids::TOKEN_PROGRAM_ID, $ix->accounts[7]->pubkey);
    }

    public function testDiscriminatorsMatchInstructionNames(): void
    {
        // Recomputed rather than trusted, same discipline as ix.rs's own
        // discriminators_match_instruction_names test.
        $p = Program::default();
        $k = [1 => self::key(1), self::key(2), self::key(3), self::key(4), self::key(5)];
        $built = [
            'initialize_site' => Ix::initializeSite($p, $k[1], $k[2], $k[3], 10, 100, 50),
            'open_fund' => Ix::openFund($p, $k[1], $k[2], 0),
            'withdraw' => Ix::withdraw($p, $k[1], $k[2], 0, $k[3], 5),
            'close_fund' => Ix::closeFund($p, $k[1], $k[2], 0),
            'open_meter' => Ix::openMeter($p, $k[1], $k[2], $k[3], $k[4], 500, 60),
            'meter_and_settle' => Ix::meterAndSettle($p, $k[1], $k[2], $k[3], $k[4], $k[5], 3),
            'renew_meter' => Ix::renewMeter($p, $k[1], $k[2], $k[3], $k[4], 900, 60),
            'close_meter' => Ix::closeMeter($p, $k[1], $k[2], $k[3], $k[4]),
        ];
        foreach ($built as $name => $ix) {
            self::assertSame(self::discriminator($name), substr($ix->data, 0, 8), $name);
        }
    }

    public function testInitializeSiteArgsAreThreeLittleEndianU64s(): void
    {
        $ix = Ix::initializeSite(Program::default(), self::key(1), self::key(2), self::key(3), 10_000, 250_000, 500_000);

        $args = substr($ix->data, 8);
        self::assertSame(pack('P', 10_000).pack('P', 250_000).pack('P', 500_000), $args);
    }

    /** The key's 32 bytes, then the limit and the expiry, little-endian: 56 bytes after the discriminator. */
    public function testOpenMeterArgsAreKeyLimitAndExpiry(): void
    {
        $ix = Ix::openMeter(Program::default(), self::key(1), self::key(2), self::key(3), self::key(4), 500, -1);

        self::assertSame(8 + 32 + 8 + 8, strlen($ix->data));
        self::assertSame(str_repeat("\x04", 32), substr($ix->data, 8, 32));
        self::assertSame(pack('P', 500), substr($ix->data, 40, 8));
        self::assertSame(str_repeat("\xff", 8), substr($ix->data, 48, 8), 'a negative i64, two\'s complement');
    }

    /** A deposit is SPL's transfer_checked into the fund's token account, signed by the source's owner. */
    public function testADepositIsATransferIntoTheFundsTokenAccount(): void
    {
        $fund = Pda::fundAddress(self::key(3), self::key(2), 0)['address'];
        $ix = Ix::deposit(Program::default(), self::key(1), self::key(3), $fund, self::key(2), 500, 6);

        self::assertSame(Ids::TOKEN_PROGRAM_ID, $ix->programId, 'an SPL instruction');
        self::assertSame("\x0c".pack('P', 500)."\x06", $ix->data);
        self::assertSame(Pda::fundTokenAccount($fund, self::key(2)), $ix->accounts[2]->pubkey);
        self::assertTrue($ix->accounts[3]->isSigner, "the source's owner signs");
    }

    public function testInstructionsCarryTheDeploymentThatBuiltThem(): void
    {
        $mine = new Program(self::key(9));
        $ix = Ix::initializeSite($mine, self::key(1), self::key(2), self::key(3), 10, 100, 50);
        self::assertSame($mine->id, $ix->programId);

        $ix = Ix::openFund($mine, self::key(1), self::key(2), 0);
        self::assertSame($mine->id, $ix->programId);
        self::assertSame(Pda::fundAddress(self::key(1), self::key(2), 0, $mine->id)['address'], $ix->accounts[1]->pubkey);
    }

    public function testTheTokenProgramFollowsTheHandle(): void
    {
        $t22 = Program::default()->withTokenProgram(Ids::TOKEN_2022_PROGRAM_ID);
        $ix = Ix::meterAndSettle($t22, self::key(1), self::key(2), self::key(3), self::key(4), self::key(5), 3);

        self::assertSame(Ids::PAY_ON_CHAIN_ID, $ix->programId, 'still our program');
        self::assertSame(Ids::TOKEN_2022_PROGRAM_ID, $ix->accounts[7]->pubkey);
        self::assertSame(
            Pda::fundTokenAccount(self::key(3), self::key(5), Ids::TOKEN_2022_PROGRAM_ID),
            $ix->accounts[4]->pubkey,
            'and the fund token account is derived under it',
        );
        self::assertSame(Ids::TOKEN_2022_PROGRAM_ID, Ix::deposit($t22, self::key(1), self::key(2), self::key(3), self::key(5), 5, 6)->programId);
    }

    public function testOpenFundAndDepositPairsTheBuildersInOrder(): void
    {
        $p = Program::default();
        [$open, $deposit] = Ix::openFundAndDeposit($p, self::key(1), self::key(2), 4, self::key(3), 500, 6);
        $fund = Pda::fundAddress(self::key(1), self::key(2), 4)['address'];

        self::assertEquals(Ix::openFund($p, self::key(1), self::key(2), 4), $open);
        self::assertEquals(Ix::deposit($p, self::key(3), self::key(1), $fund, self::key(2), 500, 6), $deposit);
    }
}
