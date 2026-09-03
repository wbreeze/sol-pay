<?php

declare(strict_types=1);

namespace SolPay\Tests\Core;

use PHPUnit\Framework\TestCase;
use SolPay\Core\Base58;
use SolPay\Core\BlockedKind;
use SolPay\Core\Contract;
use SolPay\Core\Preflight;
use SolPay\Core\Site;

final class PreflightTest extends TestCase
{
    private static function site(int $pagePrice, int $threshold, int $minLimit): Site
    {
        return new Site(
            authority: Base58::encode(str_repeat("\x01", 32)),
            mint: Base58::encode(str_repeat("\x02", 32)),
            treasury: Base58::encode(str_repeat("\x03", 32)),
            pagePrice: $pagePrice,
            collectionThreshold: $threshold,
            minLimit: $minLimit,
            bump: 255,
        );
    }

    private static function contract(int $limit, int $used, int $paid): Contract
    {
        return new Contract(
            site: Base58::encode(str_repeat("\x01", 32)),
            payer: Base58::encode(str_repeat("\x02", 32)),
            limit: $limit,
            used: $used,
            paid: $paid,
            bump: 255,
        );
    }

    public function testCanMeterStopsExactlyWhereTheProgramDoes(): void
    {
        $s = self::site(10, 100, 500);

        $c = self::contract(1_000, 990, 0);
        self::assertNull(Preflight::canMeter($c, $s, 1));

        // 990 + 10 = 1000, exactly the limit, still allowed.
        $c = self::contract(1_000, 1_000, 0);
        $blocked = Preflight::canMeter($c, $s, 1);
        self::assertSame(BlockedKind::LimitReached, $blocked->kind);
        self::assertSame(10, $blocked->over);
    }

    public function testCanMeterReportsHowFarOver(): void
    {
        $s = self::site(10, 100, 500);
        $c = self::contract(1_000, 950, 0);

        $blocked = Preflight::canMeter($c, $s, 10);
        self::assertSame(BlockedKind::LimitReached, $blocked->kind);
        self::assertSame(50, $blocked->over);
    }

    public function testOverflowIsBlockedNotWrapped(): void
    {
        // PHP's safe ceiling is PHP_INT_MAX (~9.2e18), not u64::MAX
        // (~1.8e19) -- see Preflight's class doc -- so these values differ
        // from wasm-client's equivalent test but exercise the same path.
        $s = self::site(intdiv(PHP_INT_MAX, 2), 100, 500);
        $c = self::contract(PHP_INT_MAX, 0, 0);

        $blocked = Preflight::canMeter($c, $s, 3);
        self::assertSame(BlockedKind::Overflow, $blocked->kind);
        self::assertNull(Preflight::charge($s, 3));
        self::assertFalse(Preflight::willSettle($c, $s, 3));
    }

    public function testWillSettleOnlyAtTheThreshold(): void
    {
        $s = self::site(10, 100, 500);

        self::assertFalse(Preflight::willSettle(self::contract(1_000, 80, 0), $s, 1)); // 90 unpaid
        self::assertTrue(Preflight::willSettle(self::contract(1_000, 90, 0), $s, 1));  // 100 unpaid

        // Usage already paid for does not count toward the next settle. 150
        // used is past the threshold on its own, but only 60 of it is
        // unpaid, so a check that looked at `used` alone would settle wrongly.
        self::assertFalse(Preflight::willSettle(self::contract(1_000, 150, 100), $s, 1));

        // The boundary is the unpaid amount reaching the threshold, wherever
        // paid happens to sit.
        self::assertTrue(Preflight::willSettle(self::contract(1_000, 190, 100), $s, 1));
    }

    public function testViewsRemainingFloors(): void
    {
        $s = self::site(30, 100, 500);

        self::assertSame(33, Preflight::viewsRemaining(self::contract(1_000, 0, 0), $s));
        self::assertSame(0, Preflight::viewsRemaining(self::contract(1_000, 1_000, 0), $s));
        // Past the limit is not negative views.
        self::assertSame(0, Preflight::viewsRemaining(self::contract(1_000, 2_000, 0), $s));
    }

    public function testLimitFloorIsTheSiteMinimumWhenThereIsNoContract(): void
    {
        $s = self::site(10, 100, 500);
        self::assertSame(500, Preflight::limitFloor($s, null));
    }

    public function testLimitFloorCoversCarriedUsageWhenItExceedsTheMinimum(): void
    {
        $s = self::site(10, 100, 500);

        // Nothing carried: the minimum still rules.
        self::assertSame(500, Preflight::limitFloor($s, self::contract(1_000, 300, 300)));
        // 700 unpaid is more than the minimum, so it becomes the floor.
        self::assertSame(700, Preflight::limitFloor($s, self::contract(1_000, 900, 200)));
    }

    public function testRequiredAllowanceIsTheWholeLimit(): void
    {
        self::assertSame(12_345, Preflight::requiredAllowance(12_345));
    }
}
