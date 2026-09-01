//! Whole transactions, in the order the program requires.
//!
//! Convenience, not a gate: every builder in [`super::ix`] stays public and
//! nothing here is reachable only through these. They exist so the correct
//! thing is also the shortest thing to write.
//!
//! The rule they encode is the one an integrator gets wrong once and then
//! debugs for an hour: `open_contract` and `renew_contract` verify on chain
//! that the payer's token account already names the contract PDA as delegate,
//! and they fail rather than trust the client to have done it. The approval
//! must therefore come *earlier in the same transaction*.
//!
//! The names say the pair and its order outright -- `approve_and_open`, not
//! `open_contract` again -- because these sit on [`Program`] alongside the
//! single-instruction builders they wrap.

use solana_instruction::Instruction;
use solana_pubkey::Pubkey;

use super::ix;
use super::program::Program;

impl Program {
    /// Authorize, then open. Both signed by the payer.
    #[allow(clippy::too_many_arguments)]
    pub fn approve_and_open(
        &self,
        token_program: &Pubkey,
        payer_token_account: &Pubkey,
        mint: &Pubkey,
        payer: &Pubkey,
        site: &Pubkey,
        limit: u64,
        decimals: u8,
    ) -> [Instruction; 2] {
        [
            self.approve_checked(
                token_program,
                payer_token_account,
                mint,
                payer,
                site,
                limit,
                decimals,
            ),
            self.open_contract(site, payer, payer_token_account, limit),
        ]
    }

    /// Re-authorize at the new limit, then renew.
    ///
    /// `approve` *replaces* an allowance rather than adding to it, so the new
    /// limit is passed outright rather than as a difference.
    #[allow(clippy::too_many_arguments)]
    pub fn approve_and_renew(
        &self,
        token_program: &Pubkey,
        payer_token_account: &Pubkey,
        mint: &Pubkey,
        payer: &Pubkey,
        site: &Pubkey,
        new_limit: u64,
        decimals: u8,
    ) -> [Instruction; 2] {
        [
            self.approve_checked(
                token_program,
                payer_token_account,
                mint,
                payer,
                site,
                new_limit,
                decimals,
            ),
            self.renew_contract(site, payer, payer_token_account, new_limit),
        ]
    }

    /// Close, then withdraw the approval.
    ///
    /// The leftover approval is inert once the contract account is gone -- the
    /// PDA can no longer sign -- but it stays visible in the payer's wallet
    /// until revoked, and a token account has exactly one delegate, so leaving
    /// it in place blocks the payer opening a contract with another site.
    pub fn close_and_revoke(
        &self,
        token_program: &Pubkey,
        payer_token_account: &Pubkey,
        payer: &Pubkey,
        site: &Pubkey,
    ) -> [Instruction; 2] {
        [
            self.close_contract(site, payer),
            ix::revoke(token_program, payer_token_account, payer),
        ]
    }
}

// --- the canonical deployment --------------------------------------------

/// Authorize, then open. Both signed by the payer.
#[allow(clippy::too_many_arguments)]
pub fn approve_and_open(
    token_program: &Pubkey,
    payer_token_account: &Pubkey,
    mint: &Pubkey,
    payer: &Pubkey,
    site: &Pubkey,
    limit: u64,
    decimals: u8,
) -> [Instruction; 2] {
    Program::default().approve_and_open(
        token_program,
        payer_token_account,
        mint,
        payer,
        site,
        limit,
        decimals,
    )
}

/// Re-authorize at the new limit, then renew.
#[allow(clippy::too_many_arguments)]
pub fn approve_and_renew(
    token_program: &Pubkey,
    payer_token_account: &Pubkey,
    mint: &Pubkey,
    payer: &Pubkey,
    site: &Pubkey,
    new_limit: u64,
    decimals: u8,
) -> [Instruction; 2] {
    Program::default().approve_and_renew(
        token_program,
        payer_token_account,
        mint,
        payer,
        site,
        new_limit,
        decimals,
    )
}

/// Close, then withdraw the approval.
pub fn close_and_revoke(
    token_program: &Pubkey,
    payer_token_account: &Pubkey,
    payer: &Pubkey,
    site: &Pubkey,
) -> [Instruction; 2] {
    Program::default().close_and_revoke(token_program, payer_token_account, payer, site)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ids::{PAY_ON_CHAIN_ID, TOKEN_PROGRAM_ID};

    fn k(b: u8) -> Pubkey {
        Pubkey::new_from_array([b; 32])
    }

    #[test]
    fn the_approval_comes_first() {
        let t = approve_and_open(&TOKEN_PROGRAM_ID, &k(1), &k(2), &k(3), &k(4), 500, 6);
        assert_eq!(t[0].program_id, TOKEN_PROGRAM_ID, "approve is first");
        assert_eq!(t[1].program_id, PAY_ON_CHAIN_ID);

        let t = approve_and_renew(&TOKEN_PROGRAM_ID, &k(1), &k(2), &k(3), &k(4), 900, 6);
        assert_eq!(t[0].program_id, TOKEN_PROGRAM_ID);
        assert_eq!(t[1].program_id, PAY_ON_CHAIN_ID);
    }

    #[test]
    fn the_pair_matches_the_builders_it_wraps() {
        let (tp, ata, mint, payer, site) = (TOKEN_PROGRAM_ID, k(1), k(2), k(3), k(4));
        let t = approve_and_open(&tp, &ata, &mint, &payer, &site, 500, 6);
        assert_eq!(
            t[0],
            ix::approve_checked(&tp, &ata, &mint, &payer, &site, 500, 6)
        );
        assert_eq!(t[1], ix::open_contract(&site, &payer, &ata, 500));
    }

    #[test]
    fn closing_revokes_after_the_close() {
        let t = close_and_revoke(&TOKEN_PROGRAM_ID, &k(1), &k(3), &k(4));
        assert_eq!(t[0].program_id, PAY_ON_CHAIN_ID, "close is first");
        assert_eq!(t[1].program_id, TOKEN_PROGRAM_ID, "revoke follows");
        assert_eq!(t[1], ix::revoke(&TOKEN_PROGRAM_ID, &k(1), &k(3)));
    }

    /// Both halves of the pair follow the deployment: the program half by its
    /// program id, the SPL half by the PDA it delegates to.
    #[test]
    fn a_pair_stays_within_one_deployment() {
        let mine = Program::new(k(9));
        let t = mine.approve_and_open(&TOKEN_PROGRAM_ID, &k(1), &k(2), &k(3), &k(4), 500, 6);
        assert_eq!(t[1].program_id, k(9));
        assert_eq!(t[0].accounts[2].pubkey, mine.contract_address(&k(4), &k(3)).0);
    }
}
