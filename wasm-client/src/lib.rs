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
//! Signing is deliberately absent. The reader's wallet signs what the site's
//! server composes, through a Solana Pay transaction request (SPEC §4.9), and
//! the page's browser key signs with WebCrypto; this crate decides *what* is
//! being signed.
//!
//! Anything that depends on *which* deployment of the metering program is
//! being addressed hangs off the `PayOnChain` class; the rest -- decoding,
//! unit conversion, preflight arithmetic, `shortfall`, `verifyKey` -- is free-standing,
//! because it is the same whoever deployed the program.
//!
//! ```js
//! import init, { PayOnChain, canMeter } from "sol-pay-client";
//! await init();
//! const pay = new PayOnChain();          // or new PayOnChain(yourProgramId)
//! const [openFund, deposit] = pay.openFundAndDeposit(...);
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
        item_price: u64,
        collection_threshold: u64,
        min_limit: u64,
        bump: u8,
    }

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct JsFund {
        reader: String,
        mint: String,
        index: u8,
        meters: u32,
        bump: u8,
    }

    /// `expiry` crosses as `BigInt` too: it is an `i64` of Unix seconds.
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct JsMeter {
        site: String,
        fund: String,
        key: String,
        expiry: i64,
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
                item_price: s.item_price,
                collection_threshold: s.collection_threshold,
                min_limit: s.min_limit,
                bump: s.bump,
            }
        }
    }

    impl From<state::Fund> for JsFund {
        fn from(f: state::Fund) -> Self {
            JsFund {
                reader: f.reader.to_string(),
                mint: f.mint.to_string(),
                index: f.index,
                meters: f.meters,
                bump: f.bump,
            }
        }
    }

    impl From<state::Meter> for JsMeter {
        fn from(c: state::Meter) -> Self {
            JsMeter {
                site: c.site.to_string(),
                fund: c.fund.to_string(),
                key: c.key.to_string(),
                expiry: c.expiry,
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

    fn meter_of(data: &[u8]) -> Result<state::Meter, JsError> {
        state::Meter::decode(data).map_err(|e| JsError::new(&e.to_string()))
    }

    /// The SPL Token program. A `PayOnChain` uses this unless told otherwise.
    #[wasm_bindgen(js_name = tokenProgramAddress)]
    pub fn token_program_address() -> String {
        ids::TOKEN_PROGRAM_ID.to_string()
    }

    /// The Token-2022 program, for passing to `withTokenProgram` without
    /// hardcoding a base58 string.
    #[wasm_bindgen(js_name = token2022ProgramAddress)]
    pub fn token_2022_program_address() -> String {
        ids::TOKEN_2022_PROGRAM_ID.to_string()
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

    /// Decode a `Fund` account fetched with `getAccountInfo`.
    #[wasm_bindgen(js_name = decodeFund)]
    pub fn decode_fund(data: &[u8]) -> Result<JsValue, JsError> {
        let fund = state::Fund::decode(data).map_err(|e| JsError::new(&e.to_string()))?;
        serde_wasm_bindgen::to_value(&JsFund::from(fund)).map_err(|e| JsError::new(&e.to_string()))
    }

    /// Decode a `Meter` account fetched with `getAccountInfo`.
    #[wasm_bindgen(js_name = decodeMeter)]
    pub fn decode_meter(data: &[u8]) -> Result<JsValue, JsError> {
        let meter = state::Meter::decode(data).map_err(|e| JsError::new(&e.to_string()))?;
        serde_wasm_bindgen::to_value(&JsMeter::from(meter))
            .map_err(|e| JsError::new(&e.to_string()))
    }

    /// The mint's decimals, which `deposit` needs.
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

    /// What `items` costs, or an error if it does not fit in u64.
    #[wasm_bindgen(js_name = charge)]
    pub fn charge(site_data: &[u8], items: u32) -> Result<u64, JsError> {
        preflight::charge(&site_of(site_data)?, items)
            .ok_or_else(|| JsError::new("charge does not fit in u64"))
    }

    /// `null` when the call would succeed; otherwise why it would not.
    /// `now` is Unix seconds as the server trusts them, as a `BigInt`.
    #[wasm_bindgen(js_name = canMeter)]
    pub fn can_meter(
        site_data: &[u8],
        meter_data: &[u8],
        items: u32,
        now: i64,
    ) -> Result<JsValue, JsError> {
        let site = site_of(site_data)?;
        let meter = meter_of(meter_data)?;
        match preflight::can_meter(&meter, &site, items, now) {
            Ok(()) => Ok(JsValue::NULL),
            Err(preflight::Blocked::Expired) => js(&JsBlocked {
                reason: "expired",
                over: None,
            }),
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
        meter_data: &[u8],
        items: u32,
    ) -> Result<bool, JsError> {
        Ok(preflight::will_settle(
            &meter_of(meter_data)?,
            &site_of(site_data)?,
            items,
        ))
    }

    /// How many more items fit under the limit.
    #[wasm_bindgen(js_name = itemsRemaining)]
    pub fn items_remaining(site_data: &[u8], meter_data: &[u8]) -> Result<u64, JsError> {
        Ok(preflight::items_remaining(
            &meter_of(meter_data)?,
            &site_of(site_data)?,
        ))
    }

    /// The smallest limit this reader may authorize. Pass the meter data
    /// when renewing, and nothing when opening.
    #[wasm_bindgen(js_name = limitFloor)]
    pub fn limit_floor(site_data: &[u8], meter_data: Option<Vec<u8>>) -> Result<u64, JsError> {
        let site = site_of(site_data)?;
        let meter = match meter_data {
            Some(d) => Some(meter_of(&d)?),
            None => None,
        };
        Ok(preflight::limit_floor(&site, meter.as_ref()))
    }

    // --- failures ---------------------------------------------------------

    /// How much the fund's token account is short of a settle of `unpaid`.
    /// Zero when it covers it. Pass the fund's token account data.
    #[wasm_bindgen(js_name = shortfall)]
    pub fn shortfall(token_account_data: &[u8], unpaid: u64) -> Result<u64, JsError> {
        let account = state::TokenAccount::decode(token_account_data)
            .map_err(|e| JsError::new(&e.to_string()))?;
        Ok(error::shortfall(&account, unpaid))
    }

    // --- key proof ------------------------------------------------------

    /// True when `signature` (64 bytes) is a valid Ed25519 signature by `key`
    /// (32 bytes) over `message`. For a Node server checking a page's key
    /// proof; a browser signs with `crypto.subtle` and needs nothing here.
    /// A valid signature is not a live meter: see SPEC §6.6 for the three
    /// checks that follow it. Nor is it fresh: the nonce in `message` is the
    /// server's to accept once and expire within minutes, or a copied proof
    /// reads on the reader's fund.
    #[wasm_bindgen(js_name = verifyKey)]
    pub fn verify_key(key: &[u8], message: &[u8], signature: &[u8]) -> bool {
        match (<&[u8; 32]>::try_from(key), <&[u8; 64]>::try_from(signature)) {
            (Ok(key), Ok(signature)) => crate::core::proof::verify_key(key, message, signature),
            _ => false,
        }
    }

    // --- the deployment ---------------------------------------------------

    /// One deployment of the metering program, the token program the site's
    /// mint belongs to, and everything that depends on either.
    ///
    /// ```js
    /// const pay = new PayOnChain();                    // canonical, SPL Token
    /// const pay = new PayOnChain(myProgramId);         // my own deployment
    /// const pay = new PayOnChain().withTokenProgram(   // a Token-2022 mint
    ///   token2022ProgramAddress(),
    /// );
    /// ```
    ///
    /// Both are defaults, not constraints: a site that deploys its own copy of
    /// the metering program passes that address here, and every derivation,
    /// instruction and error name follows it. Nothing is verified -- an
    /// address with no program behind it builds perfectly good instructions
    /// that fail at the runtime. For the token program there is a cheap check,
    /// `ownsMint`.
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

        /// The token program this instance builds against.
        #[wasm_bindgen(getter, js_name = tokenProgram)]
        pub fn token_program(&self) -> String {
            self.inner.token_program().to_string()
        }

        /// The same deployment, against a different token program. Returns a
        /// new instance; this one is unchanged.
        #[wasm_bindgen(js_name = withTokenProgram)]
        pub fn with_token_program(&self, token_program: &str) -> Result<PayOnChain, JsError> {
            Ok(PayOnChain {
                inner: self
                    .inner
                    .with_token_program(key(token_program, "tokenProgram")?),
            })
        }

        /// Whether a mint account belongs to this instance's token program.
        /// Pass the `owner` that came back beside the mint data from
        /// `getAccountInfo`. A `false` here means every instruction this
        /// instance builds for that mint will fail at the runtime.
        #[wasm_bindgen(js_name = ownsMint)]
        pub fn owns_mint(&self, mint_account_owner: &str) -> Result<bool, JsError> {
            Ok(self
                .inner
                .owns_mint(&key(mint_account_owner, "mintAccountOwner")?))
        }

        // --- derivation ---------------------------------------------------

        #[wasm_bindgen(js_name = deriveSiteAddress)]
        pub fn derive_site_address(&self, authority: &str) -> Result<String, JsError> {
            let authority = key(authority, "authority")?;
            Ok(self.inner.site_address(&authority).0.to_string())
        }

        #[wasm_bindgen(js_name = deriveFundAddress)]
        pub fn derive_fund_address(&self, reader: &str, mint: &str, index: u8) -> Result<String, JsError> {
            Ok(self
                .inner
                .fund_address(&key(reader, "reader")?, &key(mint, "mint")?, index)
                .0
                .to_string())
        }

        /// Where a deposit goes: the fund's associated token account, under
        /// this instance's token program.
        #[wasm_bindgen(js_name = deriveFundTokenAccount)]
        pub fn derive_fund_token_account(&self, fund: &str, mint: &str) -> Result<String, JsError> {
            Ok(self
                .inner
                .fund_token_account(&key(fund, "fund")?, &key(mint, "mint")?)
                .to_string())
        }

        #[wasm_bindgen(js_name = deriveMeterAddress)]
        pub fn derive_meter_address(&self, site: &str, fund: &str) -> Result<String, JsError> {
            let site = key(site, "site")?;
            let fund = key(fund, "fund")?;
            Ok(self.inner.meter_address(&site, &fund).0.to_string())
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
        // The fund must exist before the deposit lands, and this puts it
        // there. The individual builders below stay public.

        #[wasm_bindgen(js_name = openFundAndDeposit)]
        pub fn open_fund_and_deposit(
            &self,
            reader: &str,
            mint: &str,
            index: u8,
            source: &str,
            amount: u64,
            decimals: u8,
        ) -> Result<JsValue, JsError> {
            out_many(self.inner.open_fund_and_deposit(
                &key(reader, "reader")?,
                &key(mint, "mint")?,
                index,
                &key(source, "source")?,
                amount,
                decimals,
            ))
        }

        // --- instructions -------------------------------------------------

        #[wasm_bindgen(js_name = initializeSite)]
        pub fn initialize_site(
            &self,
            authority: &str,
            mint: &str,
            treasury: &str,
            item_price: u64,
            collection_threshold: u64,
            min_limit: u64,
        ) -> Result<JsValue, JsError> {
            out(self.inner.initialize_site(
                &key(authority, "authority")?,
                &key(mint, "mint")?,
                &key(treasury, "treasury")?,
                item_price,
                collection_threshold,
                min_limit,
            ))
        }

        #[wasm_bindgen(js_name = openFund)]
        pub fn open_fund(&self, reader: &str, mint: &str, index: u8) -> Result<JsValue, JsError> {
            out(self
                .inner
                .open_fund(&key(reader, "reader")?, &key(mint, "mint")?, index))
        }

        /// Extend a fund: an SPL transfer from `source`, owned by
        /// `sourceOwner`, into the fund's token account.
        #[wasm_bindgen(js_name = deposit)]
        pub fn deposit(
            &self,
            source: &str,
            source_owner: &str,
            fund: &str,
            mint: &str,
            amount: u64,
            decimals: u8,
        ) -> Result<JsValue, JsError> {
            out(self.inner.deposit(
                &key(source, "source")?,
                &key(source_owner, "sourceOwner")?,
                &key(fund, "fund")?,
                &key(mint, "mint")?,
                amount,
                decimals,
            ))
        }

        #[wasm_bindgen(js_name = withdraw)]
        pub fn withdraw(
            &self,
            reader: &str,
            mint: &str,
            index: u8,
            destination: &str,
            amount: u64,
        ) -> Result<JsValue, JsError> {
            out(self.inner.withdraw(
                &key(reader, "reader")?,
                &key(mint, "mint")?,
                index,
                &key(destination, "destination")?,
                amount,
            ))
        }

        #[wasm_bindgen(js_name = closeFund)]
        pub fn close_fund(&self, reader: &str, mint: &str, index: u8) -> Result<JsValue, JsError> {
            out(self
                .inner
                .close_fund(&key(reader, "reader")?, &key(mint, "mint")?, index))
        }

        /// `key` is the browser's public key, base58; `expiry` is Unix
        /// seconds as a `BigInt`.
        #[wasm_bindgen(js_name = openMeter)]
        pub fn open_meter(
            &self,
            site: &str,
            reader: &str,
            fund: &str,
            key_: &str,
            limit: u64,
            expiry: i64,
        ) -> Result<JsValue, JsError> {
            out(self.inner.open_meter(
                &key(site, "site")?,
                &key(reader, "reader")?,
                &key(fund, "fund")?,
                &key(key_, "key")?,
                limit,
                expiry,
            ))
        }

        #[wasm_bindgen(js_name = meterAndSettle)]
        pub fn meter_and_settle(
            &self,
            site: &str,
            authority: &str,
            fund: &str,
            treasury: &str,
            mint: &str,
            items: u32,
        ) -> Result<JsValue, JsError> {
            out(self.inner.meter_and_settle(
                &key(site, "site")?,
                &key(authority, "authority")?,
                &key(fund, "fund")?,
                &key(treasury, "treasury")?,
                &key(mint, "mint")?,
                items,
            ))
        }

        #[wasm_bindgen(js_name = renewMeter)]
        pub fn renew_meter(
            &self,
            site: &str,
            reader: &str,
            fund: &str,
            key_: &str,
            new_limit: u64,
            expiry: i64,
        ) -> Result<JsValue, JsError> {
            out(self.inner.renew_meter(
                &key(site, "site")?,
                &key(reader, "reader")?,
                &key(fund, "fund")?,
                &key(key_, "key")?,
                new_limit,
                expiry,
            ))
        }

        /// `signer` is the reader or the meter's key. Key-signed, this is
        /// sign-out; the page signs the bytes and the server pays the fee.
        #[wasm_bindgen(js_name = closeMeter)]
        pub fn close_meter(
            &self,
            signer: &str,
            reader: &str,
            site: &str,
            fund: &str,
        ) -> Result<JsValue, JsError> {
            out(self.inner.close_meter(
                &key(signer, "signer")?,
                &key(reader, "reader")?,
                &key(site, "site")?,
                &key(fund, "fund")?,
            ))
        }
    }
}
