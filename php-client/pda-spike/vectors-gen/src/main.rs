//! Generates conformance vectors from the published `sol-pay-client` crate, for
//! the PHP implementation in ../php to check itself against.
//!
//!   cargo run --release > ../php/vectors.json
//!   cd ../php && php verify.php vectors.json
//!
//! Inputs are derived as sha256("authority-<i>") and sha256("payer-<i>") so the
//! PHP side can reproduce them without transferring them.

use sha2::{Digest, Sha256};
use sol_pay_client::core::{ids, ix, pda};
use solana_pubkey::Pubkey;

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
    println!("  }}");
    println!("}}");
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}
