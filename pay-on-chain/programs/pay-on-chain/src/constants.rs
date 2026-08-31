use anchor_lang::prelude::*;

/// Length in bytes of a bump slug. The slug is a URL-path token, so it is
/// stored as raw bytes and is expected to be URL-safe base64url characters.
pub const SLUG_LEN: usize = 16;

#[constant]
pub const SITE_SEED: &[u8] = b"site";

#[constant]
pub const CONTRACT_SEED: &[u8] = b"contract";

#[constant]
pub const SLUG_SEED: &[u8] = b"slug";
