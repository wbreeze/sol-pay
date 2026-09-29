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

use super::ids::{TOKEN_2022_PROGRAM_ID, TOKEN_PROGRAM_ID};
use super::program::Program;
use super::state::TokenAccount;

/// This program's errors, in declaration order. Anchor gives the first the
/// code 6000; the parity tests pin every one against the program's own
/// discriminant rather than trusting that offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PayError {
    LimitBelowMinimum,
    MinimumBelowThreshold,
    ZeroItemPrice,
    LimitReached,
    LimitBelowUsage,
    MathOverflow,
    MintMismatch,
    Expired,
    ExpiryInPast,
    Unauthorized,
    FundNotEmpty,
    FundHasMeters,
}

/// Where Anchor starts numbering `#[error_code]` variants.
pub const ANCHOR_ERROR_BASE: u32 = 6000;

impl PayError {
    pub fn from_code(code: u32) -> Option<Self> {
        use PayError::*;
        Some(match code.checked_sub(ANCHOR_ERROR_BASE)? {
            0 => LimitBelowMinimum,
            1 => MinimumBelowThreshold,
            2 => ZeroItemPrice,
            3 => LimitReached,
            4 => LimitBelowUsage,
            5 => MathOverflow,
            6 => MintMismatch,
            7 => Expired,
            8 => ExpiryInPast,
            9 => Unauthorized,
            10 => FundNotEmpty,
            11 => FundHasMeters,
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
            ZeroItemPrice => "Item price must be greater than zero",
            LimitReached => "Charge would carry usage past the authorized limit",
            LimitBelowUsage => "New limit does not cover usage already accrued",
            MathOverflow => "Arithmetic overflow",
            MintMismatch => "The site and the fund are in different mints",
            Expired => "The meter is past its expiry",
            ExpiryInPast => "The expiry has already passed",
            Unauthorized => "Signer is neither the fund's reader nor the meter's key",
            FundNotEmpty => "The fund still holds a balance",
            FundHasMeters => "The fund still has meters open",
        }
    }
}

/// The SPL Token errors this flow can actually provoke. Not the whole enum:
/// naming codes sol-pay cannot cause would invite guessing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenError {
    /// Code 1. From a settle, it means one thing now that there is no
    /// allowance: the fund's token account holds less than the unpaid
    /// balance. [`shortfall`] says by how much.
    InsufficientFunds,
    /// Code 3. The token account is for a different mint than the site's.
    MintMismatch,
    /// Code 4. The signing authority does not own the source account.
    OwnerMismatch,
    /// Code 17.
    AccountFrozen,
    /// Code 18. Usually a client passing the wrong `decimals` to `deposit` or
    /// `withdraw`.
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
            InsufficientFunds => "Insufficient funds",
            MintMismatch => "Token account is for a different mint",
            OwnerMismatch => "Wrong owner",
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

impl Program {
    /// Name a failure, given the program that raised it and its code.
    ///
    /// `raised_by` is matched against *this deployment's* address rather than
    /// the compiled-in one. That is the whole reason this is a method: a site
    /// running its own deployment would otherwise see every one of its own
    /// program's errors reported as [`Cause::Unknown`], and lose the named
    /// failures this module exists to provide.
    pub fn cause(&self, raised_by: &Pubkey, code: u32) -> Cause {
        let known = if *raised_by == self.id() {
            PayError::from_code(code).map(Cause::Program)
        } else if *raised_by == TOKEN_PROGRAM_ID || *raised_by == TOKEN_2022_PROGRAM_ID {
            TokenError::from_code(code).map(Cause::Token)
        } else {
            None
        };
        known.unwrap_or(Cause::Unknown {
            program: *raised_by,
            code,
        })
    }
}

/// Name a failure raised under the canonical deployment. See
/// [`Program::cause`] for any other.
pub fn cause(program: &Pubkey, code: u32) -> Cause {
    Program::default().cause(program, code)
}

/// How much the fund's token account is short of the next settle, given
/// what that settle would move. Zero when the balance covers it.
///
/// A number rather than a verdict: the site decides what to say. It takes a
/// decoded account so decoding stays in `state` and a caller who already
/// fetched the account does not decode it twice. The other way a settle can
/// fail for lack of money is a frozen account, which is a [`TokenError`] of
/// its own and needs no arithmetic.
pub fn shortfall(fund_token_account: &TokenAccount, unpaid: u64) -> u64 {
    unpaid.saturating_sub(fund_token_account.amount)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ids::PAY_ON_CHAIN_ID;

    #[test]
    fn pay_error_codes_round_trip() {
        for e in [
            PayError::LimitBelowMinimum,
            PayError::LimitReached,
            PayError::MathOverflow,
            PayError::Expired,
            PayError::FundHasMeters,
        ] {
            assert_eq!(PayError::from_code(e.code()), Some(e));
        }
        assert_eq!(PayError::LimitReached.code(), 6003);
        assert_eq!(PayError::FundHasMeters.code(), 6011);
        assert_eq!(PayError::from_code(5999), None);
        assert_eq!(PayError::from_code(6012), None);
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

    /// The reason `cause` hangs off the deployment. A site running its own
    /// copy of the program must get its own errors named; the canonical
    /// handle, looking at that same address, must not claim them.
    #[test]
    fn errors_are_named_against_the_deployment_that_raised_them() {
        let other = Pubkey::new_from_array([9u8; 32]);
        let mine = Program::new(other);

        assert_eq!(
            mine.cause(&other, 6003),
            Cause::Program(PayError::LimitReached),
            "a deployment names its own errors"
        );
        assert!(
            matches!(cause(&other, 6003), Cause::Unknown { .. }),
            "the canonical deployment does not claim another's"
        );
        assert!(
            matches!(mine.cause(&PAY_ON_CHAIN_ID, 6003), Cause::Unknown { .. }),
            "and the relationship is not symmetric by accident"
        );

        // SPL is shared ground: both handles name token errors identically.
        assert_eq!(
            mine.cause(&TOKEN_PROGRAM_ID, 1),
            cause(&TOKEN_PROGRAM_ID, 1)
        );
    }

    fn account(amount: u64) -> TokenAccount {
        TokenAccount {
            mint: Pubkey::new_from_array([1u8; 32]),
            owner: Pubkey::new_from_array([2u8; 32]),
            amount,
            delegate: None,
            delegated_amount: 0,
        }
    }

    #[test]
    fn shortfall_is_what_the_balance_lacks() {
        assert_eq!(shortfall(&account(40), 100), 60);
        assert_eq!(shortfall(&account(100), 100), 0);
        assert_eq!(shortfall(&account(500), 100), 0, "never negative");
    }
}
