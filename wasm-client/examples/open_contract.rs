//! Opening a contract: the browser's half of an integration.
//!
//! Run it with `cargo run --example open_contract`. It prints the two
//! instructions a payer signs to start a meter, and stops exactly where this
//! crate stops -- at a pair of unsigned instructions.
//!
//! This example exists as much for the compiler as for the reader. It links
//! `sol_pay_client` as an external crate, so it can only reach the public API,
//! and `cargo test` builds it. A change that breaks what an integrator can
//! actually call fails the build rather than waiting for a release.
//!
//! The server's half -- decoding accounts, preflight, `meter_and_settle` --
//! is not here, because this crate decodes account bytes but never produces
//! them. Demonstrating the read path needs real accounts from a cluster.

use sol_pay_client::core::units::{self, UnitsError};
use sol_pay_client::core::Program;
use solana_instruction::Instruction;
use solana_pubkey::Pubkey;

/// Stand-ins. A real integration parses base58 into `Pubkey` with
/// `Pubkey::from_str` -- the site's authority and mint from its own
/// configuration, the payer and their token account from the wallet adapter.
/// Distinct byte patterns keep the printed output readable.
fn placeholder(tag: u8) -> Pubkey {
    Pubkey::new_from_array([tag; 32])
}

/// USDC's scale. Read it from the mint account with `state::mint_decimals`
/// rather than assuming it; a mint with different decimals is not an error,
/// it is a different amount.
const DECIMALS: u8 = 6;

/// What the payer chose. A decimal string, never a float: `0.1` has no exact
/// binary representation, and a payment library that rounds is not auditable.
const LIMIT: &str = "5.00";

fn main() -> Result<(), UnitsError> {
    let authority = placeholder(1); // the site, from its own configuration
    let mint = placeholder(2); // the token the site prices in
    let payer = placeholder(3); // the viewer, from the wallet adapter
    let payer_token_account = placeholder(4); // their account for that mint

    // The deployment and the token program, stated once. `Program::default()`
    // is the canonical deployment on SPL Token; `Program::new(id)` and
    // `.with_token_program(id)` change either independently.
    let pay = Program::default();

    // Addresses are derived, not looked up. There is no registry and no
    // session token: a site is its authority, a contract is its site and
    // payer.
    let (site, _bump) = pay.site_address(&authority);
    let (contract, _bump) = pay.contract_address(&site, &payer);

    let limit = units::to_base_units(LIMIT, DECIMALS)?;

    println!("deployment  {}", pay.id());
    println!("token       {}", pay.token_program());
    println!("site        {site}");
    println!("contract    {contract}");
    println!(
        "limit       {} base units ({} at {} decimals)",
        limit,
        units::from_base_units(limit, DECIMALS),
        DECIMALS
    );
    println!();

    // Two instructions, in this order, in one transaction. The approval must
    // come first: `open_contract` verifies on chain that the token account
    // already names the contract PDA as delegate for the full limit, and
    // fails rather than trusting the client to have done it.
    let instructions = pay.approve_and_open(
        &payer_token_account,
        &mint,
        &payer,
        &site,
        limit,
        DECIMALS,
    );

    for (position, instruction) in instructions.iter().enumerate() {
        describe(position, instruction);
    }

    // Everything past this point belongs to the integrator. The wallet
    // adapter assembles these into a transaction message, adds a blockhash,
    // asks the payer to sign, and submits. This crate holds no key, opens no
    // connection, and decides only what is being signed.
    Ok(())
}

fn describe(position: usize, instruction: &Instruction) {
    println!("instruction {position}");
    println!("  program {}", instruction.program_id);
    for account in &instruction.accounts {
        let mut role = String::new();
        if account.is_signer {
            role.push_str(" signer");
        }
        if account.is_writable {
            role.push_str(" writable");
        }
        println!("  account {}{role}", account.pubkey);
    }
    println!("  data    {} bytes", instruction.data.len());
    println!();
}
