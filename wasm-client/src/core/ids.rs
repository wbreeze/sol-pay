//! Well-known program addresses, hardcoded rather than pulled in as
//! dependencies to keep the WASM bundle small. These are consensus-stable.

use solana_pubkey::{pubkey, Pubkey};

/// The metering program. Must match `declare_id!` in the on-chain crate.
pub const PAY_ON_CHAIN_ID: Pubkey = pubkey!("F8UDAGgxVTm8Vmh4RmskpMBCFqhRvuTqbDxDCj8UMedL");

pub const SYSTEM_PROGRAM_ID: Pubkey = pubkey!("11111111111111111111111111111111");

pub const TOKEN_PROGRAM_ID: Pubkey = pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");

pub const TOKEN_2022_PROGRAM_ID: Pubkey = pubkey!("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb");
