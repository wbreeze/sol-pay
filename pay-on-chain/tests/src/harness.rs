//! Test fixture: an in-process SVM with the program loaded, a mint, a funded
//! payer token account, and a site already configured.
//!
//! Instructions are built from the program's own generated `accounts::` and
//! `instruction::` types, so a change to an account struct breaks these tests
//! at compile time rather than producing a call that fails mysteriously.

use std::path::PathBuf;

use anchor_lang::{system_program, AccountDeserialize, InstructionData, ToAccountMetas};
use anchor_spl::token::spl_token;
use litesvm::LiteSVM;
use solana_account::Account;
use solana_instruction::Instruction;
use solana_keypair::Keypair;
use solana_program_option::COption;
use solana_program_pack::Pack;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use solana_transaction::Transaction;

use pay_on_chain::state::Contract;

pub const DECIMALS: u8 = 6;
/// 0.001 USDC per page view.
pub const PAGE_PRICE: u64 = 1_000;
/// Collect once 0.05 USDC has accrued.
pub const THRESHOLD: u64 = 50_000;
pub const MIN_LIMIT: u64 = 200_000;
/// Views that fit under the threshold without triggering a settle.
pub const VIEWS_TO_THRESHOLD: u32 = (THRESHOLD / PAGE_PRICE) as u32;

pub struct Env {
    pub svm: LiteSVM,
    pub authority: Keypair,
    pub payer: Keypair,
    pub mint: Pubkey,
    pub site: Pubkey,
    pub treasury: Pubkey,
    pub payer_ata: Pubkey,
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

pub fn contract_pda(site: &Pubkey, payer: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[b"contract", site.as_ref(), payer.as_ref()],
        &pay_on_chain::ID,
    )
    .0
}

pub fn slug_pda(site: &Pubkey, slug: &[u8; 16]) -> Pubkey {
    Pubkey::find_program_address(&[b"slug", site.as_ref(), slug], &pay_on_chain::ID).0
}

/// Distinct, readable slugs so a failure names which one.
pub fn slug(tag: u8) -> [u8; 16] {
    let mut s = [b'a'; 16];
    s[0] = tag;
    s
}

impl Env {
    /// `payer_balance` is the payer's token balance, which is what a settle
    /// actually draws on.
    pub fn new(payer_balance: u64) -> Self {
        let mut svm = LiteSVM::new();
        svm.add_program_from_file(pay_on_chain::ID, program_so())
            .expect("run `anchor build` first: target/deploy/pay_on_chain.so is missing");

        let authority = Keypair::new();
        let payer = Keypair::new();
        svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();
        svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();

        let mint = Pubkey::new_unique();
        svm.set_account(mint, funded(mint_data(&authority.pubkey()), spl_token::ID))
            .unwrap();

        let treasury = Pubkey::new_unique();
        svm.set_account(
            treasury,
            funded(
                token_account_data(&mint, &authority.pubkey(), 0),
                spl_token::ID,
            ),
        )
        .unwrap();

        let payer_ata = Pubkey::new_unique();
        svm.set_account(
            payer_ata,
            funded(
                token_account_data(&mint, &payer.pubkey(), payer_balance),
                spl_token::ID,
            ),
        )
        .unwrap();

        let site = site_pda(&authority.pubkey());

        let mut env = Env {
            svm,
            authority,
            payer,
            mint,
            site,
            treasury,
            payer_ata,
        };

        let ix = Instruction {
            program_id: pay_on_chain::ID,
            accounts: pay_on_chain::accounts::InitializeSite {
                authority: env.authority.pubkey(),
                site: env.site,
                mint: env.mint,
                treasury: env.treasury,
                system_program: system_program::ID,
            }
            .to_account_metas(None),
            data: pay_on_chain::instruction::InitializeSite {
                page_price: PAGE_PRICE,
                collection_threshold: THRESHOLD,
                min_limit: MIN_LIMIT,
            }
            .data(),
        };
        let authority = env.authority.insecure_clone();
        env.send(&[ix], &[&authority], &authority.pubkey())
            .expect("site setup");

        env
    }

    pub fn send(
        &mut self,
        ixs: &[Instruction],
        signers: &[&Keypair],
        fee_payer: &Pubkey,
    ) -> Result<(), String> {
        // LiteSVM holds the blockhash steady until told otherwise, so two
        // identical transactions — a loop of single-view meter calls, say —
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

    pub fn contract(&self) -> Contract {
        let addr = contract_pda(&self.site, &self.payer.pubkey());
        let acct = self.svm.get_account(&addr).expect("contract account");
        Contract::try_deserialize(&mut acct.data.as_slice()).expect("contract deserializes")
    }

    pub fn contract_exists(&self) -> bool {
        self.svm
            .get_account(&contract_pda(&self.site, &self.payer.pubkey()))
            .map(|a| !a.data.is_empty())
            .unwrap_or(false)
    }

    pub fn slug_resolves(&self, s: &[u8; 16]) -> bool {
        self.svm
            .get_account(&slug_pda(&self.site, s))
            .map(|a| !a.data.is_empty())
            .unwrap_or(false)
    }

    pub fn token_balance(&self, addr: &Pubkey) -> u64 {
        let acct = self.svm.get_account(addr).expect("token account");
        spl_token::state::Account::unpack(&acct.data).expect("unpacks").amount
    }

    pub fn delegated_amount(&self, addr: &Pubkey) -> u64 {
        let acct = self.svm.get_account(addr).expect("token account");
        spl_token::state::Account::unpack(&acct.data)
            .expect("unpacks")
            .delegated_amount
    }

    // --- instruction builders --------------------------------------------

    /// The authorization the whole design rests on: let the contract PDA pull
    /// up to `amount`. Must precede open/renew in the same transaction.
    pub fn ix_approve(&self, amount: u64) -> Instruction {
        self.ix_approve_to(&contract_pda(&self.site, &self.payer.pubkey()), amount)
    }

    pub fn ix_approve_to(&self, delegate: &Pubkey, amount: u64) -> Instruction {
        spl_token::instruction::approve_checked(
            &spl_token::ID,
            &self.payer_ata,
            &self.mint,
            delegate,
            &self.payer.pubkey(),
            &[],
            amount,
            DECIMALS,
        )
        .unwrap()
    }

    pub fn ix_open(&self, s: &[u8; 16], limit: u64) -> Instruction {
        Instruction {
            program_id: pay_on_chain::ID,
            accounts: pay_on_chain::accounts::OpenContract {
                payer: self.payer.pubkey(),
                site: self.site,
                contract: contract_pda(&self.site, &self.payer.pubkey()),
                slug_index: slug_pda(&self.site, s),
                payer_token_account: self.payer_ata,
                system_program: system_program::ID,
            }
            .to_account_metas(None),
            data: pay_on_chain::instruction::OpenContract {
                slug: *s,
                limit,
            }
            .data(),
        }
    }

    pub fn ix_meter(&self, page_views: u32) -> Instruction {
        Instruction {
            program_id: pay_on_chain::ID,
            accounts: pay_on_chain::accounts::MeterAndSettle {
                site: self.site,
                authority: self.authority.pubkey(),
                payer: self.payer.pubkey(),
                contract: contract_pda(&self.site, &self.payer.pubkey()),
                payer_token_account: self.payer_ata,
                treasury: self.treasury,
                mint: self.mint,
                token_program: spl_token::ID,
            }
            .to_account_metas(None),
            data: pay_on_chain::instruction::MeterAndSettle { page_views }.data(),
        }
    }

    pub fn ix_renew(&self, current: &[u8; 16], next: &[u8; 16], new_limit: u64) -> Instruction {
        Instruction {
            program_id: pay_on_chain::ID,
            accounts: pay_on_chain::accounts::RenewContract {
                payer: self.payer.pubkey(),
                site: self.site,
                contract: contract_pda(&self.site, &self.payer.pubkey()),
                old_slug_index: slug_pda(&self.site, current),
                new_slug_index: slug_pda(&self.site, next),
                payer_token_account: self.payer_ata,
                system_program: system_program::ID,
            }
            .to_account_metas(None),
            data: pay_on_chain::instruction::RenewContract {
                new_slug: *next,
                new_limit,
            }
            .data(),
        }
    }

    pub fn ix_close(&self, s: &[u8; 16]) -> Instruction {
        Instruction {
            program_id: pay_on_chain::ID,
            accounts: pay_on_chain::accounts::CloseContract {
                payer: self.payer.pubkey(),
                site: self.site,
                contract: contract_pda(&self.site, &self.payer.pubkey()),
                slug_index: slug_pda(&self.site, s),
            }
            .to_account_metas(None),
            data: pay_on_chain::instruction::CloseContract {}.data(),
        }
    }

    // --- convenience -------------------------------------------------------

    /// Approve and open in one transaction, the way a client must.
    pub fn open(&mut self, s: &[u8; 16], limit: u64) -> Result<(), String> {
        let ixs = [self.ix_approve(limit), self.ix_open(s, limit)];
        let payer = self.payer.insecure_clone();
        self.send(&ixs, &[&payer], &payer.pubkey())
    }

    pub fn meter(&mut self, views: u32) -> Result<(), String> {
        let ix = self.ix_meter(views);
        let authority = self.authority.insecure_clone();
        self.send(&[ix], &[&authority], &authority.pubkey())
    }
}

/// Anchor writes `Error Code: <Name>` into the logs; assert on that rather
/// than on a numeric code that shifts when the enum is reordered.
pub fn assert_error(result: Result<(), String>, code: &str) {
    match result {
        Ok(()) => panic!("expected {code}, transaction succeeded"),
        Err(e) => assert!(
            e.contains(code),
            "expected {code} in failure, got: {e}"
        ),
    }
}
