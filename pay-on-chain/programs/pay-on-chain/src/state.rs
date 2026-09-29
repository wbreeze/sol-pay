use anchor_lang::prelude::*;

/// Per-site configuration. One per site authority, so a single deployment can
/// serve several sites with different pricing.
#[account]
#[derive(InitSpace)]
pub struct Site {
    /// Signer permitted to meter usage. This is the server, not the reader.
    pub authority: Pubkey,
    /// Mint that items are priced and settled in (USDC in practice).
    pub mint: Pubkey,
    /// Token account that collected money lands in.
    pub treasury: Pubkey,
    /// Cost of a single item, in mint base units.
    pub item_price: u64,
    /// Minimum unpaid balance worth the cost of a transfer.
    pub collection_threshold: u64,
    /// Smallest limit a reader may set. Must exceed the threshold.
    pub min_limit: u64,
    pub bump: u8,
}

/// A reader's money, held by the program in one mint (SPEC §4.7).
///
/// The balance is not here: it is the balance of the fund PDA's associated
/// token account, which any `transfer_checked` can extend. One reader may
/// hold several funds in one mint, told apart by `index`.
#[account]
#[derive(InitSpace)]
pub struct Fund {
    pub reader: Pubkey,
    pub mint: Pubkey,
    pub index: u8,
    /// Meters currently open against this fund. `close_fund` needs zero.
    pub meters: u32,
    pub bump: u8,
}

/// A reader's running account with one site, drawn from one fund (SPEC §4.8).
///
/// Invariants maintained by the instructions:
///   paid <= used <= limit
///   used - paid < collection_threshold immediately after any settle
#[account]
#[derive(InitSpace)]
pub struct Meter {
    pub site: Pubkey,
    pub fund: Pubkey,
    /// The browser key, per site and per device. It may sign a key proof and
    /// `close_meter`, nothing else.
    pub key: Pubkey,
    /// Unix time after which the meter cannot be metered. Governs the whole
    /// meter: metering and the key's identity end together.
    pub expiry: i64,
    /// Ceiling on `used`, set by the reader.
    pub limit: u64,
    /// Usage accrued, in mint base units.
    pub used: u64,
    /// Usage actually transferred to the treasury so far.
    pub paid: u64,
    pub bump: u8,
}

impl Meter {
    /// Usage accrued but not yet transferred.
    pub fn unpaid(&self) -> u64 {
        self.used.saturating_sub(self.paid)
    }

    /// Everything that may yet be transferred under the current limit.
    pub fn outstanding(&self) -> u64 {
        self.limit.saturating_sub(self.paid)
    }

    /// Past its expiry at `now`. Metering at `now == expiry` is still allowed.
    pub fn expired(&self, now: i64) -> bool {
        now > self.expiry
    }
}
