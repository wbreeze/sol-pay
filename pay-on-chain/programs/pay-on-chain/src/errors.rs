use anchor_lang::prelude::*;

#[error_code]
pub enum PayError {
    #[msg("Limit is below the site minimum")]
    LimitBelowMinimum,
    #[msg("Site minimum limit must exceed the collection threshold")]
    MinimumBelowThreshold,
    #[msg("Item price must be greater than zero")]
    ZeroItemPrice,
    #[msg("Charge would carry usage past the authorized limit")]
    LimitReached,
    #[msg("Reader token account names no delegate")]
    DelegateNotSet,
    #[msg("Reader token account delegates a different authority")]
    DelegateMismatch,
    #[msg("Delegated allowance does not cover the outstanding limit")]
    DelegateAllowanceTooLow,
    #[msg("New limit does not cover usage already accrued")]
    LimitBelowUsage,
    #[msg("Arithmetic overflow")]
    MathOverflow,
}
