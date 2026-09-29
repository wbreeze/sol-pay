//! Address derivation. These seeds must stay in step with `constants.rs` and
//! the `#[derive(Accounts)]` structs in the on-chain program.

use solana_pubkey::Pubkey;

use super::ids::ASSOCIATED_TOKEN_PROGRAM_ID;
use super::program::Program;

pub const SITE_SEED: &[u8] = b"site";
pub const FUND_SEED: &[u8] = b"fund";
pub const METER_SEED: &[u8] = b"meter";

impl Program {
    pub fn site_address(&self, authority: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[SITE_SEED, authority.as_ref()], &self.id())
    }

    /// A reader's fund in one mint. `index` tells several funds in the same
    /// mint apart; nothing treats zero specially (SPEC §4.7).
    pub fn fund_address(&self, reader: &Pubkey, mint: &Pubkey, index: u8) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[FUND_SEED, reader.as_ref(), mint.as_ref(), &[index]],
            &self.id(),
        )
    }

    /// The fund's token account: the associated token account of the fund
    /// PDA, under this handle's token program. This is where a deposit goes,
    /// from anywhere.
    pub fn fund_token_account(&self, fund: &Pubkey, mint: &Pubkey) -> Pubkey {
        Pubkey::find_program_address(
            &[fund.as_ref(), self.token_program().as_ref(), mint.as_ref()],
            &ASSOCIATED_TOKEN_PROGRAM_ID,
        )
        .0
    }

    /// One meter per site per fund.
    pub fn meter_address(&self, site: &Pubkey, fund: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[METER_SEED, site.as_ref(), fund.as_ref()], &self.id())
    }
}

/// Derivation against the canonical deployment. See [`Program`] for another.
pub fn site_address(authority: &Pubkey) -> (Pubkey, u8) {
    Program::default().site_address(authority)
}

/// Derivation against the canonical deployment. See [`Program`] for another.
pub fn fund_address(reader: &Pubkey, mint: &Pubkey, index: u8) -> (Pubkey, u8) {
    Program::default().fund_address(reader, mint, index)
}

/// Derivation against the canonical deployment on SPL Token.
pub fn fund_token_account(fund: &Pubkey, mint: &Pubkey) -> Pubkey {
    Program::default().fund_token_account(fund, mint)
}

/// Derivation against the canonical deployment. See [`Program`] for another.
pub fn meter_address(site: &Pubkey, fund: &Pubkey) -> (Pubkey, u8) {
    Program::default().meter_address(site, fund)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ids::TOKEN_2022_PROGRAM_ID;

    fn k(b: u8) -> Pubkey {
        Pubkey::new_from_array([b; 32])
    }

    #[test]
    fn the_free_functions_are_the_canonical_deployment() {
        let c = Program::default();
        assert_eq!(site_address(&k(1)), c.site_address(&k(1)));
        assert_eq!(fund_address(&k(1), &k(2), 0), c.fund_address(&k(1), &k(2), 0));
        assert_eq!(fund_token_account(&k(1), &k(2)), c.fund_token_account(&k(1), &k(2)));
        assert_eq!(meter_address(&k(1), &k(2)), c.meter_address(&k(1), &k(2)));
    }

    #[test]
    fn a_different_deployment_derives_different_addresses() {
        let mine = Program::new(k(9));
        assert_ne!(mine.site_address(&k(1)), site_address(&k(1)));
        assert_ne!(mine.fund_address(&k(1), &k(2), 0), fund_address(&k(1), &k(2), 0));
        assert_ne!(mine.meter_address(&k(1), &k(2)), meter_address(&k(1), &k(2)));
    }

    #[test]
    fn the_index_separates_funds_in_one_mint() {
        assert_ne!(fund_address(&k(1), &k(2), 0), fund_address(&k(1), &k(2), 1));
        assert_ne!(fund_address(&k(1), &k(2), 0), fund_address(&k(1), &k(3), 0));
    }

    /// The token account follows the token program, because the associated
    /// token account's seeds include it. The fund itself does not move.
    #[test]
    fn the_fund_token_account_follows_the_token_program() {
        let spl = Program::default();
        let t22 = spl.with_token_program(TOKEN_2022_PROGRAM_ID);
        assert_eq!(spl.fund_address(&k(1), &k(2), 0), t22.fund_address(&k(1), &k(2), 0));
        assert_ne!(spl.fund_token_account(&k(1), &k(2)), t22.fund_token_account(&k(1), &k(2)));
    }
}
