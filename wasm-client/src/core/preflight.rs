//! Will this succeed, and what does the payer still have room for?
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

use super::state::{Contract, Site};

/// Why a metering call would be refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Blocked {
    /// The charge would carry `used` past the authorized limit. The program
    /// refuses the whole call rather than metering part of it, so the site
    /// must renew or stop, not meter fewer views and hope.
    LimitReached { over: u64 },
    /// The charge itself does not fit in a u64. Only reachable with an absurd
    /// page count; the program raises `MathOverflow` for the same case.
    Overflow,
}

impl core::fmt::Display for Blocked {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Blocked::LimitReached { over } => {
                write!(f, "charge exceeds the authorized limit by {over}")
            }
            Blocked::Overflow => write!(f, "charge does not fit in u64"),
        }
    }
}

/// What `page_views` costs at this site's price.
pub fn charge(site: &Site, page_views: u32) -> Option<u64> {
    site.page_price.checked_mul(page_views as u64)
}

/// Mirrors `require!(new_used <= limit, LimitReached)`.
pub fn can_meter(contract: &Contract, site: &Site, page_views: u32) -> Result<(), Blocked> {
    let charge = charge(site, page_views).ok_or(Blocked::Overflow)?;
    let new_used = contract.used.checked_add(charge).ok_or(Blocked::Overflow)?;
    if new_used > contract.limit {
        return Err(Blocked::LimitReached {
            over: new_used - contract.limit,
        });
    }
    Ok(())
}

/// Whether this call would also move money, rather than only accruing usage.
///
/// Worth knowing because a settling call touches the treasury and the payer's
/// token account, so it is the one that can fail on a low balance.
pub fn will_settle(contract: &Contract, site: &Site, page_views: u32) -> bool {
    match charge(site, page_views).and_then(|c| contract.used.checked_add(c)) {
        Some(new_used) => new_used.saturating_sub(contract.paid) >= site.collection_threshold,
        None => false,
    }
}

/// How many more views fit under the limit.
pub fn views_remaining(contract: &Contract, site: &Site) -> u64 {
    if site.page_price == 0 {
        return 0;
    }
    contract.limit.saturating_sub(contract.used) / site.page_price
}

/// The smallest limit this payer may authorize right now.
///
/// One function, not an open-limit and a renewal-limit pair. The question is
/// identical on both screens -- what is the smallest value I can accept here --
/// and the `Option` carries state the caller already holds, since looking up
/// the contract either produced one or did not.
///
/// Renewal has two requirements at once: at or above the site minimum, and
/// covering usage carried forward. `max` is both, and it degenerates to the
/// opening rule when there is no contract.
pub fn limit_floor(site: &Site, contract: Option<&Contract>) -> u64 {
    let carried = contract.map(Contract::unpaid).unwrap_or(0);
    site.min_limit.max(carried)
}

/// What the SPL approval must cover for a contract at `limit`.
///
/// The whole limit: nothing is paid against a new limit yet, and the program
/// checks the allowance against it at open and at renew.
pub fn required_allowance(limit: u64) -> u64 {
    limit
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_pubkey::Pubkey;

    fn site(page_price: u64, threshold: u64, min_limit: u64) -> Site {
        Site {
            authority: Pubkey::new_from_array([1u8; 32]),
            mint: Pubkey::new_from_array([2u8; 32]),
            treasury: Pubkey::new_from_array([3u8; 32]),
            page_price,
            collection_threshold: threshold,
            min_limit,
            bump: 255,
        }
    }

    fn contract(limit: u64, used: u64, paid: u64) -> Contract {
        Contract {
            site: Pubkey::new_from_array([1u8; 32]),
            payer: Pubkey::new_from_array([2u8; 32]),
            limit,
            used,
            paid,
            bump: 255,
        }
    }

    #[test]
    fn can_meter_stops_exactly_where_the_program_does() {
        let s = site(10, 100, 500);
        let c = contract(1_000, 990, 0);
        assert_eq!(can_meter(&c, &s, 1), Ok(()));
        // 990 + 10 = 1000, exactly the limit, still allowed.
        let c = contract(1_000, 1_000, 0);
        assert_eq!(
            can_meter(&c, &s, 1),
            Err(Blocked::LimitReached { over: 10 })
        );
    }

    #[test]
    fn can_meter_reports_how_far_over() {
        let s = site(10, 100, 500);
        let c = contract(1_000, 950, 0);
        assert_eq!(
            can_meter(&c, &s, 10),
            Err(Blocked::LimitReached { over: 50 })
        );
    }

    #[test]
    fn overflow_is_blocked_not_wrapped() {
        let s = site(u64::MAX / 2, 100, 500);
        let c = contract(u64::MAX, 0, 0);
        assert_eq!(can_meter(&c, &s, 3), Err(Blocked::Overflow));
        assert_eq!(charge(&s, 3), None);
        assert!(!will_settle(&c, &s, 3));
    }

    #[test]
    fn will_settle_only_at_the_threshold() {
        let s = site(10, 100, 500);
        assert!(!will_settle(&contract(1_000, 80, 0), &s, 1)); // 90 unpaid
        assert!(will_settle(&contract(1_000, 90, 0), &s, 1)); // 100 unpaid

        // Usage already paid for does not count toward the next settle. 150
        // used is past the threshold on its own, but only 60 of it is unpaid,
        // so a check that looked at `used` alone would settle here wrongly.
        assert!(!will_settle(&contract(1_000, 150, 100), &s, 1));

        // The boundary is the unpaid amount reaching the threshold, wherever
        // `paid` happens to sit: 190 used against 100 paid is 90 unpaid, and
        // one more view makes it exactly 100.
        assert!(will_settle(&contract(1_000, 190, 100), &s, 1));
    }

    #[test]
    fn views_remaining_floors() {
        let s = site(30, 100, 500);
        assert_eq!(views_remaining(&contract(1_000, 0, 0), &s), 33);
        assert_eq!(views_remaining(&contract(1_000, 1_000, 0), &s), 0);
        // Past the limit is not negative views.
        assert_eq!(views_remaining(&contract(1_000, 2_000, 0), &s), 0);
    }

    #[test]
    fn limit_floor_is_the_site_minimum_when_there_is_no_contract() {
        let s = site(10, 100, 500);
        assert_eq!(limit_floor(&s, None), 500);
    }

    #[test]
    fn limit_floor_covers_carried_usage_when_it_exceeds_the_minimum() {
        let s = site(10, 100, 500);
        // Nothing carried: the minimum still rules.
        assert_eq!(limit_floor(&s, Some(&contract(1_000, 300, 300))), 500);
        // 700 unpaid is more than the minimum, so it becomes the floor.
        assert_eq!(limit_floor(&s, Some(&contract(1_000, 900, 200))), 700);
    }

    #[test]
    fn required_allowance_is_the_whole_limit() {
        assert_eq!(required_allowance(12_345), 12_345);
    }
}
