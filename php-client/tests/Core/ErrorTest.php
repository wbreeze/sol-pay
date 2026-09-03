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
    public function testPayErrorCodesRoundTrip(): void
    {
        foreach ([PayError::LimitBelowMinimum, PayError::LimitReached, PayError::DelegateAllowanceTooLow, PayError::MathOverflow] as $e) {
            self::assertSame($e, PayError::fromCode($e->code()));
        }
        self::assertSame(6003, PayError::LimitReached->code());
        self::assertNull(PayError::fromCode(5999));
        self::assertNull(PayError::fromCode(6009));
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

    private static function tokenAccount(int $amount, int $delegated, bool $hasDelegate): TokenAccount
    {
        return new TokenAccount(
            mint: Base58::encode(str_repeat("\x01", 32)),
            owner: Base58::encode(str_repeat("\x02", 32)),
            amount: $amount,
            delegate: $hasDelegate ? Base58::encode(str_repeat("\x03", 32)) : null,
            delegatedAmount: $delegated,
        );
    }

    public function testDiagnoseSeparatesWhatTheErrorCodeConflates(): void
    {
        // Balance short, allowance fine.
        $d = Shortfall::diagnose(self::tokenAccount(40, 500, true), 100);
        self::assertSame(60, $d->balanceShort);
        self::assertSame(0, $d->allowanceShort);
        self::assertFalse($d->isClear());

        // Allowance short, balance fine.
        $d = Shortfall::diagnose(self::tokenAccount(500, 40, true), 100);
        self::assertSame(0, $d->balanceShort);
        self::assertSame(60, $d->allowanceShort);

        // Both, which a single verdict would have to pick between.
        $d = Shortfall::diagnose(self::tokenAccount(40, 30, true), 100);
        self::assertSame(60, $d->balanceShort);
        self::assertSame(70, $d->allowanceShort);

        // Spent to zero: SPL clears the delegate.
        $d = Shortfall::diagnose(self::tokenAccount(500, 0, false), 100);
        self::assertFalse($d->delegatePresent);
        self::assertSame(100, $d->allowanceShort);

        $d = Shortfall::diagnose(self::tokenAccount(500, 500, true), 100);
        self::assertTrue($d->isClear());
    }
}
