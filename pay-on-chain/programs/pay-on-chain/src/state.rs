use anchor_lang::prelude::*;

/// Per-site configuration. One per site authority, so a single deployment can
/// serve several sites with different pricing.
#[account]
#[derive(InitSpace)]
pub struct Site {
    /// Signer permitted to meter usage. This is the server, not the payer.
    pub authority: Pubkey,
    /// Mint that page views are priced and settled in (USDC in practice).
    pub mint: Pubkey,
    /// Token account that collected funds land in.
    pub treasury: Pubkey,
    /// Cost of a single page view, in mint base units.
    pub page_price: u64,
    /// Minimum unpaid balance worth the cost of a transfer.
    pub collection_threshold: u64,
    /// Smallest limit a payer may authorize. Must exceed the threshold.
    pub min_limit: u64,
    pub bump: u8,
}

/// A payer's spending contract with one site.
///
/// Invariants maintained by the instructions:
///   paid <= used <= limit
///   used - paid < collection_threshold immediately after any settle
#[account]
#[derive(InitSpace)]
pub struct Contract {
    pub site: Pubkey,
    pub payer: Pubkey,
    /// Ceiling on `used`, authorized by the payer's delegate approval.
    pub limit: u64,
    /// Usage accrued, in mint base units.
    pub used: u64,
    /// Usage actually transferred to the treasury so far.
    pub paid: u64,
    pub bump: u8,
}

impl Contract {
    /// Usage accrued but not yet transferred.
    pub fn unpaid(&self) -> u64 {
        self.used.saturating_sub(self.paid)
    }

    /// Amount the delegate allowance still has to cover: everything that may
    /// yet be transferred under the current limit.
    pub fn outstanding(&self) -> u64 {
        self.limit.saturating_sub(self.paid)
    }
}
