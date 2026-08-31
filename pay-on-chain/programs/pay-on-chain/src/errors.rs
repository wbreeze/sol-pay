use anchor_lang::prelude::*;

#[error_code]
pub enum PayError {
    #[msg("Limit is below the site minimum")]
    LimitBelowMinimum,
    #[msg("Site minimum limit must exceed the collection threshold")]
    MinimumBelowThreshold,
    #[msg("Page price must be greater than zero")]
    ZeroPagePrice,
    #[msg("Charge would carry usage past the authorized limit")]
    LimitReached,
    #[msg("Payer token account names no delegate")]
    DelegateNotSet,
    #[msg("Payer token account delegates a different authority")]
    DelegateMismatch,
    #[msg("Delegated allowance does not cover the outstanding limit")]
    DelegateAllowanceTooLow,
    #[msg("New limit does not cover usage already accrued")]
    LimitBelowUsage,
    #[msg("Arithmetic overflow")]
    MathOverflow,
}
