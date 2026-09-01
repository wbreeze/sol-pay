//! The deployment handle.
//!
//! Two things every builder needs and neither of which changes between calls:
//! which deployment of the metering program is being addressed, and which SPL
//! token program the site's mint belongs to. Both are defaults rather than
//! constraints, and both are stated once here instead of at every call site.
//!
//! On the token program specifically: it is a property of the *mint*, not of
//! the deployment -- a mint is owned by either SPL Token or Token-2022, and
//! since one deployment serves many sites and each site names its own mint,
//! two sites on one deployment could in principle differ. It lives here
//! anyway, because a client instance serves one site, and repeating the same
//! word at nine call sites to say so is worse. A caller who really does span
//! both holds two handles.
//!
//! The methods themselves live beside the code they build -- derivation in
//! [`super::pda`], instructions in [`super::ix`], ordered pairs in
//! [`super::tx`], error naming in [`super::error`] -- so each stays next to
//! its own documentation and tests. This module holds only the state.

use solana_pubkey::Pubkey;

use super::ids::{PAY_ON_CHAIN_ID, TOKEN_PROGRAM_ID};

/// One deployment of the metering program, and the token program its mint
/// belongs to.
///
/// `Copy`, and no larger than the two pubkeys it wraps, so passing it around
/// costs nothing and nothing needs to borrow it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Program {
    id: Pubkey,
    token_program: Pubkey,
}

impl Program {
    /// The deployment this crate was built against, on SPL Token.
    ///
    /// Same value as [`Program::default`], available in const context.
    pub const CANONICAL: Self = Self::new(PAY_ON_CHAIN_ID);

    /// A deployment at `id`, on SPL Token.
    ///
    /// Nothing is checked: an address with no program behind it builds
    /// perfectly good instructions that fail at the runtime. Verifying the
    /// deployment is the integrator's job, and needs a network this crate
    /// does not have.
    pub const fn new(id: Pubkey) -> Self {
        Self {
            id,
            token_program: TOKEN_PROGRAM_ID,
        }
    }

    /// The same deployment, against a different token program.
    ///
    /// For a site whose mint is a Token-2022 mint:
    ///
    /// ```
    /// use sol_pay_client::core::{ids, Program};
    ///
    /// let pay = Program::default().with_token_program(ids::TOKEN_2022_PROGRAM_ID);
    /// assert_eq!(pay.token_program(), ids::TOKEN_2022_PROGRAM_ID);
    /// ```
    ///
    /// Which one a mint belongs to is not a preference: it is the mint
    /// account's owner, and passing the other one builds instructions the
    /// runtime rejects. See [`Program::owns_mint`].
    pub const fn with_token_program(self, token_program: Pubkey) -> Self {
        Self {
            id: self.id,
            token_program,
        }
    }

    /// The address this handle builds for.
    pub const fn id(&self) -> Pubkey {
        self.id
    }

    /// The token program this handle builds against.
    pub const fn token_program(&self) -> Pubkey {
        self.token_program
    }

    /// Whether a mint account belongs to the token program this handle is
    /// configured for.
    ///
    /// Pass the `owner` that came back beside the mint's data from
    /// `getAccountInfo`. The token program used to be an argument at every
    /// call site, where getting it wrong was at least visible; now that it is
    /// state set once, this is the cheap check that it was set right. A
    /// mismatch means every instruction this handle builds for that mint will
    /// fail at the runtime.
    pub fn owns_mint(&self, mint_account_owner: &Pubkey) -> bool {
        *mint_account_owner == self.token_program
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
    use crate::core::ids::TOKEN_2022_PROGRAM_ID;

    #[test]
    fn the_default_is_the_compiled_in_deployment_on_spl_token() {
        assert_eq!(Program::default().id(), PAY_ON_CHAIN_ID);
        assert_eq!(Program::default().token_program(), TOKEN_PROGRAM_ID);
        assert_eq!(Program::CANONICAL, Program::default());
    }

    #[test]
    fn a_custom_deployment_keeps_the_id_it_was_given() {
        let mine = Pubkey::new_from_array([7u8; 32]);
        assert_eq!(Program::new(mine).id(), mine);
        assert_ne!(Program::new(mine), Program::default());
    }

    /// The two axes are independent: a custom deployment on SPL Token, and
    /// the canonical deployment on Token-2022, are both reachable.
    #[test]
    fn the_deployment_and_the_token_program_vary_separately() {
        let mine = Pubkey::new_from_array([7u8; 32]);

        let t22 = Program::default().with_token_program(TOKEN_2022_PROGRAM_ID);
        assert_eq!(t22.id(), PAY_ON_CHAIN_ID, "deployment is untouched");
        assert_eq!(t22.token_program(), TOKEN_2022_PROGRAM_ID);

        let both = Program::new(mine).with_token_program(TOKEN_2022_PROGRAM_ID);
        assert_eq!(both.id(), mine);
        assert_eq!(both.token_program(), TOKEN_2022_PROGRAM_ID);

        assert_eq!(Program::new(mine).token_program(), TOKEN_PROGRAM_ID);
    }

    #[test]
    fn a_mint_belongs_to_exactly_one_of_them() {
        let pay = Program::default();
        assert!(pay.owns_mint(&TOKEN_PROGRAM_ID));
        assert!(!pay.owns_mint(&TOKEN_2022_PROGRAM_ID));

        let t22 = pay.with_token_program(TOKEN_2022_PROGRAM_ID);
        assert!(t22.owns_mint(&TOKEN_2022_PROGRAM_ID));
        assert!(!t22.owns_mint(&TOKEN_PROGRAM_ID));
    }
}
