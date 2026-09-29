//! Instruction builders. Pure functions over addresses and amounts; no I/O,
//! no signing, no browser. Account order in every builder mirrors the field
//! order of the matching `#[derive(Accounts)]` struct, which is what Anchor
//! expects.
//!
//! Each builder exists twice: as a method on [`Program`], which supplies both
//! the deployment's address and the site's token program, and as a free
//! function against the canonical deployment on SPL Token. The free ones are
//! the methods with [`Program::default`] filled in.
//!
//! Who signs what (SPEC §3): the site authority signs `initialize_site` and
//! `meter_and_settle`; the reader's wallet signs `open_fund`, `deposit`,
//! `withdraw`, `close_fund`, `open_meter` and `renew_meter`; `close_meter` is
//! signed by the reader or by the meter's browser key. The server builds all
//! of them, and composes the reader's into the one transaction the wallet
//! fetches (SPEC §4.9).

use borsh::BorshSerialize;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

use super::ids::*;
use super::program::Program;

/// Anchor discriminators: the first eight bytes of `sha256("global:<name>")`.
/// Precomputed so the client needs no hash dependency; `tests` below
/// recomputes them, so a renamed instruction fails the test rather than
/// silently building a call nobody answers.
///
/// These do not vary by deployment: they come from the instruction's name in
/// the source, not from the address it is deployed at.
pub mod discriminator {
    pub const INITIALIZE_SITE: [u8; 8] = [85, 52, 128, 208, 7, 224, 178, 79];
    pub const OPEN_FUND: [u8; 8] = [121, 233, 204, 29, 232, 237, 166, 30];
    pub const WITHDRAW: [u8; 8] = [183, 18, 70, 156, 148, 109, 161, 34];
    pub const CLOSE_FUND: [u8; 8] = [230, 183, 3, 112, 236, 252, 5, 185];
    pub const OPEN_METER: [u8; 8] = [55, 71, 55, 126, 38, 38, 60, 122];
    pub const METER_AND_SETTLE: [u8; 8] = [139, 17, 0, 139, 114, 233, 88, 121];
    pub const RENEW_METER: [u8; 8] = [247, 168, 99, 108, 19, 183, 238, 115];
    pub const CLOSE_METER: [u8; 8] = [102, 64, 197, 208, 191, 80, 153, 160];
}

fn data(disc: [u8; 8], args: &impl BorshSerialize) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + 48);
    out.extend_from_slice(&disc);
    args.serialize(&mut out).expect("borsh into Vec cannot fail");
    out
}

#[derive(BorshSerialize)]
struct InitializeSiteArgs {
    item_price: u64,
    collection_threshold: u64,
    min_limit: u64,
}

#[derive(BorshSerialize)]
struct OpenFundArgs {
    index: u8,
}

#[derive(BorshSerialize)]
struct WithdrawArgs {
    amount: u64,
}

/// `key` is borsh-encoded as its 32 bytes, which is how Anchor encodes a
/// `Pubkey` argument; taking the array keeps borsh off `solana-pubkey`.
#[derive(BorshSerialize)]
struct OpenMeterArgs {
    key: [u8; 32],
    limit: u64,
    expiry: i64,
}

#[derive(BorshSerialize)]
struct MeterAndSettleArgs {
    items: u32,
}

#[derive(BorshSerialize)]
struct RenewMeterArgs {
    key: [u8; 32],
    new_limit: u64,
    expiry: i64,
}

impl Program {
    pub fn initialize_site(
        &self,
        authority: &Pubkey,
        mint: &Pubkey,
        treasury: &Pubkey,
        item_price: u64,
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
                    item_price,
                    collection_threshold,
                    min_limit,
                },
            ),
        }
    }

    /// Create the reader's fund at `index`, and its token account. Must come
    /// before any [`Program::deposit`] into it in the same transaction --
    /// [`Program::open_fund_and_deposit`] puts them in that order.
    pub fn open_fund(&self, reader: &Pubkey, mint: &Pubkey, index: u8) -> Instruction {
        let (fund, _) = self.fund_address(reader, mint, index);
        Instruction {
            program_id: self.id(),
            accounts: vec![
                AccountMeta::new(*reader, true),
                AccountMeta::new(fund, false),
                AccountMeta::new(self.fund_token_account(&fund, mint), false),
                AccountMeta::new_readonly(*mint, false),
                AccountMeta::new_readonly(self.token_program(), false),
                AccountMeta::new_readonly(ASSOCIATED_TOKEN_PROGRAM_ID, false),
                AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
            ],
            data: data(discriminator::OPEN_FUND, &OpenFundArgs { index }),
        }
    }

    /// Extend a fund: an SPL `transfer_checked` from `source`, owned by
    /// `source_owner`, into the fund's token account. No instruction of the
    /// metering program is involved; this is the transfer, addressed.
    pub fn deposit(
        &self,
        source: &Pubkey,
        source_owner: &Pubkey,
        fund: &Pubkey,
        mint: &Pubkey,
        amount: u64,
        decimals: u8,
    ) -> Instruction {
        let mut buf = Vec::with_capacity(10);
        buf.push(TAG_TRANSFER_CHECKED);
        buf.extend_from_slice(&amount.to_le_bytes());
        buf.push(decimals);
        Instruction {
            program_id: self.token_program(),
            accounts: vec![
                AccountMeta::new(*source, false),
                AccountMeta::new_readonly(*mint, false),
                AccountMeta::new(self.fund_token_account(fund, mint), false),
                AccountMeta::new_readonly(*source_owner, true),
            ],
            data: buf,
        }
    }

    /// Move `amount` out of the fund to `destination`, any token account of
    /// the fund's mint. Signed by the reader.
    pub fn withdraw(
        &self,
        reader: &Pubkey,
        mint: &Pubkey,
        index: u8,
        destination: &Pubkey,
        amount: u64,
    ) -> Instruction {
        let (fund, _) = self.fund_address(reader, mint, index);
        Instruction {
            program_id: self.id(),
            accounts: vec![
                AccountMeta::new_readonly(*reader, true),
                AccountMeta::new_readonly(fund, false),
                AccountMeta::new(self.fund_token_account(&fund, mint), false),
                AccountMeta::new(*destination, false),
                AccountMeta::new_readonly(*mint, false),
                AccountMeta::new_readonly(self.token_program(), false),
            ],
            data: data(discriminator::WITHDRAW, &WithdrawArgs { amount }),
        }
    }

    /// Close an empty fund with no meters open. Rent returns to the reader.
    pub fn close_fund(&self, reader: &Pubkey, mint: &Pubkey, index: u8) -> Instruction {
        let (fund, _) = self.fund_address(reader, mint, index);
        Instruction {
            program_id: self.id(),
            accounts: vec![
                AccountMeta::new(*reader, true),
                AccountMeta::new(fund, false),
                AccountMeta::new(self.fund_token_account(&fund, mint), false),
                AccountMeta::new_readonly(self.token_program(), false),
            ],
            data: discriminator::CLOSE_FUND.to_vec(),
        }
    }

    /// Open a meter at `site`, drawing on `fund`, naming the browser's `key`,
    /// a `limit` and an `expiry` (Unix seconds). Signed by the reader, whose
    /// fund it must be; the fund's mint must be the site's.
    #[allow(clippy::too_many_arguments)]
    pub fn open_meter(
        &self,
        site: &Pubkey,
        reader: &Pubkey,
        fund: &Pubkey,
        key: &Pubkey,
        limit: u64,
        expiry: i64,
    ) -> Instruction {
        let (meter, _) = self.meter_address(site, fund);
        Instruction {
            program_id: self.id(),
            accounts: vec![
                AccountMeta::new(*reader, true),
                AccountMeta::new_readonly(*site, false),
                AccountMeta::new(*fund, false),
                AccountMeta::new(meter, false),
                AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
            ],
            data: data(
                discriminator::OPEN_METER,
                &OpenMeterArgs {
                    key: key.to_bytes(),
                    limit,
                    expiry,
                },
            ),
        }
    }

    /// Signed by the site authority. The reader is absent; the transfer, if
    /// the threshold is crossed, is authorized by the fund's seeds.
    #[allow(clippy::too_many_arguments)]
    pub fn meter_and_settle(
        &self,
        site: &Pubkey,
        authority: &Pubkey,
        fund: &Pubkey,
        treasury: &Pubkey,
        mint: &Pubkey,
        items: u32,
    ) -> Instruction {
        let (meter, _) = self.meter_address(site, fund);
        Instruction {
            program_id: self.id(),
            accounts: vec![
                AccountMeta::new_readonly(*site, false),
                AccountMeta::new_readonly(*authority, true),
                AccountMeta::new_readonly(*fund, false),
                AccountMeta::new(meter, false),
                AccountMeta::new(self.fund_token_account(fund, mint), false),
                AccountMeta::new(*treasury, false),
                AccountMeta::new_readonly(*mint, false),
                AccountMeta::new_readonly(self.token_program(), false),
            ],
            data: data(
                discriminator::METER_AND_SETTLE,
                &MeterAndSettleArgs { items },
            ),
        }
    }

    /// Renew with a new limit, and a key and expiry that may be new. Naming a
    /// new key is how a second device takes over the meter.
    #[allow(clippy::too_many_arguments)]
    pub fn renew_meter(
        &self,
        site: &Pubkey,
        reader: &Pubkey,
        fund: &Pubkey,
        key: &Pubkey,
        new_limit: u64,
        expiry: i64,
    ) -> Instruction {
        let (meter, _) = self.meter_address(site, fund);
        Instruction {
            program_id: self.id(),
            accounts: vec![
                AccountMeta::new_readonly(*reader, true),
                AccountMeta::new_readonly(*site, false),
                AccountMeta::new_readonly(*fund, false),
                AccountMeta::new(meter, false),
            ],
            data: data(
                discriminator::RENEW_METER,
                &RenewMeterArgs {
                    key: key.to_bytes(),
                    new_limit,
                    expiry,
                },
            ),
        }
    }

    /// Close a meter. `signer` is the reader or the meter's key; the rent goes
    /// to `reader` either way. Key-signed, it is sign-out, and the site's
    /// server pays the fee (SPEC §4.8).
    pub fn close_meter(
        &self,
        signer: &Pubkey,
        reader: &Pubkey,
        site: &Pubkey,
        fund: &Pubkey,
    ) -> Instruction {
        let (meter, _) = self.meter_address(site, fund);
        Instruction {
            program_id: self.id(),
            accounts: vec![
                AccountMeta::new_readonly(*signer, true),
                AccountMeta::new_readonly(*site, false),
                AccountMeta::new(*fund, false),
                AccountMeta::new(*reader, false),
                AccountMeta::new(meter, false),
            ],
            data: discriminator::CLOSE_METER.to_vec(),
        }
    }
}

// --- the canonical deployment, on SPL Token ------------------------------

pub fn initialize_site(
    authority: &Pubkey,
    mint: &Pubkey,
    treasury: &Pubkey,
    item_price: u64,
    collection_threshold: u64,
    min_limit: u64,
) -> Instruction {
    Program::default().initialize_site(
        authority,
        mint,
        treasury,
        item_price,
        collection_threshold,
        min_limit,
    )
}

pub fn open_fund(reader: &Pubkey, mint: &Pubkey, index: u8) -> Instruction {
    Program::default().open_fund(reader, mint, index)
}

pub fn deposit(
    source: &Pubkey,
    source_owner: &Pubkey,
    fund: &Pubkey,
    mint: &Pubkey,
    amount: u64,
    decimals: u8,
) -> Instruction {
    Program::default().deposit(source, source_owner, fund, mint, amount, decimals)
}

pub fn withdraw(
    reader: &Pubkey,
    mint: &Pubkey,
    index: u8,
    destination: &Pubkey,
    amount: u64,
) -> Instruction {
    Program::default().withdraw(reader, mint, index, destination, amount)
}

pub fn close_fund(reader: &Pubkey, mint: &Pubkey, index: u8) -> Instruction {
    Program::default().close_fund(reader, mint, index)
}

#[allow(clippy::too_many_arguments)]
pub fn open_meter(
    site: &Pubkey,
    reader: &Pubkey,
    fund: &Pubkey,
    key: &Pubkey,
    limit: u64,
    expiry: i64,
) -> Instruction {
    Program::default().open_meter(site, reader, fund, key, limit, expiry)
}

pub fn meter_and_settle(
    site: &Pubkey,
    authority: &Pubkey,
    fund: &Pubkey,
    treasury: &Pubkey,
    mint: &Pubkey,
    items: u32,
) -> Instruction {
    Program::default().meter_and_settle(site, authority, fund, treasury, mint, items)
}

#[allow(clippy::too_many_arguments)]
pub fn renew_meter(
    site: &Pubkey,
    reader: &Pubkey,
    fund: &Pubkey,
    key: &Pubkey,
    new_limit: u64,
    expiry: i64,
) -> Instruction {
    Program::default().renew_meter(site, reader, fund, key, new_limit, expiry)
}

pub fn close_meter(signer: &Pubkey, reader: &Pubkey, site: &Pubkey, fund: &Pubkey) -> Instruction {
    Program::default().close_meter(signer, reader, site, fund)
}

// --- SPL Token wire tags --------------------------------------------------
//
// `deposit` above is hand-encoded rather than pulled from spl-token, which
// drags a large dependency tree into a WASM bundle for one instruction. The
// tag is a stable part of the SPL Token ABI, and the parity tests check the
// bytes against spl-token itself.

const TAG_TRANSFER_CHECKED: u8 = 12;

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
        assert_eq!(discriminator::OPEN_FUND, expect("open_fund"));
        assert_eq!(discriminator::WITHDRAW, expect("withdraw"));
        assert_eq!(discriminator::CLOSE_FUND, expect("close_fund"));
        assert_eq!(discriminator::OPEN_METER, expect("open_meter"));
        assert_eq!(discriminator::METER_AND_SETTLE, expect("meter_and_settle"));
        assert_eq!(discriminator::RENEW_METER, expect("renew_meter"));
        assert_eq!(discriminator::CLOSE_METER, expect("close_meter"));
    }

    #[test]
    fn meter_data_is_discriminator_plus_le_u32() {
        let p = PAY_ON_CHAIN_ID;
        let ix = meter_and_settle(&p, &p, &p, &p, &p, 3);
        assert_eq!(&ix.data[..8], &discriminator::METER_AND_SETTLE);
        assert_eq!(&ix.data[8..], &3u32.to_le_bytes());
        assert_eq!(ix.accounts.len(), 8);
        assert!(ix.accounts[1].is_signer, "authority signs");
        assert!(!ix.accounts[2].is_writable, "the fund is only read");
        assert!(ix.accounts[3].is_writable, "meter is written");
        assert!(ix.accounts[4].is_writable, "the fund's token account pays");
    }

    /// The open arguments are the key's 32 bytes, then the limit and the
    /// expiry, little-endian: 56 bytes after the discriminator.
    #[test]
    fn open_meter_data_carries_key_limit_and_expiry() {
        let ix = open_meter(&k(1), &k(2), &k(3), &k(4), 500, -1);
        assert_eq!(ix.data.len(), 8 + 32 + 8 + 8);
        assert_eq!(&ix.data[8..40], &[4u8; 32]);
        assert_eq!(&ix.data[40..48], &500u64.to_le_bytes());
        assert_eq!(&ix.data[48..56], &(-1i64).to_le_bytes());
    }

    /// Every program instruction carries the deployment's own address, and
    /// the free functions carry the canonical one.
    #[test]
    fn instructions_are_stamped_with_the_deployment_that_built_them() {
        let mine = Program::new(k(9));
        for ix in [
            mine.initialize_site(&k(1), &k(2), &k(3), 10, 100, 50),
            mine.open_fund(&k(1), &k(2), 0),
            mine.withdraw(&k(1), &k(2), 0, &k(3), 5),
            mine.close_fund(&k(1), &k(2), 0),
            mine.open_meter(&k(1), &k(2), &k(3), &k(4), 500, 60),
            mine.meter_and_settle(&k(1), &k(2), &k(3), &k(4), &k(5), 3),
            mine.renew_meter(&k(1), &k(2), &k(3), &k(4), 900, 60),
            mine.close_meter(&k(1), &k(2), &k(3), &k(4)),
        ] {
            assert_eq!(ix.program_id, k(9));
        }
        assert_eq!(close_meter(&k(1), &k(2), &k(3), &k(4)).program_id, PAY_ON_CHAIN_ID);
    }

    /// A deposit is an SPL instruction into the fund's token account, which
    /// is a PDA's token account, so it moves with the deployment even though
    /// the instruction belongs to the token program.
    #[test]
    fn a_deposit_lands_in_the_deployments_own_fund() {
        let mine = Program::new(k(9));
        let (fund, _) = mine.fund_address(&k(3), &k(2), 0);
        let ix = mine.deposit(&k(1), &k(3), &fund, &k(2), 500, 6);
        assert_eq!(ix.program_id, TOKEN_PROGRAM_ID, "still an SPL instruction");
        assert_eq!(ix.accounts[2].pubkey, mine.fund_token_account(&fund, &k(2)));
        assert!(ix.accounts[3].is_signer, "the source's owner signs");
    }

    /// The instructions that address a token program take it from the handle,
    /// and nothing else on them moves when it changes.
    #[test]
    fn the_token_program_follows_the_handle() {
        let spl = Program::default();
        let t22 = spl.with_token_program(TOKEN_2022_PROGRAM_ID);
        let (fund, _) = t22.fund_address(&k(3), &k(2), 0);

        assert_eq!(
            t22.deposit(&k(1), &k(3), &fund, &k(2), 500, 6).program_id,
            TOKEN_2022_PROGRAM_ID
        );

        // On meter_and_settle it is an account, not the program being called:
        // the metering program CPIs into it.
        let m = t22.meter_and_settle(&k(1), &k(2), &fund, &k(5), &k(2), 3);
        assert_eq!(m.program_id, PAY_ON_CHAIN_ID, "still our program");
        assert_eq!(*m.accounts.last().map(|a| &a.pubkey).unwrap(), TOKEN_2022_PROGRAM_ID);
        assert_eq!(m.accounts[4].pubkey, t22.fund_token_account(&fund, &k(2)));

        let o = t22.open_fund(&k(3), &k(2), 0);
        assert_eq!(o.accounts[4].pubkey, TOKEN_2022_PROGRAM_ID);
    }

    #[test]
    fn the_free_functions_are_the_canonical_deployment_on_spl_token() {
        let c = Program::default();
        assert_eq!(
            open_meter(&k(1), &k(2), &k(3), &k(4), 500, 60),
            c.open_meter(&k(1), &k(2), &k(3), &k(4), 500, 60)
        );
        assert_eq!(
            close_meter(&k(1), &k(2), &k(3), &k(4)),
            c.close_meter(&k(1), &k(2), &k(3), &k(4))
        );
        assert_eq!(open_fund(&k(1), &k(2), 7), c.open_fund(&k(1), &k(2), 7));
        assert_eq!(
            deposit(&k(1), &k(2), &k(3), &k(4), 5, 6).program_id,
            TOKEN_PROGRAM_ID
        );
    }
}
