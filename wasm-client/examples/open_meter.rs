//! Opening a meter: the transaction a reader's wallet signs.
//!
//! Run it with `cargo run --example open_meter`. It prints the three
//! instructions a site's server composes for a new reader -- open a fund,
//! deposit into it, open a meter at this site -- and stops exactly where this
//! crate stops: at unsigned instructions. The server hands them to the
//! reader's wallet through a Solana Pay transaction request (SPEC §4.9).
//!
//! This example exists as much for the compiler as for the reader. It links
//! `sol_pay_client` as an external crate, so it can only reach the public API,
//! and `cargo test` builds it. A change that breaks what an integrator can
//! actually call fails the build rather than waiting for a release.
//!
//! The server's other half -- decoding accounts, preflight, `meter_and_settle`
//! -- is not here, because this crate decodes account bytes but never produces
//! them. Demonstrating the decode path needs real accounts from a cluster.

use sol_pay_client::core::units::{self, UnitsError};
use sol_pay_client::core::Program;
use solana_instruction::Instruction;
use solana_pubkey::Pubkey;

/// Stand-ins. A real integration parses base58 into `Pubkey` with
/// `Pubkey::from_str`: the site's authority and mint from its own
/// configuration, the reader's wallet from the `account` the wallet posts to
/// the transaction-request endpoint, and the browser key from the page, which
/// generated it and sent the public half before drawing the link. Distinct
/// byte patterns keep the printed output readable.
fn placeholder(tag: u8) -> Pubkey {
    Pubkey::new_from_array([tag; 32])
}

/// USDC's scale. Read it from the mint account with `state::mint_decimals`
/// rather than assuming it; a mint with different decimals is not an error,
/// it is a different amount.
const DECIMALS: u8 = 6;

/// What the reader chose on the page. Decimal strings, never floats: `0.1` has
/// no exact binary representation, and a payment library that rounds is not
/// auditable.
const DEPOSIT: &str = "2.00";
const LIMIT: &str = "5.00";

/// Which of the reader's funds, as the page asked them. Never a default
/// (SPEC §4.9): it travels in the transaction-request URL.
const FUND_INDEX: u8 = 0;

/// Unix seconds. The server's clock, plus however long the site offers --
/// an hour on a machine the reader does not own, longer on one they do.
const NOW: i64 = 1_800_000_000;
const EXPIRY: i64 = NOW + 3_600;

fn main() -> Result<(), UnitsError> {
    let authority = placeholder(1); // the site, from its own configuration
    let mint = placeholder(2); // the token the site prices in
    let reader = placeholder(3); // the wallet, from the transaction request
    let reader_token_account = placeholder(4); // their account for that mint
    let browser_key = placeholder(5); // the page's key, posted before the link

    // The deployment and the token program, stated once. `Program::default()`
    // is the canonical deployment on SPL Token; `Program::new(id)` and
    // `.with_token_program(id)` change either independently.
    let pay = Program::default();

    // Addresses are derived, not looked up. There is no registry and no
    // session token: a site is its authority, a fund is its reader, mint and
    // index, and a meter is its site and fund.
    let (site, _bump) = pay.site_address(&authority);
    let (fund, _bump) = pay.fund_address(&reader, &mint, FUND_INDEX);
    let fund_token_account = pay.fund_token_account(&fund, &mint);
    let (meter, _bump) = pay.meter_address(&site, &fund);

    let deposit = units::to_base_units(DEPOSIT, DECIMALS)?;
    let limit = units::to_base_units(LIMIT, DECIMALS)?;

    println!("deployment  {}", pay.id());
    println!("token       {}", pay.token_program());
    println!("site        {site}");
    println!("fund        {fund}  (index {FUND_INDEX})");
    println!("fund tokens {fund_token_account}");
    println!("meter       {meter}");
    println!(
        "deposit     {} base units ({})",
        deposit,
        units::from_base_units(deposit, DECIMALS)
    );
    println!(
        "limit       {} base units ({})",
        limit,
        units::from_base_units(limit, DECIMALS)
    );
    println!();

    // One transaction, in this order. The fund has to exist before the
    // deposit lands in it, which is the one ordering rule the program
    // imposes; `open_fund_and_deposit` states it once. The meter may come
    // anywhere after, since opening one checks no balance.
    let [open_fund, deposit_ix] = pay.open_fund_and_deposit(
        &reader,
        &mint,
        FUND_INDEX,
        &reader_token_account,
        deposit,
        DECIMALS,
    );
    let open_meter = pay.open_meter(&site, &reader, &fund, &browser_key, limit, EXPIRY);

    for (position, instruction) in [open_fund, deposit_ix, open_meter].iter().enumerate() {
        describe(position, instruction);
    }

    // Everything past this point belongs to the integrator. The server
    // compiles these into a message with a fresh blockhash and returns it to
    // the wallet, which shows it, signs and submits. This crate holds no key,
    // opens no connection, and decides only what is being signed.
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
