//! WASM surface for the pay-on-chain metering program.
//!
//! Everything of substance lives in [`core`], which knows nothing about the
//! browser. This module only converts to and from JavaScript values, so the
//! same client can later back a Rust UI without rework.
//!
//! Instructions come out shaped like `@solana/kit`'s `IInstruction`:
//!
//! ```js
//! { programAddress: string, accounts: [{ address, role }], data: Uint8Array }
//! ```
//!
//! Signing is deliberately absent. Wallet Standard is browser JavaScript, so
//! the wallet adapter assembles and signs; this crate decides *what* is being
//! signed.

pub mod core;

#[cfg(feature = "wasm")]
mod bindings {
    use core::str::FromStr;

    use serde::Serialize;
    use solana_instruction::Instruction;
    use solana_pubkey::Pubkey;
    use wasm_bindgen::prelude::*;

    use crate::core::ids;
    use crate::core::ix;
    use crate::core::pda;
    use crate::core::state;
    use crate::core::units;

    #[derive(Serialize)]
    struct JsAccountMeta {
        address: String,
        /// Matches kit's AccountRole: signer is bit 1, writable is bit 0.
        role: u8,
    }

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct JsInstruction {
        program_address: String,
        accounts: Vec<JsAccountMeta>,
        #[serde(with = "serde_bytes")]
        data: Vec<u8>,
    }

    /// `u64` crosses as `BigInt`. A JS number loses precision above 2^53, and
    /// a payment library that silently truncates is not one anybody can audit.
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct JsSite {
        authority: String,
        mint: String,
        treasury: String,
        page_price: u64,
        collection_threshold: u64,
        min_limit: u64,
        bump: u8,
    }

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct JsContract {
        site: String,
        payer: String,
        limit: u64,
        used: u64,
        paid: u64,
        bump: u8,
        /// Derived, not stored: used - paid. Every caller wants it.
        unpaid: u64,
    }

    impl From<state::Site> for JsSite {
        fn from(s: state::Site) -> Self {
            JsSite {
                authority: s.authority.to_string(),
                mint: s.mint.to_string(),
                treasury: s.treasury.to_string(),
                page_price: s.page_price,
                collection_threshold: s.collection_threshold,
                min_limit: s.min_limit,
                bump: s.bump,
            }
        }
    }

    impl From<state::Contract> for JsContract {
        fn from(c: state::Contract) -> Self {
            JsContract {
                site: c.site.to_string(),
                payer: c.payer.to_string(),
                limit: c.limit,
                used: c.used,
                paid: c.paid,
                bump: c.bump,
                unpaid: c.unpaid(),
            }
        }
    }

    impl From<Instruction> for JsInstruction {
        fn from(ix: Instruction) -> Self {
            JsInstruction {
                program_address: ix.program_id.to_string(),
                accounts: ix
                    .accounts
                    .iter()
                    .map(|m| JsAccountMeta {
                        address: m.pubkey.to_string(),
                        role: (m.is_signer as u8) << 1 | (m.is_writable as u8),
                    })
                    .collect(),
                data: ix.data,
            }
        }
    }

    fn key(s: &str, what: &str) -> Result<Pubkey, JsError> {
        Pubkey::from_str(s).map_err(|e| JsError::new(&format!("{what}: {e}")))
    }

    fn out(ix: Instruction) -> Result<JsValue, JsError> {
        serde_wasm_bindgen::to_value(&JsInstruction::from(ix))
            .map_err(|e| JsError::new(&e.to_string()))
    }

    #[wasm_bindgen(js_name = programAddress)]
    pub fn program_address() -> String {
        ids::PAY_ON_CHAIN_ID.to_string()
    }

    #[wasm_bindgen(js_name = tokenProgramAddress)]
    pub fn token_program_address() -> String {
        ids::TOKEN_PROGRAM_ID.to_string()
    }

    // --- derivation -------------------------------------------------------

    #[wasm_bindgen(js_name = deriveSiteAddress)]
    pub fn derive_site_address(authority: &str) -> Result<String, JsError> {
        let authority = key(authority, "authority")?;
        Ok(pda::site_address(&authority).0.to_string())
    }

    #[wasm_bindgen(js_name = deriveContractAddress)]
    pub fn derive_contract_address(site: &str, payer: &str) -> Result<String, JsError> {
        let site = key(site, "site")?;
        let payer = key(payer, "payer")?;
        Ok(pda::contract_address(&site, &payer).0.to_string())
    }

    // --- accounts ---------------------------------------------------------

    /// Decode a `Site` account fetched with `getAccountInfo`.
    #[wasm_bindgen(js_name = decodeSite)]
    pub fn decode_site(data: &[u8]) -> Result<JsValue, JsError> {
        let site = state::Site::decode(data).map_err(|e| JsError::new(&e.to_string()))?;
        serde_wasm_bindgen::to_value(&JsSite::from(site))
            .map_err(|e| JsError::new(&e.to_string()))
    }

    /// Decode a `Contract` account fetched with `getAccountInfo`.
    #[wasm_bindgen(js_name = decodeContract)]
    pub fn decode_contract(data: &[u8]) -> Result<JsValue, JsError> {
        let contract = state::Contract::decode(data).map_err(|e| JsError::new(&e.to_string()))?;
        serde_wasm_bindgen::to_value(&JsContract::from(contract))
            .map_err(|e| JsError::new(&e.to_string()))
    }

    /// The mint's decimals, which `approveChecked` needs.
    #[wasm_bindgen(js_name = mintDecimals)]
    pub fn mint_decimals(mint_account_data: &[u8]) -> Result<u8, JsError> {
        state::mint_decimals(mint_account_data).map_err(|e| JsError::new(&e.to_string()))
    }

    // --- amounts ----------------------------------------------------------

    /// Human amount to base units. Takes a string, not a number: `0.1` is not
    /// representable in binary floating point.
    #[wasm_bindgen(js_name = toBaseUnits)]
    pub fn to_base_units(amount: &str, decimals: u8) -> Result<u64, JsError> {
        units::to_base_units(amount, decimals).map_err(|e| JsError::new(&e.to_string()))
    }

    /// Base units back to a decimal string, without trailing zeros.
    #[wasm_bindgen(js_name = fromBaseUnits)]
    pub fn from_base_units(units_: u64, decimals: u8) -> String {
        units::from_base_units(units_, decimals)
    }

    // --- instructions -----------------------------------------------------

    #[wasm_bindgen(js_name = initializeSite)]
    pub fn initialize_site(
        authority: &str,
        mint: &str,
        treasury: &str,
        page_price: u64,
        collection_threshold: u64,
        min_limit: u64,
    ) -> Result<JsValue, JsError> {
        out(ix::initialize_site(
            &key(authority, "authority")?,
            &key(mint, "mint")?,
            &key(treasury, "treasury")?,
            page_price,
            collection_threshold,
            min_limit,
        ))
    }

    /// Authorize the contract PDA to pull up to `amount`. Put this *before*
    /// `openContract` or `renewContract` in the same transaction.
    #[wasm_bindgen(js_name = approveChecked)]
    pub fn approve_checked(
        token_program: &str,
        payer_token_account: &str,
        mint: &str,
        payer: &str,
        site: &str,
        amount: u64,
        decimals: u8,
    ) -> Result<JsValue, JsError> {
        out(ix::approve_checked(
            &key(token_program, "tokenProgram")?,
            &key(payer_token_account, "payerTokenAccount")?,
            &key(mint, "mint")?,
            &key(payer, "payer")?,
            &key(site, "site")?,
            amount,
            decimals,
        ))
    }

    #[wasm_bindgen(js_name = revoke)]
    pub fn revoke(
        token_program: &str,
        payer_token_account: &str,
        payer: &str,
    ) -> Result<JsValue, JsError> {
        out(ix::revoke(
            &key(token_program, "tokenProgram")?,
            &key(payer_token_account, "payerTokenAccount")?,
            &key(payer, "payer")?,
        ))
    }

    #[wasm_bindgen(js_name = openContract)]
    pub fn open_contract(
        site: &str,
        payer: &str,
        payer_token_account: &str,
        limit: u64,
    ) -> Result<JsValue, JsError> {
        out(ix::open_contract(
            &key(site, "site")?,
            &key(payer, "payer")?,
            &key(payer_token_account, "payerTokenAccount")?,
            limit,
        ))
    }

    #[wasm_bindgen(js_name = meterAndSettle)]
    pub fn meter_and_settle(
        site: &str,
        authority: &str,
        payer: &str,
        payer_token_account: &str,
        treasury: &str,
        mint: &str,
        token_program: &str,
        page_views: u32,
    ) -> Result<JsValue, JsError> {
        out(ix::meter_and_settle(
            &key(site, "site")?,
            &key(authority, "authority")?,
            &key(payer, "payer")?,
            &key(payer_token_account, "payerTokenAccount")?,
            &key(treasury, "treasury")?,
            &key(mint, "mint")?,
            &key(token_program, "tokenProgram")?,
            page_views,
        ))
    }

    #[wasm_bindgen(js_name = renewContract)]
    pub fn renew_contract(
        site: &str,
        payer: &str,
        payer_token_account: &str,
        new_limit: u64,
    ) -> Result<JsValue, JsError> {
        out(ix::renew_contract(
            &key(site, "site")?,
            &key(payer, "payer")?,
            &key(payer_token_account, "payerTokenAccount")?,
            new_limit,
        ))
    }

    #[wasm_bindgen(js_name = closeContract)]
    pub fn close_contract(site: &str, payer: &str) -> Result<JsValue, JsError> {
        out(ix::close_contract(&key(site, "site")?, &key(payer, "payer")?))
    }
}
