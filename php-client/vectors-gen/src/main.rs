//! Generates conformance vectors for php-client to check itself against:
//! PDA derivation and one instruction from the published `sol-pay-client`
//! crate, plus one genuine Anchor-serialized `Site`/`Contract` account and
//! the program's own error code tables, both sourced directly from the
//! `pay-on-chain` program crate rather than copied by hand.
//!
//!   cargo run --release > vectors.json
//!   php ../conformance/vectors.php
//!
//! Inputs are derived as sha256("<tag>-<i>") so the PHP side can reproduce
//! them without transferring them.
//!
//! It also emits one compiled legacy transaction message and its wire bytes,
//! produced by `solana-message` and `solana-transaction` rather than
//! transcribed, for `SolPay\Tx` to be written against. Those crates are taken
//! from the solana 2.x generation on purpose -- see Cargo.toml.

use anchor_lang::{AnchorSerialize, Discriminator};
use sha2::{Digest, Sha256};
use sol_pay_client::core::{ids, ix, pda};
use solana_hash::Hash;
use solana_instruction::Instruction;
use solana_message::Message;
use solana_pubkey::Pubkey;
use solana_signature::Signature;
use solana_transaction::Transaction;
use spl_token::error::TokenError;

fn seeded_bytes(tag: &str, i: u32) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(format!("{tag}-{i}").as_bytes());
    h.finalize().into()
}

fn seeded(tag: &str, i: u32) -> Pubkey {
    Pubkey::new_from_array(seeded_bytes(tag, i))
}

/// 64 fixed bytes standing in for a signature, seeded per case and position so
/// no two vectors share one. It signs nothing and cannot be verified: nothing
/// in sol-pay signs, in any language, and SPEC §7 says nothing will. It is
/// here so the wire framing around the message -- the compact-u16 signature
/// count and the signature array -- has ground truth for more than one signer.
fn fake_signature(tag: &str, i: usize) -> [u8; 64] {
    let mut out = [0u8; 64];
    out[..32].copy_from_slice(&seeded_bytes(&format!("signature-{tag}-hi"), i as u32));
    out[32..].copy_from_slice(&seeded_bytes(&format!("signature-{tag}-lo"), i as u32));
    out
}

/// One compiled-transaction vector. Records what compilation *consumed*
/// (`source_instructions`) beside what it produced, so the conformance check
/// holds one against the other without knowing which case it is looking at.
fn emit_transaction(
    name: &str,
    instructions: &[Instruction],
    fee_payer: &Pubkey,
    blockhash: &Hash,
    sep: &str,
) {
    let message = Message::new_with_blockhash(instructions, Some(fee_payer), blockhash);
    let sigs: Vec<[u8; 64]> = (0..message.header.num_required_signatures as usize)
        .map(|i| fake_signature(name, i))
        .collect();
    let tx = Transaction {
        signatures: sigs.iter().copied().map(Signature::from).collect(),
        message: message.clone(),
    };

    println!("    {{");
    println!("      \"name\": \"{name}\",");
    println!("      \"fee_payer\": \"{fee_payer}\",");
    println!("      \"recent_blockhash\": \"{blockhash}\",");

    println!("      \"signatures_hex\": [");
    for (k, sig) in sigs.iter().enumerate() {
        let c = if k + 1 == sigs.len() { "" } else { "," };
        println!("        \"{}\"{}", hex(sig), c);
    }
    println!("      ],");

    println!("      \"source_instructions\": [");
    for (k, inst) in instructions.iter().enumerate() {
        let c = if k + 1 == instructions.len() { "" } else { "," };
        println!("        {{");
        println!("          \"program_id\": \"{}\",", inst.program_id);
        println!("          \"data_hex\": \"{}\",", hex(&inst.data));
        println!("          \"accounts\": [");
        for (m, a) in inst.accounts.iter().enumerate() {
            let ac = if m + 1 == inst.accounts.len() { "" } else { "," };
            println!(
                "            {{\"pubkey\":\"{}\",\"is_signer\":{},\"is_writable\":{}}}{}",
                a.pubkey, a.is_signer, a.is_writable, ac
            );
        }
        println!("          ]");
        println!("        }}{c}");
    }
    println!("      ],");

    // Decomposed as well as whole. A byte-for-byte mismatch on `message_hex`
    // alone says only "differs"; the pieces below say whether the ordering,
    // the header counts or the compiled indexes are what went wrong, which is
    // the same reason the meter_and_settle vector records flags and not just
    // its data.
    println!("      \"header\": {{");
    println!("        \"num_required_signatures\": {},", message.header.num_required_signatures);
    println!("        \"num_readonly_signed_accounts\": {},", message.header.num_readonly_signed_accounts);
    println!("        \"num_readonly_unsigned_accounts\": {}", message.header.num_readonly_unsigned_accounts);
    println!("      }},");

    println!("      \"account_keys\": [");
    for (k, key) in message.account_keys.iter().enumerate() {
        let c = if k + 1 == message.account_keys.len() { "" } else { "," };
        println!("        \"{key}\"{c}");
    }
    println!("      ],");

    println!("      \"instructions\": [");
    for (k, ci) in message.instructions.iter().enumerate() {
        let c = if k + 1 == message.instructions.len() { "" } else { "," };
        let idx: Vec<String> = ci.accounts.iter().map(|a| a.to_string()).collect();
        println!(
            "        {{\"program_id_index\":{},\"account_indexes\":[{}],\"data_hex\":\"{}\"}}{}",
            ci.program_id_index,
            idx.join(","),
            hex(&ci.data),
            c
        );
    }
    println!("      ],");

    println!("      \"message_hex\": \"{}\",", hex(&message.serialize()));
    println!(
        "      \"wire_hex\": \"{}\"",
        hex(&bincode::serialize(&tx).expect("transaction serializes"))
    );
    println!("    }}{sep}");
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

    // Compiled legacy transaction messages, and the wire bytes around them.
    // These are the vectors `SolPay\Tx` gets written against, and they exist
    // *before* the encoder does on purpose: writing the compiler first means
    // hand-verifying wire bytes, which is the trap the libsodium PDA shortcut
    // was -- plausible output, no error.
    //
    // `Message::new_with_blockhash` does the whole job PHP will have to
    // reproduce: merge each key's signer/writable flags across every
    // instruction, add the invoked program ids, force the fee payer to a
    // writable signer and put it first, sort the rest into the four
    // signer/writable partitions the header counts describe, index the
    // accounts, and length-prefix everything with compact-u16.
    //
    // Three cases, because one instruction signed and paid for by one key
    // leaves most of that unexercised. Between them they cover every branch:
    // Fixed, and seeded like every other input here so the PHP side can
    // reproduce it. The blockhash is passed in and never fetched -- SPEC §7.
    let blockhash = Hash::new_from_array(seeded_bytes("blockhash", 0));
    let separate_payer = seeded("fee-payer", 0);
    let init = ix::initialize_site(&authority, &mint, &treasury, 10_000, 250_000, 500_000);

    println!("  \"transactions\": [");

    // 1. The shipping shape. The site authority signs `meter_and_settle` and
    //    pays the fee, so there is exactly one signature. Note the promotion
    //    this pins: the instruction marks the authority a *readonly* signer
    //    and the compiled header makes it writable, because it pays.
    emit_transaction("authority-pays", std::slice::from_ref(&inst), &authority, &blockhash, ",");

    // 2. A fee payer that is not the instruction's signer -- a relayer paying
    //    for someone else's instruction. Two signatures, and the authority
    //    stays readonly, which is the only way to put anything in the
    //    readonly-signer partition. Case 1 leaves that partition empty, so
    //    without this an encoder could omit it and still pass.
    emit_transaction("separate-fee-payer", std::slice::from_ref(&inst), &separate_payer, &blockhash, ",");

    // 3. Two instructions, and a fee payer that is neither of their signers.
    //    This case earns its place three times over:
    //
    //    - `site` is writable in `initialize_site` and readonly in
    //      `meter_and_settle`; `treasury` is readonly in the first and
    //      writable in the second. Both must come out writable, which pins
    //      the flag merge in both directions.
    //    - Both instructions call the same program, so its id must appear in
    //      the key list once, not twice.
    //    - Two instructions means the instruction array's compact-u16 count is
    //      something other than 1 for the first time.
    //
    //    And it discriminates a rule nothing else here can. `initialize_site`
    //    makes the authority a *writable* signer, so this message has two of
    //    them, and sha256("fee-payer-0") sorts *after* sha256("authority-0").
    //    Compilation pulls the fee payer out and puts it first rather than
    //    sorting it into place, so the fee payer leads despite sorting later.
    //    An encoder that sorts all the writable signers together passes cases
    //    1 and 2 and fails here, which is the whole reason this case exists.
    emit_transaction(
        "two-instructions",
        &[init, inst.clone()],
        &separate_payer,
        &blockhash,
        "",
    );

    println!("  ],");

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
