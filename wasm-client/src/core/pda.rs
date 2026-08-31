//! Address derivation. These seeds must stay in step with `constants.rs` and
//! the `#[derive(Accounts)]` structs in the on-chain program.

use solana_pubkey::Pubkey;

use super::ids::PAY_ON_CHAIN_ID;

pub const SITE_SEED: &[u8] = b"site";
pub const CONTRACT_SEED: &[u8] = b"contract";

pub fn site_address(authority: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[SITE_SEED, authority.as_ref()], &PAY_ON_CHAIN_ID)
}

pub fn contract_address(site: &Pubkey, payer: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[CONTRACT_SEED, site.as_ref(), payer.as_ref()],
        &PAY_ON_CHAIN_ID,
    )
}
