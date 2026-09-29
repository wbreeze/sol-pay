use anchor_lang::prelude::*;

/// Declaration order is the error code, from 6000. The client and the PHP
/// port carry this table; the parity test and the conformance vectors pin it,
/// so reorder only knowing that every code after the change moves.
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
    #[msg("New limit does not cover usage already accrued")]
    LimitBelowUsage,
    #[msg("Arithmetic overflow")]
    MathOverflow,
    #[msg("The site and the fund are in different mints")]
    MintMismatch,
    #[msg("The meter is past its expiry")]
    Expired,
    #[msg("The expiry has already passed")]
    ExpiryInPast,
    #[msg("Signer is neither the fund's reader nor the meter's key")]
    Unauthorized,
    #[msg("The fund still holds a balance")]
    FundNotEmpty,
    #[msg("The fund still has meters open")]
    FundHasMeters,
}
