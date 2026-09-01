//! The deployment handle.
//!
//! The program id is a default, not a constraint. An integrator who deploys
//! their own copy of the metering program constructs a [`Program`] with that
//! address, and every derivation, instruction and error name follows it.
//! [`Program::default`] is the canonical deployment, so the common case costs
//! nothing and the override costs one constructor argument.
//!
//! The methods themselves live beside the code they build -- derivation in
//! [`super::pda`], instructions in [`super::ix`], ordered pairs in
//! [`super::tx`], error naming in [`super::error`] -- so each stays next to
//! its own documentation and tests. This module holds only the id.

use solana_pubkey::Pubkey;

use super::ids::PAY_ON_CHAIN_ID;

/// One deployment of the metering program.
///
/// `Copy`, and no larger than the pubkey it wraps, so passing it around costs
/// nothing and nothing needs to borrow it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Program {
    id: Pubkey,
}

impl Program {
    /// The deployment this crate was built against.
    ///
    /// Same value as [`Program::default`], available in const context.
    pub const CANONICAL: Self = Self::new(PAY_ON_CHAIN_ID);

    /// A deployment at `id`.
    ///
    /// Nothing is checked: an address with no program behind it builds
    /// perfectly good instructions that fail at the runtime. Verifying the
    /// deployment is the integrator's job, and needs a network this crate
    /// does not have.
    pub const fn new(id: Pubkey) -> Self {
        Self { id }
    }

    /// The address this handle builds for.
    pub const fn id(&self) -> Pubkey {
        self.id
    }
}

impl Default for Program {
    fn default() -> Self {
        Self::CANONICAL
    }
}

impl From<Pubkey> for Program {
    fn from(id: Pubkey) -> Self {
        Self::new(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_is_the_compiled_in_deployment() {
        assert_eq!(Program::default().id(), PAY_ON_CHAIN_ID);
        assert_eq!(Program::CANONICAL, Program::default());
    }

    #[test]
    fn a_custom_deployment_keeps_the_id_it_was_given() {
        let mine = Pubkey::new_from_array([7u8; 32]);
        assert_eq!(Program::new(mine).id(), mine);
        assert_ne!(Program::new(mine), Program::default());
    }
}
