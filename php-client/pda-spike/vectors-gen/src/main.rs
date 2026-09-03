//! Generates conformance vectors for php-client to check itself against:
//! PDA derivation and one instruction from the published `sol-pay-client`
//! crate, plus one genuine Anchor-serialized `Site`/`Contract` account and
//! the program's own error code tables, both sourced directly from the
//! `pay-on-chain` program crate rather than copied by hand.
//!
//!   cargo run --release > ../php/vectors.json
//!   cd ../php && php verify.php vectors.json
//!
//! Inputs are derived as sha256("authority-<i>") and sha256("payer-<i>") so
//! the PHP side can reproduce them without transferring them.

use anchor_lang::{AnchorSerialize, Discriminator};
use sha2::{Digest, Sha256};
use sol_pay_client::core::{ids, ix, pda};
use solana_pubkey::Pubkey;
use spl_token::error::TokenError;

fn seeded(tag: &str, i: u32) -> Pubkey {
    let mut h = Sha256::new();
    h.update(format!("{tag}-{i}").as_bytes());
    Pubkey::new_from_array(h.finalize().into())
}

fn main() {
    let n: u32 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(400);

    println!("{{");
    println!("  \"program_id\": \"{}\",", ids::PAY_ON_CHAIN_ID);
    println!("  \"count\": {n},");

    println!("  \"site\": [");
    for i in 0..n {
        let authority = seeded("authority", i);
        let (addr, bump) = pda::site_address(&authority);
        let sep = if i + 1 == n { "" } else { "," };
        println!("    {{\"i\":{i},\"address\":\"{addr}\",\"bump\":{bump}}}{sep}");
    }
    println!("  ],");

    println!("  \"contract\": [");
    for i in 0..n {
        let (site, _) = pda::site_address(&seeded("authority", i));
        let payer = seeded("payer", i);
        let (addr, bump) = pda::contract_address(&site, &payer);
        let sep = if i + 1 == n { "" } else { "," };
        println!("    {{\"i\":{i},\"address\":\"{addr}\",\"bump\":{bump}}}{sep}");
    }
    println!("  ],");

    // One fully-built instruction, so the PHP side can check the discriminator,
    // the argument encoding, and the account list with its signer/writable flags.
    let authority = seeded("authority", 0);
    let (site, _) = pda::site_address(&authority);
    let payer = seeded("payer", 0);
    let payer_ata = seeded("payer-ata", 0);
    let treasury = seeded("treasury", 0);
    let mint = seeded("mint", 0);
    let inst = ix::meter_and_settle(&site, &authority, &payer, &payer_ata, &treasury, &mint, 7);
    println!("  \"meter_and_settle\": {{");
    println!("    \"page_views\": 7,");
    println!("    \"program_id\": \"{}\",", inst.program_id);
    println!("    \"data_hex\": \"{}\",", hex(&inst.data));
    println!("    \"accounts\": [");
    let last = inst.accounts.len() - 1;
    for (k, a) in inst.accounts.iter().enumerate() {
        let sep = if k == last { "" } else { "," };
        println!(
            "      {{\"pubkey\":\"{}\",\"is_signer\":{},\"is_writable\":{}}}{}",
            a.pubkey, a.is_signer, a.is_writable, sep
        );
    }
    println!("    ]");
    println!("  }},");

    // A genuine Anchor-serialized Site account: pay_on_chain::state::Site's
    // own #[account]-derived DISCRIMINATOR plus AnchorSerialize, not a
    // hand-assembled byte string. Field values reuse this file's seeds so
    // php-client/tests/Core/StateTest.php can rebuild the same addresses
    // instead of pasting new magic constants.
    let onchain_site = pay_on_chain::state::Site {
        authority,
        mint,
        treasury,
        page_price: 10_000,
        collection_threshold: 250_000,
        min_limit: 500_000,
        bump: 254,
    };
    let mut site_bytes = pay_on_chain::state::Site::DISCRIMINATOR.to_vec();
    site_bytes.extend(onchain_site.try_to_vec().expect("Site serializes"));
    println!("  \"site_account\": {{");
    println!("    \"data_hex\": \"{}\",", hex(&site_bytes));
    println!("    \"authority\": \"{authority}\",");
    println!("    \"mint\": \"{mint}\",");
    println!("    \"treasury\": \"{treasury}\",");
    println!("    \"page_price\": {},", onchain_site.page_price);
    println!("    \"collection_threshold\": {},", onchain_site.collection_threshold);
    println!("    \"min_limit\": {},", onchain_site.min_limit);
    println!("    \"bump\": {}", onchain_site.bump);
    println!("  }},");

    // Same discipline for Contract. `site` is the real PDA derived above, so
    // this is internally consistent with the meter_and_settle vector too.
    let onchain_contract = pay_on_chain::state::Contract {
        site,
        payer,
        limit: 1_000_000,
        used: 250_000,
        paid: 100_000,
        bump: 253,
    };
    let mut contract_bytes = pay_on_chain::state::Contract::DISCRIMINATOR.to_vec();
    contract_bytes.extend(onchain_contract.try_to_vec().expect("Contract serializes"));
    println!("  \"contract_account\": {{");
    println!("    \"data_hex\": \"{}\",", hex(&contract_bytes));
    println!("    \"site\": \"{site}\",");
    println!("    \"payer\": \"{payer}\",");
    println!("    \"limit\": {},", onchain_contract.limit);
    println!("    \"used\": {},", onchain_contract.used);
    println!("    \"paid\": {},", onchain_contract.paid);
    println!("    \"bump\": {}", onchain_contract.bump);
    println!("  }},");

    // Every PayError variant's real Anchor code -- ERROR_CODE_OFFSET (6000)
    // plus C-like declaration order -- read from pay_on_chain's own enum,
    // the same arithmetic Anchor itself uses, rather than nine hardcoded
    // numbers someone has to keep in step by hand.
    use pay_on_chain::errors::PayError;
    let pay_errors: &[(&str, u32)] = &[
        ("LimitBelowMinimum", PayError::LimitBelowMinimum as u32 + anchor_lang::error::ERROR_CODE_OFFSET),
        ("MinimumBelowThreshold", PayError::MinimumBelowThreshold as u32 + anchor_lang::error::ERROR_CODE_OFFSET),
        ("ZeroPagePrice", PayError::ZeroPagePrice as u32 + anchor_lang::error::ERROR_CODE_OFFSET),
        ("LimitReached", PayError::LimitReached as u32 + anchor_lang::error::ERROR_CODE_OFFSET),
        ("DelegateNotSet", PayError::DelegateNotSet as u32 + anchor_lang::error::ERROR_CODE_OFFSET),
        ("DelegateMismatch", PayError::DelegateMismatch as u32 + anchor_lang::error::ERROR_CODE_OFFSET),
        ("DelegateAllowanceTooLow", PayError::DelegateAllowanceTooLow as u32 + anchor_lang::error::ERROR_CODE_OFFSET),
        ("LimitBelowUsage", PayError::LimitBelowUsage as u32 + anchor_lang::error::ERROR_CODE_OFFSET),
        ("MathOverflow", PayError::MathOverflow as u32 + anchor_lang::error::ERROR_CODE_OFFSET),
    ];
    println!("  \"pay_errors\": [");
    for (k, (name, code)) in pay_errors.iter().enumerate() {
        let sep = if k + 1 == pay_errors.len() { "" } else { "," };
        println!("    {{\"name\":\"{name}\",\"code\":{code}}}{sep}");
    }
    println!("  ],");

    // Same for the SPL Token errors this flow can provoke, read from
    // spl-token's own enum rather than copied numbers.
    let token_errors: &[(&str, u32)] = &[
        ("InsufficientFunds", TokenError::InsufficientFunds as u32),
        ("MintMismatch", TokenError::MintMismatch as u32),
        ("OwnerMismatch", TokenError::OwnerMismatch as u32),
        ("AccountFrozen", TokenError::AccountFrozen as u32),
        ("MintDecimalsMismatch", TokenError::MintDecimalsMismatch as u32),
    ];
    println!("  \"token_errors\": [");
    for (k, (name, code)) in token_errors.iter().enumerate() {
        let sep = if k + 1 == token_errors.len() { "" } else { "," };
        println!("    {{\"name\":\"{name}\",\"code\":{code}}}{sep}");
    }
    println!("  ]");

    println!("}}");
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}
