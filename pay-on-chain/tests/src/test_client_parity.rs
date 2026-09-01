//! Does the WASM client emit the bytes the program accepts?
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

const DECIMALS: u8 = 6;
const LIMIT: u64 = 500_000;

/// Fixed, distinguishable addresses. Nothing is executed here, so they need
/// only be valid pubkeys.
struct Fixture {
    authority: Pubkey,
    payer: Pubkey,
    mint: Pubkey,
    treasury: Pubkey,
    payer_ata: Pubkey,
    site: Pubkey,
}

impl Fixture {
    fn new() -> Self {
        let authority = Pubkey::new_unique();
        let site = pda::site_address(&authority).0;
        Fixture {
            authority,
            payer: Pubkey::new_unique(),
            mint: Pubkey::new_unique(),
            treasury: Pubkey::new_unique(),
            payer_ata: Pubkey::new_unique(),
            site,
        }
    }

    fn contract(&self) -> Pubkey {
        pda::contract_address(&self.site, &self.payer).0
    }
}

/// Compare in pieces so a failure says *what* diverged rather than dumping
/// two opaque structs.
fn assert_same(label: &str, client: &Instruction, anchor: &Instruction) {
    assert_eq!(
        client.program_id, anchor.program_id,
        "{label}: program id"
    );
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
        assert_eq!(
            c.is_writable, a.is_writable,
            "{label}: account {i} is_writable"
        );
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

/// The deployment handle defaults to the program this workspace builds, and
/// the free functions are that default. An integrator may override the id;
/// what they get when they do not must still be this program.
#[test]
fn the_default_deployment_is_this_program() {
    assert_eq!(
        Program::default().id().to_bytes(),
        pay_on_chain::ID.to_bytes(),
        "Program::default() has drifted from declare_id!"
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
/// sources.
#[test]
fn client_hardcoded_program_ids_are_correct() {
    assert_eq!(
        ids::TOKEN_PROGRAM_ID.to_bytes(),
        spl_token::ID.to_bytes(),
        "SPL Token program id"
    );
    assert_eq!(
        ids::SYSTEM_PROGRAM_ID.to_bytes(),
        system_program::ID.to_bytes(),
        "System program id"
    );
    assert_eq!(
        ids::TOKEN_2022_PROGRAM_ID.to_bytes(),
        anchor_spl::token_2022::ID.to_bytes(),
        "Token-2022 program id"
    );
}

#[test]
fn client_derives_the_same_addresses() {
    let f = Fixture::new();

    let anchor_site =
        Pubkey::find_program_address(&[b"site", f.authority.as_ref()], &pay_on_chain::ID).0;
    assert_eq!(f.site, anchor_site, "site seeds");

    let anchor_contract = Pubkey::find_program_address(
        &[b"contract", f.site.as_ref(), f.payer.as_ref()],
        &pay_on_chain::ID,
    )
    .0;
    assert_eq!(f.contract(), anchor_contract, "contract seeds");
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
            page_price: 1_000,
            collection_threshold: 50_000,
            min_limit: 200_000,
        }
        .data(),
    };
    let c = client::initialize_site(
        &f.authority,
        &f.mint,
        &f.treasury,
        1_000,
        50_000,
        200_000,
    );
    assert_same("initialize_site", &c, &anchor);
}

#[test]
fn open_contract_matches() {
    let f = Fixture::new();
    let anchor = Instruction {
        program_id: pay_on_chain::ID,
        accounts: pay_on_chain::accounts::OpenContract {
            payer: f.payer,
            site: f.site,
            contract: f.contract(),
            payer_token_account: f.payer_ata,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
        data: pay_on_chain::instruction::OpenContract { limit: LIMIT }.data(),
    };
    let c = client::open_contract(&f.site, &f.payer, &f.payer_ata, LIMIT);
    assert_same("open_contract", &c, &anchor);
}

#[test]
fn meter_and_settle_matches() {
    let f = Fixture::new();
    let anchor = Instruction {
        program_id: pay_on_chain::ID,
        accounts: pay_on_chain::accounts::MeterAndSettle {
            site: f.site,
            authority: f.authority,
            payer: f.payer,
            contract: f.contract(),
            payer_token_account: f.payer_ata,
            treasury: f.treasury,
            mint: f.mint,
            token_program: spl_token::ID,
        }
        .to_account_metas(None),
        data: pay_on_chain::instruction::MeterAndSettle { page_views: 7 }.data(),
    };
    let c = client::meter_and_settle(
        &f.site,
        &f.authority,
        &f.payer,
        &f.payer_ata,
        &f.treasury,
        &f.mint,
        &spl_token::ID,
        7,
    );
    assert_same("meter_and_settle", &c, &anchor);
}

#[test]
fn renew_contract_matches() {
    let f = Fixture::new();
    let anchor = Instruction {
        program_id: pay_on_chain::ID,
        accounts: pay_on_chain::accounts::RenewContract {
            payer: f.payer,
            site: f.site,
            contract: f.contract(),
            payer_token_account: f.payer_ata,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
        data: pay_on_chain::instruction::RenewContract { new_limit: LIMIT }.data(),
    };
    let c = client::renew_contract(&f.site, &f.payer, &f.payer_ata, LIMIT);
    assert_same("renew_contract", &c, &anchor);
}

#[test]
fn close_contract_matches() {
    let f = Fixture::new();
    let anchor = Instruction {
        program_id: pay_on_chain::ID,
        accounts: pay_on_chain::accounts::CloseContract {
            payer: f.payer,
            site: f.site,
            contract: f.contract(),
        }
        .to_account_metas(None),
        data: pay_on_chain::instruction::CloseContract {}.data(),
    };
    let c = client::close_contract(&f.site, &f.payer);
    assert_same("close_contract", &c, &anchor);
}

/// The client hand-encodes the SPL Token instructions rather than depending on
/// spl-token, to keep the WASM bundle small. That trade is only safe if the
/// bytes match what spl-token itself produces.
#[test]
fn hand_rolled_spl_instructions_match_spl_token() {
    let f = Fixture::new();

    let theirs = spl_token::instruction::approve_checked(
        &spl_token::ID,
        &f.payer_ata,
        &f.mint,
        &f.contract(),
        &f.payer,
        &[],
        LIMIT,
        DECIMALS,
    )
    .unwrap();
    let ours = client::approve_checked(
        &spl_token::ID,
        &f.payer_ata,
        &f.mint,
        &f.payer,
        &f.site,
        LIMIT,
        DECIMALS,
    );
    assert_same("approve_checked", &ours, &theirs);

    let theirs = spl_token::instruction::revoke(&spl_token::ID, &f.payer_ata, &f.payer, &[]).unwrap();
    let ours = client::revoke(&spl_token::ID, &f.payer_ata, &f.payer);
    assert_same("revoke", &ours, &theirs);
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
    assert_eq!(
        client_state::SITE_LEN,
        8 + pay_on_chain::state::Site::INIT_SPACE,
        "Site length"
    );
    assert_eq!(
        client_state::CONTRACT_LEN,
        8 + pay_on_chain::state::Contract::INIT_SPACE,
        "Contract length"
    );
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
        page_price: 10_000,
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
    assert_eq!(decoded.page_price, site.page_price);
    assert_eq!(decoded.collection_threshold, site.collection_threshold);
    assert_eq!(decoded.min_limit, site.min_limit);
    assert_eq!(decoded.bump, site.bump);

    let contract = pay_on_chain::state::Contract {
        site: f.site,
        payer: f.payer,
        limit: LIMIT,
        used: 120_000,
        paid: 100_000,
        bump: 251,
    };
    let mut bytes = Vec::new();
    contract.try_serialize(&mut bytes).unwrap();
    assert_eq!(
        bytes.len(),
        client_state::CONTRACT_LEN,
        "serialized Contract length"
    );

    let decoded = client_state::Contract::decode(&bytes).expect("client decodes Contract");
    assert_eq!(decoded.site, contract.site);
    assert_eq!(decoded.payer, contract.payer);
    assert_eq!(decoded.limit, contract.limit);
    assert_eq!(decoded.used, contract.used);
    assert_eq!(decoded.paid, contract.paid);
    assert_eq!(decoded.bump, contract.bump);

    // The derived helpers must agree with the program's own.
    assert_eq!(decoded.unpaid(), contract.unpaid());
    assert_eq!(decoded.outstanding(), contract.outstanding());
}

/// An account of the right size but the wrong type must be refused, not
/// reinterpreted. Site and Contract differ in length, so the case worth
/// checking is a Site's bytes with a Contract's discriminator swapped in.
#[test]
fn client_refuses_an_account_of_another_type() {
    let f = Fixture::new();
    let site = pay_on_chain::state::Site {
        authority: f.authority,
        mint: f.mint,
        treasury: f.treasury,
        page_price: 1,
        collection_threshold: 2,
        min_limit: 3,
        bump: 250,
    };
    let mut bytes = Vec::new();
    site.try_serialize(&mut bytes).unwrap();

    assert!(
        client_state::Contract::decode(&bytes).is_err(),
        "a Site must not decode as a Contract"
    );
}
