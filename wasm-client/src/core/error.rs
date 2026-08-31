//! Naming what went wrong, across two programs.
//!
//! A failed metering call can come from either side of a CPI, and the two use
//! different, overlapping code spaces. Anchor numbers this program's errors
//! from 6000 in declaration order; SPL Token numbers its own from 0. A bare
//! number never says whose it is, so nothing here takes a code alone.
//!
//! Attributing a code to a program means reading transaction logs, which this
//! crate deliberately does not do -- see the README, "Transaction logs are
//! yours to filter". Pass in the program id you pulled out of them.

use solana_pubkey::Pubkey;

use super::ids::{PAY_ON_CHAIN_ID, TOKEN_2022_PROGRAM_ID, TOKEN_PROGRAM_ID};
use super::state::TokenAccount;

/// This program's errors, in declaration order. Anchor gives the first the
/// code 6000; the parity tests pin every one against the program's own
/// discriminant rather than trusting that offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PayError {
    LimitBelowMinimum,
    MinimumBelowThreshold,
    ZeroPagePrice,
    LimitReached,
    DelegateNotSet,
    DelegateMismatch,
    DelegateAllowanceTooLow,
    LimitBelowUsage,
    MathOverflow,
}

/// Where Anchor starts numbering `#[error_code]` variants.
pub const ANCHOR_ERROR_BASE: u32 = 6000;

impl PayError {
    pub fn from_code(code: u32) -> Option<Self> {
        use PayError::*;
        Some(match code.checked_sub(ANCHOR_ERROR_BASE)? {
            0 => LimitBelowMinimum,
            1 => MinimumBelowThreshold,
            2 => ZeroPagePrice,
            3 => LimitReached,
            4 => DelegateNotSet,
            5 => DelegateMismatch,
            6 => DelegateAllowanceTooLow,
            7 => LimitBelowUsage,
            8 => MathOverflow,
            _ => return None,
        })
    }

    pub fn code(&self) -> u32 {
        ANCHOR_ERROR_BASE + *self as u32
    }

    pub fn message(&self) -> &'static str {
        use PayError::*;
        match self {
            LimitBelowMinimum => "Limit is below the site minimum",
            MinimumBelowThreshold => "Site minimum limit must exceed the collection threshold",
            ZeroPagePrice => "Page price must be greater than zero",
            LimitReached => "Charge would carry usage past the authorized limit",
            DelegateNotSet => "Payer token account names no delegate",
            DelegateMismatch => "Payer token account delegates a different authority",
            DelegateAllowanceTooLow => "Delegated allowance does not cover the outstanding limit",
            LimitBelowUsage => "New limit does not cover usage already accrued",
            MathOverflow => "Arithmetic overflow",
        }
    }
}

/// The SPL Token errors this flow can actually provoke. Not the whole enum:
/// naming codes sol-pay cannot cause would invite guessing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenError {
    /// Code 1. Raised both when the payer's balance is too low *and* when the
    /// delegated allowance is too low, which is why [`diagnose`] exists.
    InsufficientFunds,
    /// Code 3. The token account is for a different mint than the site's.
    MintMismatch,
    /// Code 4. Includes the case where the delegate was cleared: SPL drops the
    /// delegate once its allowance reaches zero, and a cleared delegate is no
    /// longer an authority at all.
    OwnerMismatch,
    /// Code 17.
    AccountFrozen,
    /// Code 18. Usually a client passing the wrong `decimals` to
    /// `approve_checked`.
    MintDecimalsMismatch,
}

impl TokenError {
    pub fn from_code(code: u32) -> Option<Self> {
        use TokenError::*;
        Some(match code {
            1 => InsufficientFunds,
            3 => MintMismatch,
            4 => OwnerMismatch,
            17 => AccountFrozen,
            18 => MintDecimalsMismatch,
            _ => return None,
        })
    }

    pub fn code(&self) -> u32 {
        use TokenError::*;
        match self {
            InsufficientFunds => 1,
            MintMismatch => 3,
            OwnerMismatch => 4,
            AccountFrozen => 17,
            MintDecimalsMismatch => 18,
        }
    }

    pub fn message(&self) -> &'static str {
        use TokenError::*;
        match self {
            InsufficientFunds => "Insufficient funds or delegated allowance",
            MintMismatch => "Token account is for a different mint",
            OwnerMismatch => "Wrong owner, or the delegate is no longer set",
            AccountFrozen => "Token account is frozen",
            MintDecimalsMismatch => "Decimals do not match the mint",
        }
    }
}

/// What raised a failure, once the caller has said which program did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cause {
    Program(PayError),
    Token(TokenError),
    /// Deliberate. The runtime can surface errors from programs neither this
    /// crate nor the integrator anticipated, and mapping those onto our own
    /// enum would be a lie.
    Unknown { program: Pubkey, code: u32 },
}

/// Name a failure, given the program that raised it and its code.
pub fn cause(program: &Pubkey, code: u32) -> Cause {
    let known = if *program == PAY_ON_CHAIN_ID {
        PayError::from_code(code).map(Cause::Program)
    } else if *program == TOKEN_PROGRAM_ID || *program == TOKEN_2022_PROGRAM_ID {
        TokenError::from_code(code).map(Cause::Token)
    } else {
        None
    };
    known.unwrap_or(Cause::Unknown {
        program: *program,
        code,
    })
}

/// Which constraint on the payer's token account is short, and by how much.
///
/// A struct rather than a verdict, because both can be short at once and
/// because the response differs: a low balance means top up, a low allowance
/// means re-authorize. This reports the state and leaves the response to the
/// site.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shortfall {
    /// Zero when the balance covers it.
    pub balance_short: u64,
    /// Zero when the allowance covers it.
    pub allowance_short: u64,
    /// False once SPL has cleared the delegate -- which it does the moment the
    /// allowance is spent to zero, as well as on an explicit revoke.
    pub delegate_present: bool,
}

impl Shortfall {
    /// Nothing on this account would stop a transfer of the amount asked about.
    pub fn is_clear(&self) -> bool {
        self.balance_short == 0 && self.allowance_short == 0 && self.delegate_present
    }
}

/// Read the payer's token account and say what would stop a settle of
/// `unpaid`. A read, not a guess: neither shortfall is inferable from the
/// error code, because SPL reports both as `InsufficientFunds`.
pub fn diagnose(account: &TokenAccount, unpaid: u64) -> Shortfall {
    Shortfall {
        balance_short: unpaid.saturating_sub(account.amount),
        allowance_short: unpaid.saturating_sub(account.delegated_amount),
        delegate_present: account.delegate.is_some(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pay_error_codes_round_trip() {
        for e in [
            PayError::LimitBelowMinimum,
            PayError::LimitReached,
            PayError::DelegateAllowanceTooLow,
            PayError::MathOverflow,
        ] {
            assert_eq!(PayError::from_code(e.code()), Some(e));
        }
        assert_eq!(PayError::LimitReached.code(), 6003);
        assert_eq!(PayError::from_code(5999), None);
        assert_eq!(PayError::from_code(6009), None);
        // A code below the base must not wrap around.
        assert_eq!(PayError::from_code(0), None);
    }

    #[test]
    fn token_error_codes_round_trip() {
        for e in [TokenError::InsufficientFunds, TokenError::MintDecimalsMismatch] {
            assert_eq!(TokenError::from_code(e.code()), Some(e));
        }
        assert_eq!(TokenError::from_code(2), None);
    }

    /// The point of the whole module: 1 and 6003 are different programs
    /// speaking, and neither number means anything on its own.
    #[test]
    fn the_same_number_means_different_things_per_program() {
        assert_eq!(
            cause(&PAY_ON_CHAIN_ID, 6003),
            Cause::Program(PayError::LimitReached)
        );
        assert_eq!(
            cause(&TOKEN_PROGRAM_ID, 1),
            Cause::Token(TokenError::InsufficientFunds)
        );
        // Our program never raises 1, so it is not one of ours.
        assert!(matches!(
            cause(&PAY_ON_CHAIN_ID, 1),
            Cause::Unknown { code: 1, .. }
        ));
        // Token-2022 shares the code space.
        assert_eq!(
            cause(&TOKEN_2022_PROGRAM_ID, 1),
            Cause::Token(TokenError::InsufficientFunds)
        );
    }

    #[test]
    fn an_unrecognised_program_stays_unknown() {
        let other = Pubkey::new_from_array([9u8; 32]);
        assert_eq!(
            cause(&other, 6003),
            Cause::Unknown {
                program: other,
                code: 6003
            }
        );
    }

    fn account(amount: u64, delegated: u64, has_delegate: bool) -> TokenAccount {
        TokenAccount {
            mint: Pubkey::new_from_array([1u8; 32]),
            owner: Pubkey::new_from_array([2u8; 32]),
            amount,
            delegate: has_delegate.then(|| Pubkey::new_from_array([3u8; 32])),
            delegated_amount: delegated,
        }
    }

    #[test]
    fn diagnose_separates_what_the_error_code_conflates() {
        // Balance short, allowance fine.
        let d = diagnose(&account(40, 500, true), 100);
        assert_eq!(d.balance_short, 60);
        assert_eq!(d.allowance_short, 0);
        assert!(!d.is_clear());

        // Allowance short, balance fine.
        let d = diagnose(&account(500, 40, true), 100);
        assert_eq!(d.balance_short, 0);
        assert_eq!(d.allowance_short, 60);

        // Both, which a single verdict would have to pick between.
        let d = diagnose(&account(40, 30, true), 100);
        assert_eq!(d.balance_short, 60);
        assert_eq!(d.allowance_short, 70);

        // Spent to zero: SPL clears the delegate.
        let d = diagnose(&account(500, 0, false), 100);
        assert!(!d.delegate_present);
        assert_eq!(d.allowance_short, 100);

        let d = diagnose(&account(500, 500, true), 100);
        assert!(d.is_clear());
    }
}
