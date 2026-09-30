<?php

declare(strict_types=1);

namespace SolPay\Tests\Core;

use PHPUnit\Framework\TestCase;
use SolPay\Core\AccountMeta;
use SolPay\Core\Base58;
use SolPay\Core\Ids;
use SolPay\Core\Instruction;
use SolPay\Core\Ix;
use SolPay\Core\Pda;
use SolPay\Core\Program;
use SolPay\Core\Tx;
use SolPay\Core\TxErrorKind;
use SolPay\Core\TxException;

/**
 * Expected values are hardcoded literals, the same discipline as the other
 * suites here: a local edit that breaks compilation names the assertion that
 * broke, which freshly generated vectors cannot do. The complement is
 * `conformance/vectors.php`, which regenerates from the published crate and
 * notices the crate moving, which these literals cannot do. Neither replaces
 * the other -- see php-client/README.md, "Drift control".
 *
 * Seeds match vectors-gen's, so every literal below is the same case
 * `solana-message` and `solana-transaction` compiled.
 *
 * The meter_and_settle literals were recomputed on 2026-09-29, twice: when
 * the meter's seed changed (SPEC.md §4.11), and again for the fund design
 * (§4.7), which put the fund and its token account in the key list in place
 * of the reader's; bin/test-php confirms them against the crate.
 */
final class TxTest extends TestCase
{
    private static function seed(string $tag): string
    {
        return Base58::encode(hash('sha256', $tag, true));
    }

    /** The `authority-pays` case: the site authority signs and pays. */
    private static function meterAndSettle(): Instruction
    {
        $authority = self::seed('authority-0');

        return Ix::meterAndSettle(
            Program::default(),
            Pda::siteAddress($authority)['address'],
            $authority,
            Pda::fundAddress(self::seed('payer-0'), self::seed('mint-0'), 0)['address'],
            self::seed('treasury-0'),
            self::seed('mint-0'),
            7,
        );
    }

    private static function initializeSite(): Instruction
    {
        return Ix::initializeSite(
            Program::default(),
            self::seed('authority-0'),
            self::seed('mint-0'),
            self::seed('treasury-0'),
            10_000,
            250_000,
            500_000,
        );
    }

    public function testMeterAndSettleCompilesToTheBytesTheRustCrateProduces(): void
    {
        $message = Tx::compile([self::meterAndSettle()], self::seed('authority-0'), self::seed('blockhash-0'));

        self::assertSame(
            '010005092c8e0047d7d6624eb2213f5d1191d37301836d6731bafa4fbe874311'
            .'0bbe852a5be478319bcda025f646f4604b77f11efd93f0bb934f074c88b7ce0e'
            .'d0f0cfe57f119555a5f2e9ab30519f5113c79edef32f6c0d03f8cf1ded3cfad6'
            .'b74333e8f2fc70d575a7d90ddcd766d803d58026fb5d8c87c88cec4301ab2667'
            .'1101c34306ddf6e1d765a193d9cbe146ceeb79ac1cb485ed5f5b37913a8cf585'
            .'7eff00a9a9abf5f0e8b46c57452bdf96cc079830ab249020269c0c77435cc936'
            .'691394a8adf2ab77784b3498f01e4cfac30c3b49ce189860459d29dc13cab0ff'
            .'41b96f45d1ed72dcfbf82262eff5e9abee9cb9dca3ae229e0f170f3a55202e71'
            .'cfcb4d9ff2d6713221830bf6ab79743159da843f4ea258b322eabfa5ecf92d2c'
            .'8a6601f04d003c82f33a48a10d3fc88c2602dd2e1ae25d5372992b9eeba4dbe7'
            .'c54c929c01070808000601030205040c8b11008b72e9587907000000',
            bin2hex($message),
        );
    }

    /**
     * Everything after the account keys, which is the part `Tx` alone
     * decides: blockhash, instruction count, the program-id index, the
     * account indexes in the instruction's own order, and the length-prefixed
     * payload. The indexes are scrambled relative to the instruction because
     * the key list is sorted, which is the point of the next test.
     */
    public function testTheTailIsBlockhashThenCountsIndexesAndPayload(): void
    {
        $message = Tx::compile([self::meterAndSettle()], self::seed('authority-0'), self::seed('blockhash-0'));
        $tail = bin2hex(substr($message, 4 + 32 * 9));

        self::assertSame(
            bin2hex(hash('sha256', 'blockhash-0', true))   // recent blockhash
            .'01'                                          // one instruction
            .'07'                                          // program id is key 7
            .'08'                                          // eight accounts
            .'0800060103020504'                            // in the instruction's order
            .'0c'                                          // twelve bytes of data
            .'8b11008b72e9587907000000',
            $tail,
        );
    }

    /**
     * The rule most easily missed. Inside a partition the keys ascend by raw
     * pubkey bytes; an encoder that keeps the order the instruction named
     * them in builds a different message that still parses.
     */
    public function testPartitionsAscendByPubkeyBytesNotInstructionOrder(): void
    {
        $instruction = self::meterAndSettle();
        $message = Tx::compile([$instruction], self::seed('authority-0'), self::seed('blockhash-0'));

        $keys = [];
        for ($n = 0; $n < 9; $n++) {
            $keys[] = substr($message, 4 + 32 * $n, 32);
        }

        $readonly = array_slice($keys, 4);          // header says 1/0/5
        $sorted = $readonly;
        sort($sorted, SORT_STRING);
        self::assertSame($sorted, $readonly, 'readonly partition ascends by raw bytes');

        $named = array_map(
            static fn (AccountMeta $a): string => Base58::decode($a->pubkey),
            $instruction->accounts,
        );
        self::assertNotSame(
            array_slice($named, 0, 5),
            $readonly,
            'and that is not the order the instruction named them in',
        );
    }

    /**
     * `meter_and_settle` marks the site authority a *readonly* signer; paying
     * the fee makes it writable. Carrying the instruction's flag through
     * unchanged would give a header of 1/1/5 and a transaction the runtime
     * rejects.
     */
    public function testTheFeePayerLeadsAndIsPromotedToWritable(): void
    {
        $instruction = self::meterAndSettle();
        self::assertTrue($instruction->accounts[1]->isSigner);
        self::assertFalse($instruction->accounts[1]->isWritable, 'readonly in the instruction');

        $authority = self::seed('authority-0');
        $message = Tx::compile([$instruction], $authority, self::seed('blockhash-0'));

        self::assertSame("\x01\x00\x05", substr($message, 0, 3), 'one signer, none readonly');
        self::assertSame(Base58::decode($authority), substr($message, 4, 32), 'and it leads the keys');
    }

    /**
     * Prepended, not sorted into place. sha256("fee-payer-0") sorts *after*
     * sha256("authority-0"), and `initialize_site` makes the authority a
     * writable signer too, so an encoder that sorts all the writable signers
     * together puts them the other way round.
     */
    public function testTheFeePayerIsPrependedRatherThanSorted(): void
    {
        $feePayer = self::seed('fee-payer-0');
        $authority = self::seed('authority-0');
        self::assertGreaterThan(
            Base58::decode($authority),
            Base58::decode($feePayer),
            'the fee payer sorts after the authority, so sorting would put it second',
        );

        $message = Tx::compile(
            [self::initializeSite(), self::meterAndSettle()],
            $feePayer,
            self::seed('blockhash-0'),
        );

        self::assertSame("\x02\x00\x05", substr($message, 0, 3), 'two writable signers');
        self::assertSame(Base58::decode($feePayer), substr($message, 4, 32));
        self::assertSame(Base58::decode($authority), substr($message, 4 + 32, 32));
    }

    /**
     * `site` is writable in `initialize_site` and readonly in
     * `meter_and_settle`; `treasury` is the other way round. Both come out
     * writable: the flags describe the transaction, not one instruction.
     */
    public function testFlagsMergeAcrossInstructionsInBothDirections(): void
    {
        $feePayer = self::seed('fee-payer-0');
        $message = Tx::compile(
            [self::initializeSite(), self::meterAndSettle()],
            $feePayer,
            self::seed('blockhash-0'),
        );

        $count = ord($message[3]);
        self::assertSame(11, $count);
        $keys = [];
        for ($n = 0; $n < $count; $n++) {
            $keys[] = Base58::encode(substr($message, 4 + 32 * $n, 32));
        }

        // header 2/0/5 over 11 keys: indexes 2..5 are the writable non-signers.
        $writable = array_slice($keys, 2, 11 - 2 - 5);
        $site = Pda::siteAddress(self::seed('authority-0'))['address'];
        self::assertContains($site, $writable, 'writable in initialize_site only');
        self::assertContains(self::seed('treasury-0'), $writable, 'writable in meter_and_settle only');
    }

    public function testTheProgramIdAppearsOnceAcrossInstructions(): void
    {
        $message = Tx::compile(
            [self::initializeSite(), self::meterAndSettle()],
            self::seed('fee-payer-0'),
            self::seed('blockhash-0'),
        );

        $count = ord($message[3]);
        $keys = [];
        for ($n = 0; $n < $count; $n++) {
            $keys[] = Base58::encode(substr($message, 4 + 32 * $n, 32));
        }

        self::assertSame([Ids::PAY_ON_CHAIN_ID], array_values(array_filter(
            $keys,
            static fn (string $k): bool => $k === Ids::PAY_ON_CHAIN_ID,
        )));
    }

    /**
     * compact-u16 is a single byte only while the length is under 128. It is
     * written out properly rather than assumed, so this pins the two-byte
     * form the shipped instructions never reach.
     */
    public function testCompactU16TakesTwoBytesAbove127(): void
    {
        $signer = self::seed('authority-0');
        $instruction = new Instruction(
            self::seed('program'),
            [new AccountMeta($signer, true, true)],
            str_repeat("\x41", 200),
        );

        $message = Tx::compile([$instruction], $signer, self::seed('blockhash-0'));

        // header 3 + keycount 1 + 2 keys + blockhash 32 + ixcount 1 + program 1 + acctcount 1 + index 1
        self::assertSame("\xc8\x01", substr($message, 3 + 1 + 64 + 32 + 1 + 1 + 1 + 1, 2));
    }

    public function testWireIsTheSignatureCountThenSignaturesThenMessage(): void
    {
        $message = Tx::compile([self::meterAndSettle()], self::seed('authority-0'), self::seed('blockhash-0'));
        $signature = str_repeat("\x07", 64);

        self::assertSame("\x01".$signature.$message, Tx::wire($message, [$signature]));
    }

    public function testWireRefusesTheWrongNumberOfSignatures(): void
    {
        $message = Tx::compile([self::meterAndSettle()], self::seed('authority-0'), self::seed('blockhash-0'));

        try {
            Tx::wire($message, [str_repeat("\x07", 64), str_repeat("\x08", 64)]);
            self::fail('expected a TxException');
        } catch (TxException $e) {
            self::assertSame(TxErrorKind::SignatureCount, $e->kind);
        }
    }

    public function testWireRefusesASignatureThatIsNot64Bytes(): void
    {
        $message = Tx::compile([self::meterAndSettle()], self::seed('authority-0'), self::seed('blockhash-0'));

        try {
            Tx::wire($message, [str_repeat("\x07", 63)]);
            self::fail('expected a TxException');
        } catch (TxException $e) {
            self::assertSame(TxErrorKind::BadSignature, $e->kind);
        }
    }

    public function testCompileRefusesAnAddressThatIsNot32Bytes(): void
    {
        try {
            Tx::compile([self::meterAndSettle()], self::seed('authority-0'), '1111');
            self::fail('expected a TxException');
        } catch (TxException $e) {
            self::assertSame(TxErrorKind::BadAddress, $e->kind);
        }
    }

    /**
     * A compiled instruction indexes the key list with single bytes, so a
     * 256th key could not be referred to. Refusing beats emitting a message
     * whose indexes silently wrap.
     */
    public function testCompileRefusesMoreThan255Accounts(): void
    {
        $accounts = [];
        for ($n = 0; $n < 256; $n++) {
            $accounts[] = new AccountMeta(self::seed("filler-$n"), false, false);
        }

        try {
            Tx::compile(
                [new Instruction(self::seed('program'), $accounts, '')],
                self::seed('authority-0'),
                self::seed('blockhash-0'),
            );
            self::fail('expected a TxException');
        } catch (TxException $e) {
            self::assertSame(TxErrorKind::TooManyAccounts, $e->kind);
        }
    }
}
