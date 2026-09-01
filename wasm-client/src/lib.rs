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
//!
//! Anything that depends on *which* deployment of the metering program is
//! being addressed hangs off the `PayOnChain` class; the rest -- decoding,
//! unit conversion, preflight arithmetic, `revoke` -- is free-standing,
//! because it is the same whoever deployed the program.
//!
//! ```js
//! import init, { PayOnChain, canMeter } from "sol-pay-client";
//! await init();
//! const pay = new PayOnChain();          // or new PayOnChain(yourProgramId)
//! const [approve, open] = pay.approveAndOpen(...);
//! ```

pub mod core;

#[cfg(feature = "wasm")]
mod bindings {
    use core::str::FromStr;

    use serde::Serialize;
    use solana_instruction::Instruction;
    use solana_pubkey::Pubkey;
    use wasm_bindgen::prelude::*;

    use crate::core::error;
    use crate::core::ids;
    use crate::core::ix;
    use crate::core::preflight;
    use crate::core::program;
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

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct JsBlocked {
        reason: &'static str,
        #[serde(skip_serializing_if = "Option::is_none")]
        over: Option<u64>,
    }

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct JsShortfall {
        balance_short: u64,
        allowance_short: u64,
        delegate_present: bool,
        is_clear: bool,
    }

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct JsCause {
        /// "program", "token" or "unknown".
        kind: &'static str,
        code: u32,
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        program: Option<String>,
    }

    impl From<error::Cause> for JsCause {
        fn from(c: error::Cause) -> Self {
            match c {
                error::Cause::Program(e) => JsCause {
                    kind: "program",
                    code: e.code(),
                    message: e.message().to_string(),
                    name: Some(format!("{e:?}")),
                    program: None,
                },
                error::Cause::Token(e) => JsCause {
                    kind: "token",
                    code: e.code(),
                    message: e.message().to_string(),
                    name: Some(format!("{e:?}")),
                    program: None,
                },
                error::Cause::Unknown { program, code } => JsCause {
                    kind: "unknown",
                    code,
                    message: "error from a program this library does not know".to_string(),
                    name: None,
                    program: Some(program.to_string()),
                },
            }
        }
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

    fn out_many(ixs: impl IntoIterator<Item = Instruction>) -> Result<JsValue, JsError> {
        let v: Vec<JsInstruction> = ixs.into_iter().map(JsInstruction::from).collect();
        serde_wasm_bindgen::to_value(&v).map_err(|e| JsError::new(&e.to_string()))
    }

    fn js<T: Serialize>(value: &T) -> Result<JsValue, JsError> {
        serde_wasm_bindgen::to_value(value).map_err(|e| JsError::new(&e.to_string()))
    }

    fn site_of(data: &[u8]) -> Result<state::Site, JsError> {
        state::Site::decode(data).map_err(|e| JsError::new(&e.to_string()))
    }

    fn contract_of(data: &[u8]) -> Result<state::Contract, JsError> {
        state::Contract::decode(data).map_err(|e| JsError::new(&e.to_string()))
    }

    #[wasm_bindgen(js_name = tokenProgramAddress)]
    pub fn token_program_address() -> String {
        ids::TOKEN_PROGRAM_ID.to_string()
    }

    // --- accounts ---------------------------------------------------------
    //
    // Account layouts do not vary by deployment, so these stay free.

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

    // --- preflight --------------------------------------------------------
    //
    // Facts, not instructions. Nothing here says what to render, and none of
    // it depends on the deployment: it is arithmetic over accounts already
    // fetched and decoded.

    /// What `pageViews` costs, or an error if it does not fit in u64.
    #[wasm_bindgen(js_name = charge)]
    pub fn charge(site_data: &[u8], page_views: u32) -> Result<u64, JsError> {
        preflight::charge(&site_of(site_data)?, page_views)
            .ok_or_else(|| JsError::new("charge does not fit in u64"))
    }

    /// `null` when the call would succeed; otherwise why it would not.
    #[wasm_bindgen(js_name = canMeter)]
    pub fn can_meter(
        site_data: &[u8],
        contract_data: &[u8],
        page_views: u32,
    ) -> Result<JsValue, JsError> {
        let site = site_of(site_data)?;
        let contract = contract_of(contract_data)?;
        match preflight::can_meter(&contract, &site, page_views) {
            Ok(()) => Ok(JsValue::NULL),
            Err(preflight::Blocked::LimitReached { over }) => js(&JsBlocked {
                reason: "limitReached",
                over: Some(over),
            }),
            Err(preflight::Blocked::Overflow) => js(&JsBlocked {
                reason: "overflow",
                over: None,
            }),
        }
    }

    /// Whether this call would also move money.
    #[wasm_bindgen(js_name = willSettle)]
    pub fn will_settle(
        site_data: &[u8],
        contract_data: &[u8],
        page_views: u32,
    ) -> Result<bool, JsError> {
        Ok(preflight::will_settle(
            &contract_of(contract_data)?,
            &site_of(site_data)?,
            page_views,
        ))
    }

    /// How many more views fit under the limit.
    #[wasm_bindgen(js_name = viewsRemaining)]
    pub fn views_remaining(site_data: &[u8], contract_data: &[u8]) -> Result<u64, JsError> {
        Ok(preflight::views_remaining(
            &contract_of(contract_data)?,
            &site_of(site_data)?,
        ))
    }

    /// The smallest limit this payer may authorize. Pass the contract data
    /// when renewing, and nothing when opening.
    #[wasm_bindgen(js_name = limitFloor)]
    pub fn limit_floor(site_data: &[u8], contract_data: Option<Vec<u8>>) -> Result<u64, JsError> {
        let site = site_of(site_data)?;
        let contract = match contract_data {
            Some(d) => Some(contract_of(&d)?),
            None => None,
        };
        Ok(preflight::limit_floor(&site, contract.as_ref()))
    }

    // --- failures ---------------------------------------------------------

    /// Which constraint on the payer's token account is short, and by how
    /// much. SPL reports a short balance and a short allowance identically,
    /// so this reads the account rather than guessing from the code.
    ///
    /// Deployment-independent: it reads an SPL token account, and the amount
    /// it compares against is one the caller already has.
    #[wasm_bindgen(js_name = diagnose)]
    pub fn diagnose(token_account_data: &[u8], unpaid: u64) -> Result<JsValue, JsError> {
        let account = state::TokenAccount::decode(token_account_data)
            .map_err(|e| JsError::new(&e.to_string()))?;
        let s = error::diagnose(&account, unpaid);
        js(&JsShortfall {
            balance_short: s.balance_short,
            allowance_short: s.allowance_short,
            delegate_present: s.delegate_present,
            is_clear: s.is_clear(),
        })
    }

    // --- instructions with no deployment in them --------------------------

    /// Withdraw the authorization. Names only the token account and its
    /// owner, so it says nothing about which deployment held the allowance.
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

    // --- the deployment ---------------------------------------------------

    /// One deployment of the metering program, and everything that depends on
    /// which deployment that is.
    ///
    /// ```js
    /// const pay = new PayOnChain();              // the canonical deployment
    /// const pay = new PayOnChain("YourPr0gram…"); // your own
    /// ```
    ///
    /// The program id is a default, not a constraint: a site that deploys its
    /// own copy of the metering program passes that address here, and every
    /// derivation, instruction and error name follows it. Nothing is verified
    /// -- an address with no program behind it builds perfectly good
    /// instructions that fail at the runtime.
    #[wasm_bindgen]
    pub struct PayOnChain {
        inner: program::Program,
    }

    #[wasm_bindgen]
    impl PayOnChain {
        /// Pass nothing for the deployment this package was built against.
        #[wasm_bindgen(constructor)]
        pub fn new(program_id: Option<String>) -> Result<PayOnChain, JsError> {
            let inner = match program_id {
                Some(id) => program::Program::new(key(&id, "programId")?),
                None => program::Program::default(),
            };
            Ok(PayOnChain { inner })
        }

        /// The address this instance builds for.
        #[wasm_bindgen(getter, js_name = programAddress)]
        pub fn program_address(&self) -> String {
            self.inner.id().to_string()
        }

        // --- derivation ---------------------------------------------------

        #[wasm_bindgen(js_name = deriveSiteAddress)]
        pub fn derive_site_address(&self, authority: &str) -> Result<String, JsError> {
            let authority = key(authority, "authority")?;
            Ok(self.inner.site_address(&authority).0.to_string())
        }

        #[wasm_bindgen(js_name = deriveContractAddress)]
        pub fn derive_contract_address(&self, site: &str, payer: &str) -> Result<String, JsError> {
            let site = key(site, "site")?;
            let payer = key(payer, "payer")?;
            Ok(self.inner.contract_address(&site, &payer).0.to_string())
        }

        // --- failures -----------------------------------------------------

        /// Name a failure, given the program that raised it and its code. The
        /// program id matters: the same number means different things.
        ///
        /// `raisedBy` is matched against *this instance's* address, so a site
        /// on its own deployment gets its own errors named rather than
        /// reported as unknown.
        #[wasm_bindgen(js_name = cause)]
        pub fn cause(&self, raised_by: &str, code: u32) -> Result<JsValue, JsError> {
            let raised_by = key(raised_by, "raisedBy")?;
            js(&JsCause::from(self.inner.cause(&raised_by, code)))
        }

        // --- transactions -------------------------------------------------
        //
        // The approve must precede the program instruction, and these put it
        // there. The individual builders below stay public.

        #[wasm_bindgen(js_name = approveAndOpen)]
        pub fn approve_and_open(
            &self,
            token_program: &str,
            payer_token_account: &str,
            mint: &str,
            payer: &str,
            site: &str,
            limit: u64,
            decimals: u8,
        ) -> Result<JsValue, JsError> {
            out_many(self.inner.approve_and_open(
                &key(token_program, "tokenProgram")?,
                &key(payer_token_account, "payerTokenAccount")?,
                &key(mint, "mint")?,
                &key(payer, "payer")?,
                &key(site, "site")?,
                limit,
                decimals,
            ))
        }

        #[wasm_bindgen(js_name = approveAndRenew)]
        pub fn approve_and_renew(
            &self,
            token_program: &str,
            payer_token_account: &str,
            mint: &str,
            payer: &str,
            site: &str,
            new_limit: u64,
            decimals: u8,
        ) -> Result<JsValue, JsError> {
            out_many(self.inner.approve_and_renew(
                &key(token_program, "tokenProgram")?,
                &key(payer_token_account, "payerTokenAccount")?,
                &key(mint, "mint")?,
                &key(payer, "payer")?,
                &key(site, "site")?,
                new_limit,
                decimals,
            ))
        }

        #[wasm_bindgen(js_name = closeAndRevoke)]
        pub fn close_and_revoke(
            &self,
            token_program: &str,
            payer_token_account: &str,
            payer: &str,
            site: &str,
        ) -> Result<JsValue, JsError> {
            out_many(self.inner.close_and_revoke(
                &key(token_program, "tokenProgram")?,
                &key(payer_token_account, "payerTokenAccount")?,
                &key(payer, "payer")?,
                &key(site, "site")?,
            ))
        }

        // --- instructions -------------------------------------------------

        #[wasm_bindgen(js_name = initializeSite)]
        pub fn initialize_site(
            &self,
            authority: &str,
            mint: &str,
            treasury: &str,
            page_price: u64,
            collection_threshold: u64,
            min_limit: u64,
        ) -> Result<JsValue, JsError> {
            out(self.inner.initialize_site(
                &key(authority, "authority")?,
                &key(mint, "mint")?,
                &key(treasury, "treasury")?,
                page_price,
                collection_threshold,
                min_limit,
            ))
        }

        /// Authorize the contract PDA to pull up to `amount`. Put this
        /// *before* `openContract` or `renewContract` in the same
        /// transaction.
        #[wasm_bindgen(js_name = approveChecked)]
        pub fn approve_checked(
            &self,
            token_program: &str,
            payer_token_account: &str,
            mint: &str,
            payer: &str,
            site: &str,
            amount: u64,
            decimals: u8,
        ) -> Result<JsValue, JsError> {
            out(self.inner.approve_checked(
                &key(token_program, "tokenProgram")?,
                &key(payer_token_account, "payerTokenAccount")?,
                &key(mint, "mint")?,
                &key(payer, "payer")?,
                &key(site, "site")?,
                amount,
                decimals,
            ))
        }

        #[wasm_bindgen(js_name = openContract)]
        pub fn open_contract(
            &self,
            site: &str,
            payer: &str,
            payer_token_account: &str,
            limit: u64,
        ) -> Result<JsValue, JsError> {
            out(self.inner.open_contract(
                &key(site, "site")?,
                &key(payer, "payer")?,
                &key(payer_token_account, "payerTokenAccount")?,
                limit,
            ))
        }

        #[wasm_bindgen(js_name = meterAndSettle)]
        #[allow(clippy::too_many_arguments)]
        pub fn meter_and_settle(
            &self,
            site: &str,
            authority: &str,
            payer: &str,
            payer_token_account: &str,
            treasury: &str,
            mint: &str,
            token_program: &str,
            page_views: u32,
        ) -> Result<JsValue, JsError> {
            out(self.inner.meter_and_settle(
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
            &self,
            site: &str,
            payer: &str,
            payer_token_account: &str,
            new_limit: u64,
        ) -> Result<JsValue, JsError> {
            out(self.inner.renew_contract(
                &key(site, "site")?,
                &key(payer, "payer")?,
                &key(payer_token_account, "payerTokenAccount")?,
                new_limit,
            ))
        }

        #[wasm_bindgen(js_name = closeContract)]
        pub fn close_contract(&self, site: &str, payer: &str) -> Result<JsValue, JsError> {
            out(self
                .inner
                .close_contract(&key(site, "site")?, &key(payer, "payer")?))
        }
    }
}
