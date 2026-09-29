use anchor_lang::prelude::*;

#[constant]
pub const SITE_SEED: &[u8] = b"site";

#[constant]
pub const FUND_SEED: &[u8] = b"fund";

#[constant]
pub const METER_SEED: &[u8] = b"meter";

/// The Associated Token Account program. Called by hand rather than through
/// anchor-spl's `associated_token` feature, which needs an
/// spl-associated-token-account release that does not exist (see
/// `tests/Cargo.toml`). Its address and its `CreateIdempotent` instruction
/// are stable parts of Solana; `open_fund` uses both.
///
/// `ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL`, written as its bytes
/// because `solana_program` under anchor-lang 0.32.1 does not re-export the
/// `pubkey!` macro. The parity test checks these bytes against the client's
/// base58 constant, so a typo here fails there.
pub const ASSOCIATED_TOKEN_PROGRAM_ID: Pubkey = Pubkey::new_from_array([
    140, 151, 37, 143, 78, 36, 137, 241, 187, 61, 16, 41, 20, 142, 13, 131, 11, 90, 19, 153,
    218, 255, 16, 132, 4, 142, 123, 216, 219, 233, 248, 89,
]);
