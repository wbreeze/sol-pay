<?php

declare(strict_types=1);

namespace SolPay\Tests\Core;

use PHPUnit\Framework\TestCase;
use SolPay\Core\Base58;
use SolPay\Core\Cause;
use SolPay\Core\CauseKind;
use SolPay\Core\Ids;
use SolPay\Core\PayError;
use SolPay\Core\Program;
use SolPay\Core\Shortfall;
use SolPay\Core\TokenAccount;
use SolPay\Core\TokenError;

final class ErrorTest extends TestCase
{
    /**
     * Every PayError code, read by php-client/vectors-gen as
     * `PayError::<variant> as u32 + anchor_lang::error::ERROR_CODE_OFFSET`
     * against pay-on-chain's own enum -- not copied by hand -- and recorded
     * in php-client/vectors-gen/vectors.json's "pay_errors". Regenerate
     * after any change to errors.rs and update this table if it changes.
     */
    public function testPayErrorCodesMatchTheProgramsOwnEnum(): void
    {
        $fromVectorsGen = [
            PayError::LimitBelowMinimum->name => 6000,
            PayError::MinimumBelowThreshold->name => 6001,
            PayError::ZeroItemPrice->name => 6002,
            PayError::LimitReached->name => 6003,
            PayError::LimitBelowUsage->name => 6004,
            PayError::MathOverflow->name => 6005,
            PayError::MintMismatch->name => 6006,
            PayError::Expired->name => 6007,
            PayError::ExpiryInPast->name => 6008,
            PayError::Unauthorized->name => 6009,
            PayError::FundNotEmpty->name => 6010,
            PayError::FundHasMeters->name => 6011,
        ];
        foreach (PayError::cases() as $case) {
            self::assertSame($fromVectorsGen[$case->name], $case->code(), $case->name);
        }
        self::assertCount(count(PayError::cases()), $fromVectorsGen, 'every variant is covered, neither side has an extra');
    }

    /**
     * Every TokenError code this package names, read by vectors-gen from
     * spl-token's own enum (`TokenError::<variant> as u32`) rather than
     * copied by hand, and recorded in vectors.json's "token_errors".
     */
    public function testTokenErrorCodesMatchTheSplTokenEnum(): void
    {
        $fromVectorsGen = [
            TokenError::InsufficientFunds->name => 1,
            TokenError::MintMismatch->name => 3,
            TokenError::OwnerMismatch->name => 4,
            TokenError::AccountFrozen->name => 17,
            TokenError::MintDecimalsMismatch->name => 18,
        ];
        foreach (TokenError::cases() as $case) {
            self::assertSame($fromVectorsGen[$case->name], $case->code(), $case->name);
        }
        self::assertCount(count(TokenError::cases()), $fromVectorsGen, 'every variant is covered, neither side has an extra');
    }

    public function testPayErrorCodesRoundTrip(): void
    {
        foreach ([PayError::LimitBelowMinimum, PayError::LimitReached, PayError::Expired, PayError::FundHasMeters] as $e) {
            self::assertSame($e, PayError::fromCode($e->code()));
        }
        self::assertSame(6003, PayError::LimitReached->code());
        self::assertNull(PayError::fromCode(5999));
        self::assertSame(6011, PayError::FundHasMeters->code());
        self::assertNull(PayError::fromCode(6012));
        self::assertNull(PayError::fromCode(0));
    }

    public function testTokenErrorCodesRoundTrip(): void
    {
        foreach ([TokenError::InsufficientFunds, TokenError::MintDecimalsMismatch] as $e) {
            self::assertSame($e, TokenError::fromCode($e->code()));
        }
        self::assertNull(TokenError::fromCode(2));
    }

    /** The point of the whole module: 1 and 6003 are different programs speaking. */
    public function testTheSameNumberMeansDifferentThingsPerProgram(): void
    {
        $program = Program::default();

        $c = Cause::of($program, Ids::PAY_ON_CHAIN_ID, 6003);
        self::assertSame(CauseKind::Program, $c->kind);
        self::assertSame(PayError::LimitReached, $c->payError);

        $c = Cause::of($program, Ids::TOKEN_PROGRAM_ID, 1);
        self::assertSame(CauseKind::Token, $c->kind);
        self::assertSame(TokenError::InsufficientFunds, $c->tokenError);

        // Our program never raises 1, so it is not one of ours.
        $c = Cause::of($program, Ids::PAY_ON_CHAIN_ID, 1);
        self::assertSame(CauseKind::Unknown, $c->kind);
        self::assertSame(1, $c->unknownCode);

        // Token-2022 shares the code space.
        $c = Cause::of($program, Ids::TOKEN_2022_PROGRAM_ID, 1);
        self::assertSame(TokenError::InsufficientFunds, $c->tokenError);
    }

    public function testAnUnrecognisedProgramStaysUnknown(): void
    {
        $other = Base58::encode(str_repeat("\x09", 32));
        $c = Cause::of(Program::default(), $other, 6003);

        self::assertSame(CauseKind::Unknown, $c->kind);
        self::assertSame($other, $c->unknownProgram);
        self::assertSame(6003, $c->unknownCode);
    }

    /** A deployment names its own errors; the canonical handle must not claim another's. */
    public function testErrorsAreNamedAgainstTheDeploymentThatRaisedThem(): void
    {
        $other = Base58::encode(str_repeat("\x09", 32));
        $mine = new Program($other);

        $c = Cause::of($mine, $other, 6003);
        self::assertSame(PayError::LimitReached, $c->payError, 'a deployment names its own errors');

        $c = Cause::of(Program::default(), $other, 6003);
        self::assertSame(CauseKind::Unknown, $c->kind, "the canonical deployment does not claim another's");

        $c = Cause::of($mine, Ids::PAY_ON_CHAIN_ID, 6003);
        self::assertSame(CauseKind::Unknown, $c->kind, 'and the relationship is not symmetric by accident');

        // SPL is shared ground: both handles name token errors identically.
        self::assertSame(
            Cause::of($mine, Ids::TOKEN_PROGRAM_ID, 1)->tokenError,
            Cause::of(Program::default(), Ids::TOKEN_PROGRAM_ID, 1)->tokenError,
        );
    }

    private static function tokenAccount(int $amount): TokenAccount
    {
        return new TokenAccount(
            mint: Base58::encode(str_repeat("\x01", 32)),
            owner: Base58::encode(str_repeat("\x02", 32)),
            amount: $amount,
            delegate: null,
            delegatedAmount: 0,
        );
    }

    public function testShortfallIsWhatTheBalanceLacks(): void
    {
        self::assertSame(60, Shortfall::of(self::tokenAccount(40), 100));
        self::assertSame(0, Shortfall::of(self::tokenAccount(100), 100));
        self::assertSame(0, Shortfall::of(self::tokenAccount(500), 100), 'never negative');
    }
}
