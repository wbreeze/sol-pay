//! Will this succeed, and what does the reader still have room for?
//!
//! Every function here mirrors one check the program makes, so a site can ask
//! before it spends a transaction fee finding out. They report **facts, not
//! instructions**: nothing here decides what to render, redirect to, or block.
//! That line is what keeps the library out of the site's product decisions.
//!
//! The arithmetic is duplicated from the program on purpose -- the client
//! cannot call into it -- so `pay-on-chain/tests` runs each predicate against
//! the real program and requires them to agree. A predicate that disagrees is
//! worse than no predicate.

use super::state::{Meter, Site};

/// Why a metering call would be refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Blocked {
    /// The meter is past its expiry. The remedy is a renewal, same as for a
    /// full meter, but the reader is told something different.
    Expired,
    /// The charge would carry `used` past the authorized limit. The program
    /// refuses the whole call rather than metering part of it, so the site
    /// must renew or stop, not meter fewer items and hope.
    LimitReached { over: u64 },
    /// The charge itself does not fit in a u64. Only reachable with an absurd
    /// item count; the program raises `MathOverflow` for the same case.
    Overflow,
}

impl core::fmt::Display for Blocked {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Blocked::Expired => write!(f, "the meter is past its expiry"),
            Blocked::LimitReached { over } => {
                write!(f, "charge exceeds the authorized limit by {over}")
            }
            Blocked::Overflow => write!(f, "charge does not fit in u64"),
        }
    }
}

/// What `items` costs at this site's price.
pub fn charge(site: &Site, items: u32) -> Option<u64> {
    site.item_price.checked_mul(items as u64)
}

/// Mirrors the program's two refusals, in the program's order: `Expired`
/// when `now > expiry`, then `LimitReached` when `used + charge > limit`.
///
/// `now` is an argument because this crate has no clock, the same way it has
/// no RPC: pass the time the server trusts. The program reads the cluster's
/// clock, so a server whose clock is off will disagree near the expiry by
/// exactly that much (SPEC §6.3).
pub fn can_meter(meter: &Meter, site: &Site, items: u32, now: i64) -> Result<(), Blocked> {
    if meter.expired(now) {
        return Err(Blocked::Expired);
    }
    let charge = charge(site, items).ok_or(Blocked::Overflow)?;
    let new_used = meter.used.checked_add(charge).ok_or(Blocked::Overflow)?;
    if new_used > meter.limit {
        return Err(Blocked::LimitReached {
            over: new_used - meter.limit,
        });
    }
    Ok(())
}

/// Whether this call would also move money, rather than only accruing usage.
///
/// Worth knowing because a settling call touches the treasury and the fund's
/// token account, so it is the one that can fail on a low balance.
pub fn will_settle(meter: &Meter, site: &Site, items: u32) -> bool {
    match charge(site, items).and_then(|c| meter.used.checked_add(c)) {
        Some(new_used) => new_used.saturating_sub(meter.paid) >= site.collection_threshold,
        None => false,
    }
}

/// How many more items fit under the limit.
pub fn items_remaining(meter: &Meter, site: &Site) -> u64 {
    if site.item_price == 0 {
        return 0;
    }
    meter.limit.saturating_sub(meter.used) / site.item_price
}

/// The smallest limit this reader may authorize right now.
///
/// One function, not an open-limit and a renewal-limit pair. The question is
/// identical on both screens -- what is the smallest value I can accept here --
/// and the `Option` carries state the caller already holds, since looking up
/// the meter either produced one or did not.
///
/// Renewal has two requirements at once: at or above the site minimum, and
/// covering usage carried forward. `max` is both, and it degenerates to the
/// opening rule when there is no meter.
pub fn limit_floor(site: &Site, meter: Option<&Meter>) -> u64 {
    let carried = meter.map(Meter::unpaid).unwrap_or(0);
    site.min_limit.max(carried)
}

// Nothing here checks the fund's balance against a limit, deliberately: the
// program does not check it at open or renew (SPEC §4.7), so a predicate that
// did would be prescribing site policy. `error::shortfall` answers the
// balance question at the moment the program actually asks it, a settle.

#[cfg(test)]
mod tests {
    use super::*;
    use solana_pubkey::Pubkey;

    fn site(item_price: u64, threshold: u64, min_limit: u64) -> Site {
        Site {
            authority: Pubkey::new_from_array([1u8; 32]),
            mint: Pubkey::new_from_array([2u8; 32]),
            treasury: Pubkey::new_from_array([3u8; 32]),
            item_price,
            collection_threshold: threshold,
            min_limit,
            bump: 255,
        }
    }

    const NOW: i64 = 1_800_000_000;

    fn meter(limit: u64, used: u64, paid: u64) -> Meter {
        Meter {
            site: Pubkey::new_from_array([1u8; 32]),
            fund: Pubkey::new_from_array([2u8; 32]),
            key: Pubkey::new_from_array([3u8; 32]),
            expiry: NOW + 3_600,
            limit,
            used,
            paid,
            bump: 255,
        }
    }

    #[test]
    fn can_meter_stops_exactly_where_the_program_does() {
        let s = site(10, 100, 500);
        let c = meter(1_000, 990, 0);
        assert_eq!(can_meter(&c, &s, 1, NOW), Ok(()));
        // 990 + 10 = 1000, exactly the limit, still allowed.
        let c = meter(1_000, 1_000, 0);
        assert_eq!(
            can_meter(&c, &s, 1, NOW),
            Err(Blocked::LimitReached { over: 10 })
        );
    }

    #[test]
    fn can_meter_reports_how_far_over() {
        let s = site(10, 100, 500);
        let c = meter(1_000, 950, 0);
        assert_eq!(
            can_meter(&c, &s, 10, NOW),
            Err(Blocked::LimitReached { over: 50 })
        );
    }

    #[test]
    fn overflow_is_blocked_not_wrapped() {
        let s = site(u64::MAX / 2, 100, 500);
        let c = meter(u64::MAX, 0, 0);
        assert_eq!(can_meter(&c, &s, 3, NOW), Err(Blocked::Overflow));
        assert_eq!(charge(&s, 3), None);
        assert!(!will_settle(&c, &s, 3));
    }

    #[test]
    fn will_settle_only_at_the_threshold() {
        let s = site(10, 100, 500);
        assert!(!will_settle(&meter(1_000, 80, 0), &s, 1)); // 90 unpaid
        assert!(will_settle(&meter(1_000, 90, 0), &s, 1)); // 100 unpaid

        // Usage already paid for does not count toward the next settle. 150
        // used is past the threshold on its own, but only 60 of it is unpaid,
        // so a check that looked at `used` alone would settle here wrongly.
        assert!(!will_settle(&meter(1_000, 150, 100), &s, 1));

        // The boundary is the unpaid amount reaching the threshold, wherever
        // `paid` happens to sit: 190 used against 100 paid is 90 unpaid, and
        // one more item makes it exactly 100.
        assert!(will_settle(&meter(1_000, 190, 100), &s, 1));
    }

    #[test]
    fn items_remaining_floors() {
        let s = site(30, 100, 500);
        assert_eq!(items_remaining(&meter(1_000, 0, 0), &s), 33);
        assert_eq!(items_remaining(&meter(1_000, 1_000, 0), &s), 0);
        // Past the limit is not negative items.
        assert_eq!(items_remaining(&meter(1_000, 2_000, 0), &s), 0);
    }

    #[test]
    fn limit_floor_is_the_site_minimum_when_there_is_no_meter() {
        let s = site(10, 100, 500);
        assert_eq!(limit_floor(&s, None), 500);
    }

    #[test]
    fn limit_floor_covers_carried_usage_when_it_exceeds_the_minimum() {
        let s = site(10, 100, 500);
        // Nothing carried: the minimum still rules.
        assert_eq!(limit_floor(&s, Some(&meter(1_000, 300, 300))), 500);
        // 700 unpaid is more than the minimum, so it becomes the floor.
        assert_eq!(limit_floor(&s, Some(&meter(1_000, 900, 200))), 700);
    }

    /// Expiry is checked first, as the program checks it, and the expiry
    /// second itself still meters.
    #[test]
    fn expiry_comes_before_the_limit_and_is_inclusive() {
        let s = site(10, 100, 500);
        let full = meter(1_000, 1_000, 0);
        assert_eq!(can_meter(&full, &s, 1, full.expiry + 1), Err(Blocked::Expired));
        assert_eq!(
            can_meter(&full, &s, 1, full.expiry),
            Err(Blocked::LimitReached { over: 10 })
        );

        let fresh = meter(1_000, 0, 0);
        assert_eq!(can_meter(&fresh, &s, 1, fresh.expiry), Ok(()));
        assert_eq!(can_meter(&fresh, &s, 1, fresh.expiry + 1), Err(Blocked::Expired));
    }
}
