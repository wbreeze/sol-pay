//! Instruction builders. Pure functions over addresses and amounts; no I/O,
//! no signing, no browser. Account order in every builder mirrors the field
//! order of the matching `#[derive(Accounts)]` struct, which is what Anchor
//! expects.
//!
//! Each builder exists twice: as a method on [`Program`], which supplies both
//! the deployment's address and the site's token program, and as a free
//! function against the canonical deployment on SPL Token. The free ones are
//! the methods with [`Program::default`] filled in.

use borsh::BorshSerialize;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

use super::ids::*;
use super::program::Program;

/// Anchor discriminators: the first eight bytes of sha256("global:<name>").
/// Precomputed so the client needs no hash dependency; `tests` below
/// recomputes them, so a renamed instruction fails the test rather than
/// silently building a call nobody answers.
///
/// These do not vary by deployment: they come from the instruction's name in
/// the source, not from the address it is deployed at.
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

impl Program {
    pub fn initialize_site(
        &self,
        authority: &Pubkey,
        mint: &Pubkey,
        treasury: &Pubkey,
        page_price: u64,
        collection_threshold: u64,
        min_limit: u64,
    ) -> Instruction {
        let (site, _) = self.site_address(authority);
        Instruction {
            program_id: self.id(),
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

    /// Must be preceded in the same transaction by [`Program::approve_checked`]
    /// naming the contract PDA as delegate for at least `limit`.
    pub fn open_contract(
        &self,
        site: &Pubkey,
        payer: &Pubkey,
        payer_token_account: &Pubkey,
        limit: u64,
    ) -> Instruction {
        let (contract, _) = self.contract_address(site, payer);
        Instruction {
            program_id: self.id(),
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
        &self,
        site: &Pubkey,
        authority: &Pubkey,
        payer: &Pubkey,
        payer_token_account: &Pubkey,
        treasury: &Pubkey,
        mint: &Pubkey,
        page_views: u32,
    ) -> Instruction {
        let (contract, _) = self.contract_address(site, payer);
        Instruction {
            program_id: self.id(),
            accounts: vec![
                AccountMeta::new_readonly(*site, false),
                AccountMeta::new_readonly(*authority, true),
                AccountMeta::new_readonly(*payer, false),
                AccountMeta::new(contract, false),
                AccountMeta::new(*payer_token_account, false),
                AccountMeta::new(*treasury, false),
                AccountMeta::new_readonly(*mint, false),
                AccountMeta::new_readonly(self.token_program(), false),
            ],
            data: data(
                discriminator::METER_AND_SETTLE,
                &MeterAndSettleArgs { page_views },
            ),
        }
    }

    /// Must be preceded by [`Program::approve_checked`] for at least
    /// `new_limit`.
    pub fn renew_contract(
        &self,
        site: &Pubkey,
        payer: &Pubkey,
        payer_token_account: &Pubkey,
        new_limit: u64,
    ) -> Instruction {
        let (contract, _) = self.contract_address(site, payer);
        Instruction {
            program_id: self.id(),
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

    pub fn close_contract(&self, site: &Pubkey, payer: &Pubkey) -> Instruction {
        let (contract, _) = self.contract_address(site, payer);
        Instruction {
            program_id: self.id(),
            accounts: vec![
                AccountMeta::new(*payer, true),
                AccountMeta::new_readonly(*site, false),
                AccountMeta::new(contract, false),
            ],
            data: discriminator::CLOSE_CONTRACT.to_vec(),
        }
    }

    /// The authorization step: let the contract PDA move up to `amount` of the
    /// payer's tokens. `approve` *replaces* any previous allowance, it does not
    /// add to it, so renewal passes the new limit outright.
    ///
    /// An SPL Token instruction on both counts the handle carries: it goes to
    /// this handle's token program, and the delegate it names is this
    /// deployment's contract PDA.
    pub fn approve_checked(
        &self,
        payer_token_account: &Pubkey,
        mint: &Pubkey,
        payer: &Pubkey,
        site: &Pubkey,
        amount: u64,
        decimals: u8,
    ) -> Instruction {
        let (contract, _) = self.contract_address(site, payer);
        let mut buf = Vec::with_capacity(10);
        buf.push(TAG_APPROVE_CHECKED);
        buf.extend_from_slice(&amount.to_le_bytes());
        buf.push(decimals);
        Instruction {
            program_id: self.token_program(),
            accounts: vec![
                AccountMeta::new(*payer_token_account, false),
                AccountMeta::new_readonly(*mint, false),
                AccountMeta::new_readonly(contract, false),
                AccountMeta::new_readonly(*payer, true),
            ],
            data: buf,
        }
    }

    /// Withdraw the authorization. Worth pairing with
    /// [`Program::close_contract`].
    ///
    /// Names only the token account and its owner, so it says nothing about
    /// which deployment held the allowance -- but it must go to the right
    /// token program, and that is on the handle, so this is a method like the
    /// rest.
    pub fn revoke(&self, payer_token_account: &Pubkey, payer: &Pubkey) -> Instruction {
        Instruction {
            program_id: self.token_program(),
            accounts: vec![
                AccountMeta::new(*payer_token_account, false),
                AccountMeta::new_readonly(*payer, true),
            ],
            data: vec![TAG_REVOKE],
        }
    }
}

// --- the canonical deployment, on SPL Token ------------------------------

pub fn initialize_site(
    authority: &Pubkey,
    mint: &Pubkey,
    treasury: &Pubkey,
    page_price: u64,
    collection_threshold: u64,
    min_limit: u64,
) -> Instruction {
    Program::default().initialize_site(
        authority,
        mint,
        treasury,
        page_price,
        collection_threshold,
        min_limit,
    )
}

/// Must be preceded in the same transaction by [`approve_checked`] naming the
/// contract PDA as delegate for at least `limit`.
pub fn open_contract(
    site: &Pubkey,
    payer: &Pubkey,
    payer_token_account: &Pubkey,
    limit: u64,
) -> Instruction {
    Program::default().open_contract(site, payer, payer_token_account, limit)
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
    page_views: u32,
) -> Instruction {
    Program::default().meter_and_settle(
        site,
        authority,
        payer,
        payer_token_account,
        treasury,
        mint,
        page_views,
    )
}

/// Must be preceded by [`approve_checked`] for at least `new_limit`.
pub fn renew_contract(
    site: &Pubkey,
    payer: &Pubkey,
    payer_token_account: &Pubkey,
    new_limit: u64,
) -> Instruction {
    Program::default().renew_contract(site, payer, payer_token_account, new_limit)
}

pub fn close_contract(site: &Pubkey, payer: &Pubkey) -> Instruction {
    Program::default().close_contract(site, payer)
}

/// The authorization step: let the contract PDA move up to `amount` of the
/// payer's tokens. `approve` *replaces* any previous allowance, it does not
/// add to it, so renewal passes the new limit outright.
pub fn approve_checked(
    payer_token_account: &Pubkey,
    mint: &Pubkey,
    payer: &Pubkey,
    site: &Pubkey,
    amount: u64,
    decimals: u8,
) -> Instruction {
    Program::default().approve_checked(payer_token_account, mint, payer, site, amount, decimals)
}

/// Withdraw the authorization. Worth pairing with `close_contract`.
pub fn revoke(payer_token_account: &Pubkey, payer: &Pubkey) -> Instruction {
    Program::default().revoke(payer_token_account, payer)
}

// --- SPL Token wire tags --------------------------------------------------
//
// `approve_checked` and `revoke` above are hand-encoded rather than pulled
// from spl-token, which drags a large dependency tree into a WASM bundle for
// two instructions. These tags are stable parts of the SPL Token ABI, and the
// parity tests check the bytes against spl-token itself.

const TAG_APPROVE_CHECKED: u8 = 13;
const TAG_REVOKE: u8 = 5;

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

    fn k(b: u8) -> Pubkey {
        Pubkey::new_from_array([b; 32])
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
        let p = PAY_ON_CHAIN_ID;
        let ix = meter_and_settle(&p, &p, &p, &p, &p, &p, 3);
        assert_eq!(&ix.data[..8], &discriminator::METER_AND_SETTLE);
        assert_eq!(&ix.data[8..], &3u32.to_le_bytes());
        assert_eq!(ix.accounts.len(), 8);
        assert!(ix.accounts[1].is_signer, "authority signs");
        assert!(ix.accounts[3].is_writable, "contract is written");
    }

    /// Every program instruction carries the deployment's own address, and
    /// the free functions carry the canonical one.
    #[test]
    fn instructions_are_stamped_with_the_deployment_that_built_them() {
        let mine = Program::new(k(9));
        for ix in [
            mine.initialize_site(&k(1), &k(2), &k(3), 10, 100, 50),
            mine.open_contract(&k(1), &k(2), &k(3), 500),
            mine.meter_and_settle(&k(1), &k(2), &k(3), &k(4), &k(5), &k(6), 3),
            mine.renew_contract(&k(1), &k(2), &k(3), 900),
            mine.close_contract(&k(1), &k(2)),
        ] {
            assert_eq!(ix.program_id, k(9));
        }
        assert_eq!(close_contract(&k(1), &k(2)).program_id, PAY_ON_CHAIN_ID);
    }

    /// The delegate an approval names is a PDA, so it moves with the
    /// deployment even though the instruction itself belongs to SPL Token.
    #[test]
    fn an_approval_delegates_to_the_deployments_own_contract_pda() {
        let mine = Program::new(k(9));
        let ix = mine.approve_checked(&k(1), &k(2), &k(3), &k(4), 500, 6);
        assert_eq!(ix.program_id, TOKEN_PROGRAM_ID, "still an SPL instruction");
        assert_eq!(ix.accounts[2].pubkey, mine.contract_address(&k(4), &k(3)).0);
        assert_ne!(
            ix.accounts[2].pubkey,
            approve_checked(&k(1), &k(2), &k(3), &k(4), 500, 6).accounts[2].pubkey,
            "a different deployment delegates to a different PDA"
        );
    }

    /// The three instructions that address a token program take it from the
    /// handle, and nothing else on them moves when it changes.
    #[test]
    fn the_token_program_follows_the_handle() {
        let spl = Program::default();
        let t22 = spl.with_token_program(TOKEN_2022_PROGRAM_ID);

        assert_eq!(
            t22.approve_checked(&k(1), &k(2), &k(3), &k(4), 500, 6).program_id,
            TOKEN_2022_PROGRAM_ID
        );
        assert_eq!(t22.revoke(&k(1), &k(3)).program_id, TOKEN_2022_PROGRAM_ID);

        // On meter_and_settle it is an account, not the program being called:
        // the metering program CPIs into it.
        let m = t22.meter_and_settle(&k(1), &k(2), &k(3), &k(4), &k(5), &k(6), 3);
        assert_eq!(m.program_id, PAY_ON_CHAIN_ID, "still our program");
        assert_eq!(*m.accounts.last().map(|a| &a.pubkey).unwrap(), TOKEN_2022_PROGRAM_ID);

        // Same deployment, same PDAs: only the token program moved.
        assert_eq!(
            t22.contract_address(&k(4), &k(3)),
            spl.contract_address(&k(4), &k(3))
        );
        assert_eq!(
            t22.approve_checked(&k(1), &k(2), &k(3), &k(4), 500, 6).data,
            spl.approve_checked(&k(1), &k(2), &k(3), &k(4), 500, 6).data
        );
    }

    #[test]
    fn the_free_functions_are_the_canonical_deployment_on_spl_token() {
        let c = Program::default();
        assert_eq!(
            open_contract(&k(1), &k(2), &k(3), 500),
            c.open_contract(&k(1), &k(2), &k(3), 500)
        );
        assert_eq!(close_contract(&k(1), &k(2)), c.close_contract(&k(1), &k(2)));
        assert_eq!(revoke(&k(1), &k(3)), c.revoke(&k(1), &k(3)));
        assert_eq!(revoke(&k(1), &k(3)).program_id, TOKEN_PROGRAM_ID);
    }
}
