//! Address derivation. These seeds must stay in step with `constants.rs` and
//! the `#[derive(Accounts)]` structs in the on-chain program.

use solana_pubkey::Pubkey;

use super::program::Program;

pub const SITE_SEED: &[u8] = b"site";
pub const CONTRACT_SEED: &[u8] = b"contract";

impl Program {
    pub fn site_address(&self, authority: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[SITE_SEED, authority.as_ref()], &self.id())
    }

    pub fn contract_address(&self, site: &Pubkey, payer: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[CONTRACT_SEED, site.as_ref(), payer.as_ref()],
            &self.id(),
        )
    }
}

/// Derivation against the canonical deployment. See [`Program`] for another.
pub fn site_address(authority: &Pubkey) -> (Pubkey, u8) {
    Program::default().site_address(authority)
}

/// Derivation against the canonical deployment. See [`Program`] for another.
pub fn contract_address(site: &Pubkey, payer: &Pubkey) -> (Pubkey, u8) {
    Program::default().contract_address(site, payer)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k(b: u8) -> Pubkey {
        Pubkey::new_from_array([b; 32])
    }

    #[test]
    fn the_free_functions_are_the_canonical_deployment() {
        assert_eq!(site_address(&k(1)), Program::default().site_address(&k(1)));
        assert_eq!(
            contract_address(&k(1), &k(2)),
            Program::default().contract_address(&k(1), &k(2))
        );
    }

    #[test]
    fn a_different_deployment_derives_different_addresses() {
        let mine = Program::new(k(9));
        assert_ne!(mine.site_address(&k(1)), site_address(&k(1)));
        assert_ne!(
            mine.contract_address(&k(1), &k(2)),
            contract_address(&k(1), &k(2))
        );
    }
}
