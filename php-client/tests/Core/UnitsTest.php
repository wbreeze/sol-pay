<?php

declare(strict_types=1);

namespace SolPay\Tests\Core;

use PHPUnit\Framework\TestCase;
use SolPay\Core\Units;
use SolPay\Core\UnitsErrorKind;
use SolPay\Core\UnitsException;

final class UnitsTest extends TestCase
{
    public function testParsesAtUsdcScale(): void
    {
        self::assertSame(1_000_000, Units::toBaseUnits('1', 6));
        self::assertSame(1_500_000, Units::toBaseUnits('1.5', 6));
        self::assertSame(1, Units::toBaseUnits('0.000001', 6));
        self::assertSame(500_000, Units::toBaseUnits('.5', 6));
        self::assertSame(12_000_000, Units::toBaseUnits('12.', 6));
        self::assertSame(0, Units::toBaseUnits('0', 6));
        self::assertSame(2_250_000, Units::toBaseUnits('  2.25  ', 6));
    }

    public function testRefusesWhatItCannotRepresentRatherThanRounding(): void
    {
        try {
            Units::toBaseUnits('0.0000001', 6);
            self::fail('expected a UnitsException');
        } catch (UnitsException $e) {
            self::assertSame(UnitsErrorKind::TooPrecise, $e->kind);
            self::assertSame(6, $e->decimals);
            self::assertSame(7, $e->given);
        }

        $this->assertThrowsKind(fn () => Units::toBaseUnits('', 6), UnitsErrorKind::Empty);
        $this->assertThrowsKind(fn () => Units::toBaseUnits('.', 6), UnitsErrorKind::NotANumber);
        $this->assertThrowsKind(fn () => Units::toBaseUnits('-1', 6), UnitsErrorKind::NotANumber);
        $this->assertThrowsKind(fn () => Units::toBaseUnits('1e6', 6), UnitsErrorKind::NotANumber);
        $this->assertThrowsKind(fn () => Units::toBaseUnits('1_000', 6), UnitsErrorKind::NotANumber);
        $this->assertThrowsKind(fn () => Units::toBaseUnits('1.2.3', 6), UnitsErrorKind::NotANumber);
        $this->assertThrowsKind(fn () => Units::toBaseUnits('184467440738', 9), UnitsErrorKind::Overflow);
    }

    private function assertThrowsKind(callable $fn, UnitsErrorKind $kind): void
    {
        try {
            $fn();
            self::fail("expected a UnitsException ({$kind->name})");
        } catch (UnitsException $e) {
            self::assertSame($kind, $e->kind);
        }
    }

    public function testRendersWithoutTrailingZeros(): void
    {
        self::assertSame('1.5', Units::fromBaseUnits(1_500_000, 6));
        self::assertSame('1', Units::fromBaseUnits(1_000_000, 6));
        self::assertSame('0.000001', Units::fromBaseUnits(1, 6));
        self::assertSame('0', Units::fromBaseUnits(0, 6));
        self::assertSame('42', Units::fromBaseUnits(42, 0));
    }

    public function testRoundTrips(): void
    {
        // PHP has no unsigned 64-bit type; PHP_INT_MAX (~9.2e18) is this
        // package's ceiling, coincidentally equal to u64::MAX / 2 -- see
        // Units's class doc.
        foreach ([0, 1, 999_999, 1_000_000, 1_500_000, PHP_INT_MAX] as $units) {
            foreach ([0, 2, 6, 9] as $decimals) {
                $text = Units::fromBaseUnits($units, $decimals);
                self::assertSame($units, Units::toBaseUnits($text, $decimals), "{$text} at {$decimals} decimals");
            }
        }
    }

    /** The fault this module exists to prevent. */
    public function testScalingTwiceIsNotSilentlyAccepted(): void
    {
        $once = Units::toBaseUnits('50', 6);
        self::assertSame(50_000_000, $once);
        // Feeding the already-scaled number back in is the bug. It is a
        // different number, loudly, rather than the same one quietly.
        $twice = Units::toBaseUnits((string) $once, 6);
        self::assertNotSame($once, $twice);
        self::assertSame(50_000_000_000_000, $twice);
    }
}
