//! Test fixture: an in-process SVM with the program loaded, a mint, a reader
//! with tokens in their own account, a browser key, and a site already
//! configured. The clock is pinned to `NOW`, so expiries are exact.
//!
//! Instructions are built from the program's own generated `accounts::` and
//! `instruction::` types, so a change to an account struct breaks these tests
//! at compile time rather than producing a call that fails mysteriously.

use std::path::PathBuf;

use anchor_lang::prelude::Clock;
use anchor_lang::{system_program, AccountDeserialize, InstructionData, ToAccountMetas};
use anchor_spl::token::spl_token;
use litesvm::LiteSVM;
use solana_account::Account;
use solana_instruction::{AccountMeta, Instruction};
use solana_keypair::Keypair;
use solana_program_option::COption;
use solana_program_pack::Pack;
use solana_pubkey::{pubkey, Pubkey};
use solana_signer::Signer;
use solana_transaction::Transaction;

use pay_on_chain::state::{Fund, Meter};

pub const DECIMALS: u8 = 6;
/// 0.001 USDC per item.
pub const ITEM_PRICE: u64 = 1_000;
/// Collect once 0.05 USDC has accrued.
pub const THRESHOLD: u64 = 50_000;
pub const MIN_LIMIT: u64 = 200_000;
/// Items that fit under the threshold without triggering a settle.
pub const ITEMS_TO_THRESHOLD: u32 = (THRESHOLD / ITEM_PRICE) as u32;

/// The cluster's clock, as far as these tests are concerned.
pub const NOW: i64 = 1_800_000_000;
pub const HOUR: i64 = 3_600;

pub const ASSOCIATED_TOKEN_PROGRAM_ID: Pubkey =
    pubkey!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");

pub struct Env {
    pub svm: LiteSVM,
    pub authority: Keypair,
    pub reader: Keypair,
    /// The browser key the meter names.
    pub key: Keypair,
    pub mint: Pubkey,
    pub site: Pubkey,
    pub treasury: Pubkey,
    /// The reader's own token account, where deposits come from.
    pub reader_ata: Pubkey,
    /// Fund index 0 in `mint`, and its token account.
    pub fund: Pubkey,
    pub fund_ata: Pubkey,
    /// What `open` deposits: the whole of the reader's tokens.
    pub deposit: u64,
    /// What `open` and `renew` name as the expiry.
    pub expiry: i64,
}

fn program_so() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../target/deploy/pay_on_chain.so")
}

fn funded(data: Vec<u8>, owner: Pubkey) -> Account {
    Account {
        lamports: 1_000_000_000,
        data,
        owner,
        executable: false,
        rent_epoch: 0,
    }
}

fn mint_data(authority: &Pubkey) -> Vec<u8> {
    let mut data = vec![0u8; spl_token::state::Mint::LEN];
    spl_token::state::Mint {
        mint_authority: COption::Some(*authority),
        supply: 0,
        decimals: DECIMALS,
        is_initialized: true,
        freeze_authority: COption::None,
    }
    .pack_into_slice(&mut data);
    data
}

fn token_account_data(mint: &Pubkey, owner: &Pubkey, amount: u64) -> Vec<u8> {
    let mut data = vec![0u8; spl_token::state::Account::LEN];
    spl_token::state::Account {
        mint: *mint,
        owner: *owner,
        amount,
        delegate: COption::None,
        state: spl_token::state::AccountState::Initialized,
        is_native: COption::None,
        delegated_amount: 0,
        close_authority: COption::None,
    }
    .pack_into_slice(&mut data);
    data
}

// --- address derivation, mirroring the program's seeds --------------------

pub fn site_pda(authority: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[b"site", authority.as_ref()], &pay_on_chain::ID).0
}

pub fn fund_pda(reader: &Pubkey, mint: &Pubkey, index: u8) -> Pubkey {
    Pubkey::find_program_address(
        &[b"fund", reader.as_ref(), mint.as_ref(), &[index]],
        &pay_on_chain::ID,
    )
    .0
}

/// The associated token account formula, written out: owner, token program,
/// mint, under the ATA program.
pub fn fund_ata_of(fund: &Pubkey, mint: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[fund.as_ref(), spl_token::ID.as_ref(), mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    )
    .0
}

pub fn meter_pda(site: &Pubkey, fund: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[b"meter", site.as_ref(), fund.as_ref()],
        &pay_on_chain::ID,
    )
    .0
}

/// A second site on the same deployment, for the tests about one fund
/// serving several.
pub struct OtherSite {
    pub authority: Keypair,
    pub site: Pubkey,
    pub treasury: Pubkey,
}

impl Env {
    /// `deposit` is the reader's token balance, and what `open` moves into
    /// the fund -- so it is what a settle can actually draw on.
    pub fn new(deposit: u64) -> Self {
        let mut svm = LiteSVM::new();
        svm.add_program_from_file(pay_on_chain::ID, program_so())
            .expect("run `anchor build` first: target/deploy/pay_on_chain.so is missing");

        let authority = Keypair::new();
        let reader = Keypair::new();
        svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();
        svm.airdrop(&reader.pubkey(), 10_000_000_000).unwrap();

        let mint = Pubkey::new_unique();
        svm.set_account(mint, funded(mint_data(&authority.pubkey()), spl_token::ID))
            .unwrap();

        let treasury = Pubkey::new_unique();
        svm.set_account(
            treasury,
            funded(token_account_data(&mint, &authority.pubkey(), 0), spl_token::ID),
        )
        .unwrap();

        let reader_ata = Pubkey::new_unique();
        svm.set_account(
            reader_ata,
            funded(token_account_data(&mint, &reader.pubkey(), deposit), spl_token::ID),
        )
        .unwrap();

        let site = site_pda(&authority.pubkey());
        let fund = fund_pda(&reader.pubkey(), &mint, 0);
        let fund_ata = fund_ata_of(&fund, &mint);

        let mut env = Env {
            svm,
            authority,
            reader,
            key: Keypair::new(),
            mint,
            site,
            treasury,
            reader_ata,
            fund,
            fund_ata,
            deposit,
            expiry: NOW + HOUR,
        };
        env.set_now(NOW);

        let ix = env.ix_initialize_site(&env.authority.pubkey(), &env.treasury);
        let authority = env.authority.insecure_clone();
        env.send(&[ix], &[&authority], &authority.pubkey())
            .expect("site setup");

        env
    }

    pub fn set_now(&mut self, unix_timestamp: i64) {
        let mut clock: Clock = self.svm.get_sysvar();
        clock.unix_timestamp = unix_timestamp;
        self.svm.set_sysvar(&clock);
    }

    /// Another site, in the same mint unless told otherwise.
    pub fn add_site(&mut self) -> OtherSite {
        let authority = Keypair::new();
        self.svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();
        let treasury = Pubkey::new_unique();
        self.svm
            .set_account(
                treasury,
                funded(
                    token_account_data(&self.mint, &authority.pubkey(), 0),
                    spl_token::ID,
                ),
            )
            .unwrap();
        let ix = self.ix_initialize_site(&authority.pubkey(), &treasury);
        let signer = authority.insecure_clone();
        self.send(&[ix], &[&signer], &signer.pubkey())
            .expect("second site setup");
        OtherSite {
            site: site_pda(&authority.pubkey()),
            authority,
            treasury,
        }
    }

    /// A second mint, and a token account in it for the reader holding
    /// `amount`. Returns (mint, reader's token account).
    pub fn add_mint(&mut self, amount: u64) -> (Pubkey, Pubkey) {
        let mint = Pubkey::new_unique();
        self.svm
            .set_account(mint, funded(mint_data(&self.authority.pubkey()), spl_token::ID))
            .unwrap();
        let ata = Pubkey::new_unique();
        self.svm
            .set_account(
                ata,
                funded(token_account_data(&mint, &self.reader.pubkey(), amount), spl_token::ID),
            )
            .unwrap();
        (mint, ata)
    }

    /// A token account nobody in particular owns, for withdrawals to land in.
    pub fn empty_token_account(&mut self, owner: &Pubkey) -> Pubkey {
        let addr = Pubkey::new_unique();
        self.svm
            .set_account(addr, funded(token_account_data(&self.mint, owner, 0), spl_token::ID))
            .unwrap();
        addr
    }

    pub fn send(
        &mut self,
        ixs: &[Instruction],
        signers: &[&Keypair],
        fee_payer: &Pubkey,
    ) -> Result<(), String> {
        // LiteSVM holds the blockhash steady until told otherwise, so two
        // identical transactions — a loop of single-item meter calls, say —
        // would carry the same signature and the second would be rejected as
        // AlreadyProcessed. Advance it so every send is distinct.
        self.svm.expire_blockhash();

        let tx = Transaction::new_signed_with_payer(
            ixs,
            Some(fee_payer),
            signers,
            self.svm.latest_blockhash(),
        );
        self.svm.send_transaction(tx).map(|_| ()).map_err(|e| {
            // Keep the logs: Anchor writes "Error Code: <Name>" into them,
            // which is a steadier assertion target than an error number.
            format!("{:?} logs={:?}", e.err, e.meta.logs)
        })
    }

    /// Signed and paid for by the reader, the way the wallet sends the setup
    /// transaction.
    pub fn as_reader(&mut self, ixs: &[Instruction]) -> Result<(), String> {
        let reader = self.reader.insecure_clone();
        self.send(ixs, &[&reader], &reader.pubkey())
    }

    // --- reading state ---------------------------------------------------

    pub fn meter_addr(&self) -> Pubkey {
        meter_pda(&self.site, &self.fund)
    }

    pub fn meter_account(&self) -> Meter {
        let acct = self.svm.get_account(&self.meter_addr()).expect("meter account");
        Meter::try_deserialize(&mut acct.data.as_slice()).expect("meter deserializes")
    }

    pub fn meter_exists(&self) -> bool {
        self.exists(&self.meter_addr())
    }

    pub fn fund_account(&self) -> Fund {
        let acct = self.svm.get_account(&self.fund).expect("fund account");
        Fund::try_deserialize(&mut acct.data.as_slice()).expect("fund deserializes")
    }

    pub fn exists(&self, addr: &Pubkey) -> bool {
        self.svm
            .get_account(addr)
            .map(|a| a.lamports > 0 || !a.data.is_empty())
            .unwrap_or(false)
    }

    pub fn token_balance(&self, addr: &Pubkey) -> u64 {
        let acct = self.svm.get_account(addr).expect("token account");
        spl_token::state::Account::unpack(&acct.data).expect("unpacks").amount
    }

    pub fn lamports(&self, addr: &Pubkey) -> u64 {
        self.svm.get_account(addr).map(|a| a.lamports).unwrap_or(0)
    }

    // --- instruction builders --------------------------------------------

    pub fn ix_initialize_site(&self, authority: &Pubkey, treasury: &Pubkey) -> Instruction {
        Instruction {
            program_id: pay_on_chain::ID,
            accounts: pay_on_chain::accounts::InitializeSite {
                authority: *authority,
                site: site_pda(authority),
                mint: self.mint,
                treasury: *treasury,
                system_program: system_program::ID,
            }
            .to_account_metas(None),
            data: pay_on_chain::instruction::InitializeSite {
                item_price: ITEM_PRICE,
                collection_threshold: THRESHOLD,
                min_limit: MIN_LIMIT,
            }
            .data(),
        }
    }

    pub fn ix_open_fund(&self) -> Instruction {
        self.ix_open_fund_in(&self.mint, 0)
    }

    pub fn ix_open_fund_in(&self, mint: &Pubkey, index: u8) -> Instruction {
        let fund = fund_pda(&self.reader.pubkey(), mint, index);
        Instruction {
            program_id: pay_on_chain::ID,
            accounts: pay_on_chain::accounts::OpenFund {
                reader: self.reader.pubkey(),
                fund,
                fund_token_account: fund_ata_of(&fund, mint),
                mint: *mint,
                token_program: spl_token::ID,
                associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
                system_program: system_program::ID,
            }
            .to_account_metas(None),
            data: pay_on_chain::instruction::OpenFund { index }.data(),
        }
    }

    /// An SPL transfer from the reader's own account into the fund's.
    pub fn ix_deposit(&self, amount: u64) -> Instruction {
        spl_token::instruction::transfer_checked(
            &spl_token::ID,
            &self.reader_ata,
            &self.mint,
            &self.fund_ata,
            &self.reader.pubkey(),
            &[],
            amount,
            DECIMALS,
        )
        .unwrap()
    }

    pub fn ix_withdraw(&self, destination: &Pubkey, amount: u64) -> Instruction {
        Instruction {
            program_id: pay_on_chain::ID,
            accounts: pay_on_chain::accounts::Withdraw {
                reader: self.reader.pubkey(),
                fund: self.fund,
                fund_token_account: self.fund_ata,
                destination: *destination,
                mint: self.mint,
                token_program: spl_token::ID,
            }
            .to_account_metas(None),
            data: pay_on_chain::instruction::Withdraw { amount }.data(),
        }
    }

    pub fn ix_close_fund(&self) -> Instruction {
        Instruction {
            program_id: pay_on_chain::ID,
            accounts: pay_on_chain::accounts::CloseFund {
                reader: self.reader.pubkey(),
                fund: self.fund,
                fund_token_account: self.fund_ata,
                token_program: spl_token::ID,
            }
            .to_account_metas(None),
            data: pay_on_chain::instruction::CloseFund {}.data(),
        }
    }

    pub fn ix_open(&self, limit: u64) -> Instruction {
        self.ix_open_at(&self.site, &self.fund, &self.key.pubkey(), limit, self.expiry)
    }

    pub fn ix_open_at(
        &self,
        site: &Pubkey,
        fund: &Pubkey,
        key: &Pubkey,
        limit: u64,
        expiry: i64,
    ) -> Instruction {
        Instruction {
            program_id: pay_on_chain::ID,
            accounts: pay_on_chain::accounts::OpenMeter {
                reader: self.reader.pubkey(),
                site: *site,
                fund: *fund,
                meter: meter_pda(site, fund),
                system_program: system_program::ID,
            }
            .to_account_metas(None),
            data: pay_on_chain::instruction::OpenMeter {
                key: *key,
                limit,
                expiry,
            }
            .data(),
        }
    }

    pub fn ix_meter(&self, items: u32) -> Instruction {
        self.ix_meter_at(&self.site, &self.authority.pubkey(), &self.treasury, items)
    }

    pub fn ix_meter_at(
        &self,
        site: &Pubkey,
        authority: &Pubkey,
        treasury: &Pubkey,
        items: u32,
    ) -> Instruction {
        Instruction {
            program_id: pay_on_chain::ID,
            accounts: pay_on_chain::accounts::MeterAndSettle {
                site: *site,
                authority: *authority,
                fund: self.fund,
                meter: meter_pda(site, &self.fund),
                fund_token_account: self.fund_ata,
                treasury: *treasury,
                mint: self.mint,
                token_program: spl_token::ID,
            }
            .to_account_metas(None),
            data: pay_on_chain::instruction::MeterAndSettle { items }.data(),
        }
    }

    pub fn ix_renew(&self, new_limit: u64) -> Instruction {
        self.ix_renew_with(&self.key.pubkey(), new_limit, self.expiry)
    }

    pub fn ix_renew_with(&self, key: &Pubkey, new_limit: u64, expiry: i64) -> Instruction {
        Instruction {
            program_id: pay_on_chain::ID,
            accounts: pay_on_chain::accounts::RenewMeter {
                reader: self.reader.pubkey(),
                site: self.site,
                fund: self.fund,
                meter: self.meter_addr(),
            }
            .to_account_metas(None),
            data: pay_on_chain::instruction::RenewMeter {
                key: *key,
                new_limit,
                expiry,
            }
            .data(),
        }
    }

    /// `signer` must sign the transaction too; the rent goes to the reader
    /// whoever signs.
    pub fn ix_close(&self, signer: &Pubkey) -> Instruction {
        Instruction {
            program_id: pay_on_chain::ID,
            accounts: pay_on_chain::accounts::CloseMeter {
                signer: *signer,
                site: self.site,
                fund: self.fund,
                reader: self.reader.pubkey(),
                meter: self.meter_addr(),
            }
            .to_account_metas(None),
            data: pay_on_chain::instruction::CloseMeter {}.data(),
        }
    }

    /// The Associated Token Account program's own `CreateIdempotent`, which
    /// anyone may send for any owner.
    pub fn ix_create_ata(&self, payer: &Pubkey, owner: &Pubkey) -> Instruction {
        Instruction {
            program_id: ASSOCIATED_TOKEN_PROGRAM_ID,
            accounts: vec![
                AccountMeta::new(*payer, true),
                AccountMeta::new(fund_ata_of(owner, &self.mint), false),
                AccountMeta::new_readonly(*owner, false),
                AccountMeta::new_readonly(self.mint, false),
                AccountMeta::new_readonly(system_program::ID, false),
                AccountMeta::new_readonly(spl_token::ID, false),
            ],
            data: vec![1],
        }
    }

    // --- convenience -------------------------------------------------------

    /// The setup transaction a new reader's wallet signs (SPEC §4.9): open
    /// the fund, deposit into it, open the meter.
    pub fn open(&mut self, limit: u64) -> Result<(), String> {
        let ixs = [self.ix_open_fund(), self.ix_deposit(self.deposit), self.ix_open(limit)];
        self.as_reader(&ixs)
    }

    pub fn meter(&mut self, items: u32) -> Result<(), String> {
        let ix = self.ix_meter(items);
        let authority = self.authority.insecure_clone();
        self.send(&[ix], &[&authority], &authority.pubkey())
    }

    /// Sign-out: the browser key signs the close and, holding no SOL, leaves
    /// the fee to the site's server.
    pub fn sign_out(&mut self) -> Result<(), String> {
        let ix = self.ix_close(&self.key.pubkey());
        let key = self.key.insecure_clone();
        let server = self.authority.insecure_clone();
        self.send(&[ix], &[&server, &key], &server.pubkey())
    }
}

/// Anchor writes `Error Code: <Name>` into the logs; assert on that rather
/// than on a numeric code that shifts when the enum is reordered.
pub fn assert_error(result: Result<(), String>, code: &str) {
    match result {
        Ok(()) => panic!("expected {code}, transaction succeeded"),
        Err(e) => assert!(e.contains(code), "expected {code} in failure, got: {e}"),
    }
}
