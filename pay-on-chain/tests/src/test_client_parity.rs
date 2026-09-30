//! Does the client emit the bytes the program accepts?
//!
//! Every other test builds instructions from Anchor's generated
//! `accounts::` / `instruction::` types. The client builds them by hand from
//! precomputed discriminators and hand-written account lists. Both can be
//! internally consistent and still disagree with each other, and nothing so
//! far would notice. These tests build each instruction both ways and require
//! them to be identical, so any future drift fails here.

use anchor_lang::{system_program, AccountSerialize, InstructionData, ToAccountMetas};
use anchor_spl::token::spl_token;
use solana_instruction::Instruction;
use solana_pubkey::Pubkey;

use sol_pay_client::core::{ids, ix as client, pda, state as client_state, Program};

use crate::harness::{fund_ata_of, ASSOCIATED_TOKEN_PROGRAM_ID};

const DECIMALS: u8 = 6;
const LIMIT: u64 = 500_000;
const EXPIRY: i64 = 1_800_003_600;
const INDEX: u8 = 3;

/// Fixed, distinguishable addresses. Nothing is executed here, so they need
/// only be valid pubkeys.
struct Fixture {
    authority: Pubkey,
    reader: Pubkey,
    key: Pubkey,
    mint: Pubkey,
    treasury: Pubkey,
    source: Pubkey,
    destination: Pubkey,
    site: Pubkey,
    fund: Pubkey,
}

impl Fixture {
    fn new() -> Self {
        let authority = Pubkey::new_unique();
        let reader = Pubkey::new_unique();
        let mint = Pubkey::new_unique();
        Fixture {
            site: pda::site_address(&authority).0,
            fund: pda::fund_address(&reader, &mint, INDEX).0,
            authority,
            reader,
            key: Pubkey::new_unique(),
            mint,
            treasury: Pubkey::new_unique(),
            source: Pubkey::new_unique(),
            destination: Pubkey::new_unique(),
        }
    }

    fn fund_ata(&self) -> Pubkey {
        pda::fund_token_account(&self.fund, &self.mint)
    }

    fn meter(&self) -> Pubkey {
        pda::meter_address(&self.site, &self.fund).0
    }
}

/// Compare in pieces so a failure says *what* diverged rather than dumping
/// two opaque structs.
fn assert_same(label: &str, client: &Instruction, anchor: &Instruction) {
    assert_eq!(client.program_id, anchor.program_id, "{label}: program id");
    assert_eq!(
        client.data, anchor.data,
        "{label}: instruction data (discriminator or argument encoding)"
    );
    assert_eq!(
        client.accounts.len(),
        anchor.accounts.len(),
        "{label}: account count"
    );
    for (i, (c, a)) in client.accounts.iter().zip(anchor.accounts.iter()).enumerate() {
        assert_eq!(c.pubkey, a.pubkey, "{label}: account {i} address");
        assert_eq!(c.is_signer, a.is_signer, "{label}: account {i} is_signer");
        assert_eq!(c.is_writable, a.is_writable, "{label}: account {i} is_writable");
    }
}

#[test]
fn client_and_program_agree_on_the_program_id() {
    assert_eq!(
        ids::PAY_ON_CHAIN_ID.to_bytes(),
        pay_on_chain::ID.to_bytes(),
        "the client's hardcoded program id has drifted from declare_id!"
    );
}

/// The deployment handle defaults to the program this workspace builds, on
/// the token program the tests run against, and the free functions are that
/// default. An integrator may override either; what they get when they do not
/// must still be this program and SPL Token.
#[test]
fn the_default_deployment_is_this_program() {
    assert_eq!(
        Program::default().id().to_bytes(),
        pay_on_chain::ID.to_bytes(),
        "Program::default() has drifted from declare_id!"
    );
    assert_eq!(
        Program::default().token_program().to_bytes(),
        spl_token::ID.to_bytes(),
        "Program::default() has drifted from SPL Token"
    );
    let authority = Pubkey::new_unique();
    assert_eq!(
        Program::default().site_address(&authority),
        pda::site_address(&authority),
        "the free functions are not the default deployment"
    );
}

/// The client hardcodes these base58 strings instead of depending on the
/// crates that define them. Cheap to typo, so check them against the real
/// sources -- the ATA program against the program's own constant, since no
/// crate here defines it.
#[test]
fn client_hardcoded_program_ids_are_correct() {
    assert_eq!(ids::TOKEN_PROGRAM_ID.to_bytes(), spl_token::ID.to_bytes(), "SPL Token");
    assert_eq!(
        ids::SYSTEM_PROGRAM_ID.to_bytes(),
        system_program::ID.to_bytes(),
        "System program"
    );
    assert_eq!(
        ids::TOKEN_2022_PROGRAM_ID.to_bytes(),
        anchor_spl::token_2022::ID.to_bytes(),
        "Token-2022"
    );
    assert_eq!(
        ids::ASSOCIATED_TOKEN_PROGRAM_ID.to_bytes(),
        pay_on_chain::constants::ASSOCIATED_TOKEN_PROGRAM_ID.to_bytes(),
        "Associated Token Account program, client vs program"
    );
    assert_eq!(
        ids::ASSOCIATED_TOKEN_PROGRAM_ID.to_bytes(),
        ASSOCIATED_TOKEN_PROGRAM_ID.to_bytes(),
        "Associated Token Account program, client vs the harness LiteSVM runs"
    );
}

#[test]
fn client_derives_the_same_addresses() {
    let f = Fixture::new();

    let anchor_site =
        Pubkey::find_program_address(&[b"site", f.authority.as_ref()], &pay_on_chain::ID).0;
    assert_eq!(f.site, anchor_site, "site seeds");

    let anchor_fund = Pubkey::find_program_address(
        &[b"fund", f.reader.as_ref(), f.mint.as_ref(), &[INDEX]],
        &pay_on_chain::ID,
    )
    .0;
    assert_eq!(f.fund, anchor_fund, "fund seeds");

    let anchor_meter = Pubkey::find_program_address(
        &[b"meter", f.site.as_ref(), f.fund.as_ref()],
        &pay_on_chain::ID,
    )
    .0;
    assert_eq!(f.meter(), anchor_meter, "meter seeds");

    // The fund's token account three ways: the client, the program's own
    // helper that pins it in `open_fund` and `close_fund`, and the formula
    // the harness uses to find what LiteSVM's ATA program created.
    assert_eq!(
        f.fund_ata(),
        pay_on_chain::fund_token_address(&f.fund, &f.mint, &spl_token::ID),
        "fund token account, client vs program"
    );
    assert_eq!(f.fund_ata(), fund_ata_of(&f.fund, &f.mint), "fund token account formula");
}

#[test]
fn initialize_site_matches() {
    let f = Fixture::new();
    let anchor = Instruction {
        program_id: pay_on_chain::ID,
        accounts: pay_on_chain::accounts::InitializeSite {
            authority: f.authority,
            site: f.site,
            mint: f.mint,
            treasury: f.treasury,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
        data: pay_on_chain::instruction::InitializeSite {
            item_price: 1_000,
            collection_threshold: 50_000,
            min_limit: 200_000,
        }
        .data(),
    };
    let c = client::initialize_site(&f.authority, &f.mint, &f.treasury, 1_000, 50_000, 200_000);
    assert_same("initialize_site", &c, &anchor);
}

#[test]
fn open_fund_matches() {
    let f = Fixture::new();
    let anchor = Instruction {
        program_id: pay_on_chain::ID,
        accounts: pay_on_chain::accounts::OpenFund {
            reader: f.reader,
            fund: f.fund,
            fund_token_account: f.fund_ata(),
            mint: f.mint,
            token_program: spl_token::ID,
            associated_token_program: ASSOCIATED_TOKEN_PROGRAM_ID,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
        data: pay_on_chain::instruction::OpenFund { index: INDEX }.data(),
    };
    let c = client::open_fund(&f.reader, &f.mint, INDEX);
    assert_same("open_fund", &c, &anchor);
}

#[test]
fn withdraw_matches() {
    let f = Fixture::new();
    let anchor = Instruction {
        program_id: pay_on_chain::ID,
        accounts: pay_on_chain::accounts::Withdraw {
            reader: f.reader,
            fund: f.fund,
            fund_token_account: f.fund_ata(),
            destination: f.destination,
            mint: f.mint,
            token_program: spl_token::ID,
        }
        .to_account_metas(None),
        data: pay_on_chain::instruction::Withdraw { amount: 12_345 }.data(),
    };
    let c = client::withdraw(&f.reader, &f.mint, INDEX, &f.destination, 12_345);
    assert_same("withdraw", &c, &anchor);
}

#[test]
fn close_fund_matches() {
    let f = Fixture::new();
    let anchor = Instruction {
        program_id: pay_on_chain::ID,
        accounts: pay_on_chain::accounts::CloseFund {
            reader: f.reader,
            fund: f.fund,
            fund_token_account: f.fund_ata(),
            token_program: spl_token::ID,
        }
        .to_account_metas(None),
        data: pay_on_chain::instruction::CloseFund {}.data(),
    };
    let c = client::close_fund(&f.reader, &f.mint, INDEX);
    assert_same("close_fund", &c, &anchor);
}

#[test]
fn open_meter_matches() {
    let f = Fixture::new();
    let anchor = Instruction {
        program_id: pay_on_chain::ID,
        accounts: pay_on_chain::accounts::OpenMeter {
            reader: f.reader,
            site: f.site,
            fund: f.fund,
            meter: f.meter(),
            system_program: system_program::ID,
        }
        .to_account_metas(None),
        data: pay_on_chain::instruction::OpenMeter {
            key: f.key,
            limit: LIMIT,
            expiry: EXPIRY,
        }
        .data(),
    };
    let c = client::open_meter(&f.site, &f.reader, &f.fund, &f.key, LIMIT, EXPIRY);
    assert_same("open_meter", &c, &anchor);
}

#[test]
fn meter_and_settle_matches() {
    let f = Fixture::new();
    let anchor = Instruction {
        program_id: pay_on_chain::ID,
        accounts: pay_on_chain::accounts::MeterAndSettle {
            site: f.site,
            authority: f.authority,
            fund: f.fund,
            meter: f.meter(),
            fund_token_account: f.fund_ata(),
            treasury: f.treasury,
            mint: f.mint,
            token_program: spl_token::ID,
        }
        .to_account_metas(None),
        data: pay_on_chain::instruction::MeterAndSettle { items: 7 }.data(),
    };
    let c = client::meter_and_settle(&f.site, &f.authority, &f.fund, &f.treasury, &f.mint, 7);
    assert_same("meter_and_settle", &c, &anchor);
}

#[test]
fn renew_meter_matches() {
    let f = Fixture::new();
    let anchor = Instruction {
        program_id: pay_on_chain::ID,
        accounts: pay_on_chain::accounts::RenewMeter {
            reader: f.reader,
            site: f.site,
            fund: f.fund,
            meter: f.meter(),
        }
        .to_account_metas(None),
        data: pay_on_chain::instruction::RenewMeter {
            key: f.key,
            new_limit: LIMIT,
            expiry: EXPIRY,
        }
        .data(),
    };
    let c = client::renew_meter(&f.site, &f.reader, &f.fund, &f.key, LIMIT, EXPIRY);
    assert_same("renew_meter", &c, &anchor);
}

#[test]
fn close_meter_matches() {
    let f = Fixture::new();
    for signer in [f.reader, f.key] {
        let anchor = Instruction {
            program_id: pay_on_chain::ID,
            accounts: pay_on_chain::accounts::CloseMeter {
                signer,
                site: f.site,
                fund: f.fund,
                reader: f.reader,
                meter: f.meter(),
            }
            .to_account_metas(None),
            data: pay_on_chain::instruction::CloseMeter {}.data(),
        };
        let c = client::close_meter(&signer, &f.reader, &f.site, &f.fund);
        assert_same("close_meter", &c, &anchor);
    }
}

/// The client hand-encodes the SPL `transfer_checked` a deposit is, rather
/// than depending on spl-token, to keep the WASM bundle small. That trade is
/// only safe if the bytes match what spl-token itself produces.
#[test]
fn hand_rolled_deposit_matches_spl_token() {
    let f = Fixture::new();
    let theirs = spl_token::instruction::transfer_checked(
        &spl_token::ID,
        &f.source,
        &f.mint,
        &f.fund_ata(),
        &f.reader,
        &[],
        LIMIT,
        DECIMALS,
    )
    .unwrap();
    let ours = client::deposit(&f.source, &f.reader, &f.fund, &f.mint, LIMIT, DECIMALS);
    assert_same("deposit", &ours, &theirs);
}

// --- account decoding -----------------------------------------------------
//
// The client decodes account data by hand, from byte offsets. That is only
// safe while the layout it assumes is the layout the program writes, so both
// halves of that claim are asserted here rather than described in a comment.

/// A field added on chain shifts every field after it. Sizes are the cheapest
/// tripwire for that, and `INIT_SPACE` is generated from the struct itself.
#[test]
fn client_account_sizes_match_the_program() {
    use anchor_lang::Space;
    assert_eq!(client_state::SITE_LEN, 8 + pay_on_chain::state::Site::INIT_SPACE, "Site");
    assert_eq!(client_state::FUND_LEN, 8 + pay_on_chain::state::Fund::INIT_SPACE, "Fund");
    assert_eq!(client_state::METER_LEN, 8 + pay_on_chain::state::Meter::INIT_SPACE, "Meter");
}

/// The real test: let Anchor write an account exactly as the program would,
/// then read it back with the client. This pins the discriminator, the field
/// order and every offset at once, and it fails if any of them move.
#[test]
fn client_decodes_what_anchor_serializes() {
    let f = Fixture::new();

    let site = pay_on_chain::state::Site {
        authority: f.authority,
        mint: f.mint,
        treasury: f.treasury,
        item_price: 10_000,
        collection_threshold: 250_000,
        min_limit: 500_000,
        bump: 253,
    };
    let mut bytes = Vec::new();
    site.try_serialize(&mut bytes).unwrap();
    assert_eq!(bytes.len(), client_state::SITE_LEN, "serialized Site length");
    let decoded = client_state::Site::decode(&bytes).expect("client decodes Site");
    assert_eq!(decoded.authority, site.authority);
    assert_eq!(decoded.mint, site.mint);
    assert_eq!(decoded.treasury, site.treasury);
    assert_eq!(decoded.item_price, site.item_price);
    assert_eq!(decoded.collection_threshold, site.collection_threshold);
    assert_eq!(decoded.min_limit, site.min_limit);
    assert_eq!(decoded.bump, site.bump);

    let fund = pay_on_chain::state::Fund {
        reader: f.reader,
        mint: f.mint,
        index: INDEX,
        meters: 70_000,
        bump: 252,
    };
    let mut bytes = Vec::new();
    fund.try_serialize(&mut bytes).unwrap();
    assert_eq!(bytes.len(), client_state::FUND_LEN, "serialized Fund length");
    let decoded = client_state::Fund::decode(&bytes).expect("client decodes Fund");
    assert_eq!(decoded.reader, fund.reader);
    assert_eq!(decoded.mint, fund.mint);
    assert_eq!(decoded.index, fund.index);
    assert_eq!(decoded.meters, fund.meters);
    assert_eq!(decoded.bump, fund.bump);

    let meter = pay_on_chain::state::Meter {
        site: f.site,
        fund: f.fund,
        key: f.key,
        expiry: -EXPIRY, // a sign bit, so a u64 read would show
        limit: LIMIT,
        used: 120_000,
        paid: 100_000,
        bump: 251,
    };
    let mut bytes = Vec::new();
    meter.try_serialize(&mut bytes).unwrap();
    assert_eq!(bytes.len(), client_state::METER_LEN, "serialized Meter length");
    let decoded = client_state::Meter::decode(&bytes).expect("client decodes Meter");
    assert_eq!(decoded.site, meter.site);
    assert_eq!(decoded.fund, meter.fund);
    assert_eq!(decoded.key, meter.key);
    assert_eq!(decoded.expiry, meter.expiry);
    assert_eq!(decoded.limit, meter.limit);
    assert_eq!(decoded.used, meter.used);
    assert_eq!(decoded.paid, meter.paid);
    assert_eq!(decoded.bump, meter.bump);

    // The derived helpers must agree with the program's own.
    assert_eq!(decoded.unpaid(), meter.unpaid());
    assert_eq!(decoded.outstanding(), meter.outstanding());
    for now in [-EXPIRY - 1, -EXPIRY, -EXPIRY + 1] {
        assert_eq!(decoded.expired(now), meter.expired(now), "expired({now})");
    }
}

/// An account of the wrong type must be refused, not reinterpreted.
#[test]
fn client_refuses_an_account_of_another_type() {
    let f = Fixture::new();
    let site = pay_on_chain::state::Site {
        authority: f.authority,
        mint: f.mint,
        treasury: f.treasury,
        item_price: 1,
        collection_threshold: 2,
        min_limit: 3,
        bump: 250,
    };
    let mut bytes = Vec::new();
    site.try_serialize(&mut bytes).unwrap();
    assert!(client_state::Meter::decode(&bytes).is_err(), "a Site is not a Meter");
    assert!(client_state::Fund::decode(&bytes).is_err(), "a Site is not a Fund");
}

/// SPEC §8: `verify_key` against a signature made by the key material the
/// rest of Solana uses -- `solana-keypair`, which signs with ed25519-dalek
/// under the hood -- so the crate's dependency and the ecosystem's agree.
/// The RFC 8032 vectors are pinned in the client's own tests.
#[test]
fn verify_key_accepts_what_a_solana_keypair_signs() {
    use sol_pay_client::core::proof::verify_key;
    use solana_keypair::Keypair;
    use solana_signer::Signer;

    let key = Keypair::new();
    let message = b"site-issued nonce 7f3a, issued 1800000000";
    let signature = key.sign_message(message);
    let signature: [u8; 64] = signature.as_ref().try_into().unwrap();
    let public = key.pubkey().to_bytes();

    assert!(verify_key(&public, message, &signature));
    assert!(!verify_key(&public, b"another message", &signature));
    assert!(!verify_key(&Keypair::new().pubkey().to_bytes(), message, &signature));
}
