//! Address derivation. These seeds must stay in step with `constants.rs` and
//! the `#[derive(Accounts)]` structs in the on-chain program.

use solana_pubkey::Pubkey;

use super::ids::PAY_ON_CHAIN_ID;
use super::slug::Slug;

pub const SITE_SEED: &[u8] = b"site";
pub const CONTRACT_SEED: &[u8] = b"contract";
pub const SLUG_SEED: &[u8] = b"slug";

pub fn site_address(authority: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[SITE_SEED, authority.as_ref()], &PAY_ON_CHAIN_ID)
}

pub fn contract_address(site: &Pubkey, payer: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[CONTRACT_SEED, site.as_ref(), payer.as_ref()],
        &PAY_ON_CHAIN_ID,
    )
}

/// The whole point of the index: a page request turns a slug into an address
/// with one hash, then one account read. No getProgramAccounts scan.
pub fn slug_index_address(site: &Pubkey, slug: &Slug) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[SLUG_SEED, site.as_ref(), slug.as_bytes()],
        &PAY_ON_CHAIN_ID,
    )
}
