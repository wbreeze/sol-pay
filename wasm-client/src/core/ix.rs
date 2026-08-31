//! Instruction builders. Pure functions over addresses and amounts; no I/O,
//! no signing, no browser. Account order in every builder mirrors the field
//! order of the matching `#[derive(Accounts)]` struct, which is what Anchor
//! expects.

use borsh::BorshSerialize;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

use super::ids::*;
use super::pda::*;

/// Anchor discriminators: the first eight bytes of sha256("global:<name>").
/// Precomputed so the client needs no hash dependency; `tests` below
/// recomputes them, so a renamed instruction fails the test rather than
/// silently building a call nobody answers.
pub mod discriminator {
    pub const INITIALIZE_SITE: [u8; 8] = [85, 52, 128, 208, 7, 224, 178, 79];
    pub const OPEN_CONTRACT: [u8; 8] = [124, 62, 192, 145, 192, 90, 59, 211];
    pub const METER_AND_SETTLE: [u8; 8] = [139, 17, 0, 139, 114, 233, 88, 121];
    pub const RENEW_CONTRACT: [u8; 8] = [125, 228, 198, 154, 176, 239, 140, 144];
    pub const CLOSE_CONTRACT: [u8; 8] = [37, 244, 34, 168, 92, 202, 80, 106];
}

fn data(disc: [u8; 8], args: &impl BorshSerialize) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + 32);
    out.extend_from_slice(&disc);
    args.serialize(&mut out).expect("borsh into Vec cannot fail");
    out
}

#[derive(BorshSerialize)]
struct InitializeSiteArgs {
    page_price: u64,
    collection_threshold: u64,
    min_limit: u64,
}

#[derive(BorshSerialize)]
struct OpenContractArgs {
    limit: u64,
}

#[derive(BorshSerialize)]
struct MeterAndSettleArgs {
    page_views: u32,
}

#[derive(BorshSerialize)]
struct RenewContractArgs {
    new_limit: u64,
}

pub fn initialize_site(
    authority: &Pubkey,
    mint: &Pubkey,
    treasury: &Pubkey,
    page_price: u64,
    collection_threshold: u64,
    min_limit: u64,
) -> Instruction {
    let (site, _) = site_address(authority);
    Instruction {
        program_id: PAY_ON_CHAIN_ID,
        accounts: vec![
            AccountMeta::new(*authority, true),
            AccountMeta::new(site, false),
            AccountMeta::new_readonly(*mint, false),
            AccountMeta::new_readonly(*treasury, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
        ],
        data: data(
            discriminator::INITIALIZE_SITE,
            &InitializeSiteArgs {
                page_price,
                collection_threshold,
                min_limit,
            },
        ),
    }
}

/// Must be preceded in the same transaction by [`approve_checked`] naming the
/// contract PDA as delegate for at least `limit`.
pub fn open_contract(
    site: &Pubkey,
    payer: &Pubkey,
    payer_token_account: &Pubkey,
    limit: u64,
) -> Instruction {
    let (contract, _) = contract_address(site, payer);
    Instruction {
        program_id: PAY_ON_CHAIN_ID,
        accounts: vec![
            AccountMeta::new(*payer, true),
            AccountMeta::new_readonly(*site, false),
            AccountMeta::new(contract, false),
            AccountMeta::new_readonly(*payer_token_account, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
        ],
        data: data(discriminator::OPEN_CONTRACT, &OpenContractArgs { limit }),
    }
}

/// Signed by the site authority. The payer is absent; the transfer, if the
/// threshold is crossed, rides on the delegate approval.
#[allow(clippy::too_many_arguments)]
pub fn meter_and_settle(
    site: &Pubkey,
    authority: &Pubkey,
    payer: &Pubkey,
    payer_token_account: &Pubkey,
    treasury: &Pubkey,
    mint: &Pubkey,
    token_program: &Pubkey,
    page_views: u32,
) -> Instruction {
    let (contract, _) = contract_address(site, payer);
    Instruction {
        program_id: PAY_ON_CHAIN_ID,
        accounts: vec![
            AccountMeta::new_readonly(*site, false),
            AccountMeta::new_readonly(*authority, true),
            AccountMeta::new_readonly(*payer, false),
            AccountMeta::new(contract, false),
            AccountMeta::new(*payer_token_account, false),
            AccountMeta::new(*treasury, false),
            AccountMeta::new_readonly(*mint, false),
            AccountMeta::new_readonly(*token_program, false),
        ],
        data: data(
            discriminator::METER_AND_SETTLE,
            &MeterAndSettleArgs { page_views },
        ),
    }
}

/// Must be preceded by [`approve_checked`] for at least `new_limit`.
pub fn renew_contract(
    site: &Pubkey,
    payer: &Pubkey,
    payer_token_account: &Pubkey,
    new_limit: u64,
) -> Instruction {
    let (contract, _) = contract_address(site, payer);
    Instruction {
        program_id: PAY_ON_CHAIN_ID,
        accounts: vec![
            AccountMeta::new(*payer, true),
            AccountMeta::new_readonly(*site, false),
            AccountMeta::new(contract, false),
            AccountMeta::new_readonly(*payer_token_account, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
        ],
        data: data(discriminator::RENEW_CONTRACT, &RenewContractArgs { new_limit }),
    }
}

pub fn close_contract(site: &Pubkey, payer: &Pubkey) -> Instruction {
    let (contract, _) = contract_address(site, payer);
    Instruction {
        program_id: PAY_ON_CHAIN_ID,
        accounts: vec![
            AccountMeta::new(*payer, true),
            AccountMeta::new_readonly(*site, false),
            AccountMeta::new(contract, false),
        ],
        data: discriminator::CLOSE_CONTRACT.to_vec(),
    }
}

// --- SPL Token instructions the flow needs -------------------------------
//
// Built by hand rather than pulling in spl-token, which drags a large
// dependency tree into a WASM bundle for two instructions. The tags are
// stable parts of the SPL Token ABI.

const TAG_APPROVE_CHECKED: u8 = 13;
const TAG_REVOKE: u8 = 5;

/// The authorization step: let the contract PDA move up to `amount` of the
/// payer's tokens. `approve` *replaces* any previous allowance, it does not
/// add to it, so renewal passes the new limit outright.
pub fn approve_checked(
    token_program: &Pubkey,
    payer_token_account: &Pubkey,
    mint: &Pubkey,
    payer: &Pubkey,
    site: &Pubkey,
    amount: u64,
    decimals: u8,
) -> Instruction {
    let (contract, _) = contract_address(site, payer);
    let mut buf = Vec::with_capacity(10);
    buf.push(TAG_APPROVE_CHECKED);
    buf.extend_from_slice(&amount.to_le_bytes());
    buf.push(decimals);
    Instruction {
        program_id: *token_program,
        accounts: vec![
            AccountMeta::new(*payer_token_account, false),
            AccountMeta::new_readonly(*mint, false),
            AccountMeta::new_readonly(contract, false),
            AccountMeta::new_readonly(*payer, true),
        ],
        data: buf,
    }
}

/// Withdraw the authorization. Worth pairing with `close_contract`.
pub fn revoke(token_program: &Pubkey, payer_token_account: &Pubkey, payer: &Pubkey) -> Instruction {
    Instruction {
        program_id: *token_program,
        accounts: vec![
            AccountMeta::new(*payer_token_account, false),
            AccountMeta::new_readonly(*payer, true),
        ],
        data: vec![TAG_REVOKE],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    fn expect(name: &str) -> [u8; 8] {
        let mut h = Sha256::new();
        h.update(format!("global:{name}").as_bytes());
        let out = h.finalize();
        let mut d = [0u8; 8];
        d.copy_from_slice(&out[..8]);
        d
    }

    #[test]
    fn discriminators_match_instruction_names() {
        assert_eq!(discriminator::INITIALIZE_SITE, expect("initialize_site"));
        assert_eq!(discriminator::OPEN_CONTRACT, expect("open_contract"));
        assert_eq!(discriminator::METER_AND_SETTLE, expect("meter_and_settle"));
        assert_eq!(discriminator::RENEW_CONTRACT, expect("renew_contract"));
        assert_eq!(discriminator::CLOSE_CONTRACT, expect("close_contract"));
    }

    #[test]
    fn meter_data_is_discriminator_plus_le_u32() {
        let k = PAY_ON_CHAIN_ID;
        let ix = meter_and_settle(&k, &k, &k, &k, &k, &k, &TOKEN_PROGRAM_ID, 3);
        assert_eq!(&ix.data[..8], &discriminator::METER_AND_SETTLE);
        assert_eq!(&ix.data[8..], &3u32.to_le_bytes());
        assert_eq!(ix.accounts.len(), 8);
        assert!(ix.accounts[1].is_signer, "authority signs");
        assert!(ix.accounts[3].is_writable, "contract is written");
    }
}
