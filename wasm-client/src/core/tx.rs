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

use super::program::Program;

impl Program {
    /// Authorize, then open. Both signed by the payer.
    pub fn approve_and_open(
        &self,
        payer_token_account: &Pubkey,
        mint: &Pubkey,
        payer: &Pubkey,
        site: &Pubkey,
        limit: u64,
        decimals: u8,
    ) -> [Instruction; 2] {
        [
            self.approve_checked(payer_token_account, mint, payer, site, limit, decimals),
            self.open_contract(site, payer, payer_token_account, limit),
        ]
    }

    /// Re-authorize at the new limit, then renew.
    ///
    /// `approve` *replaces* an allowance rather than adding to it, so the new
    /// limit is passed outright rather than as a difference.
    pub fn approve_and_renew(
        &self,
        payer_token_account: &Pubkey,
        mint: &Pubkey,
        payer: &Pubkey,
        site: &Pubkey,
        new_limit: u64,
        decimals: u8,
    ) -> [Instruction; 2] {
        [
            self.approve_checked(payer_token_account, mint, payer, site, new_limit, decimals),
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
        payer_token_account: &Pubkey,
        payer: &Pubkey,
        site: &Pubkey,
    ) -> [Instruction; 2] {
        [
            self.close_contract(site, payer),
            self.revoke(payer_token_account, payer),
        ]
    }
}

// --- the canonical deployment, on SPL Token ------------------------------

/// Authorize, then open. Both signed by the payer.
pub fn approve_and_open(
    payer_token_account: &Pubkey,
    mint: &Pubkey,
    payer: &Pubkey,
    site: &Pubkey,
    limit: u64,
    decimals: u8,
) -> [Instruction; 2] {
    Program::default().approve_and_open(payer_token_account, mint, payer, site, limit, decimals)
}

/// Re-authorize at the new limit, then renew.
pub fn approve_and_renew(
    payer_token_account: &Pubkey,
    mint: &Pubkey,
    payer: &Pubkey,
    site: &Pubkey,
    new_limit: u64,
    decimals: u8,
) -> [Instruction; 2] {
    Program::default().approve_and_renew(payer_token_account, mint, payer, site, new_limit, decimals)
}

/// Close, then withdraw the approval.
pub fn close_and_revoke(
    payer_token_account: &Pubkey,
    payer: &Pubkey,
    site: &Pubkey,
) -> [Instruction; 2] {
    Program::default().close_and_revoke(payer_token_account, payer, site)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ids::{PAY_ON_CHAIN_ID, TOKEN_2022_PROGRAM_ID, TOKEN_PROGRAM_ID};
    use crate::core::ix;

    fn k(b: u8) -> Pubkey {
        Pubkey::new_from_array([b; 32])
    }

    #[test]
    fn the_approval_comes_first() {
        let t = approve_and_open(&k(1), &k(2), &k(3), &k(4), 500, 6);
        assert_eq!(t[0].program_id, TOKEN_PROGRAM_ID, "approve is first");
        assert_eq!(t[1].program_id, PAY_ON_CHAIN_ID);

        let t = approve_and_renew(&k(1), &k(2), &k(3), &k(4), 900, 6);
        assert_eq!(t[0].program_id, TOKEN_PROGRAM_ID);
        assert_eq!(t[1].program_id, PAY_ON_CHAIN_ID);
    }

    #[test]
    fn the_pair_matches_the_builders_it_wraps() {
        let (ata, mint, payer, site) = (k(1), k(2), k(3), k(4));
        let t = approve_and_open(&ata, &mint, &payer, &site, 500, 6);
        assert_eq!(t[0], ix::approve_checked(&ata, &mint, &payer, &site, 500, 6));
        assert_eq!(t[1], ix::open_contract(&site, &payer, &ata, 500));
    }

    #[test]
    fn closing_revokes_after_the_close() {
        let t = close_and_revoke(&k(1), &k(3), &k(4));
        assert_eq!(t[0].program_id, PAY_ON_CHAIN_ID, "close is first");
        assert_eq!(t[1].program_id, TOKEN_PROGRAM_ID, "revoke follows");
        assert_eq!(t[1], ix::revoke(&k(1), &k(3)));
    }

    /// Both halves of the pair follow the deployment: the program half by its
    /// program id, the SPL half by the PDA it delegates to.
    #[test]
    fn a_pair_stays_within_one_deployment() {
        let mine = Program::new(k(9));
        let t = mine.approve_and_open(&k(1), &k(2), &k(3), &k(4), 500, 6);
        assert_eq!(t[1].program_id, k(9));
        assert_eq!(t[0].accounts[2].pubkey, mine.contract_address(&k(4), &k(3)).0);
    }

    /// And within one token program. A pair built by a Token-2022 handle must
    /// not send half the transaction to SPL Token.
    #[test]
    fn a_pair_stays_within_one_token_program() {
        let t22 = Program::default().with_token_program(TOKEN_2022_PROGRAM_ID);

        let t = t22.approve_and_open(&k(1), &k(2), &k(3), &k(4), 500, 6);
        assert_eq!(t[0].program_id, TOKEN_2022_PROGRAM_ID);
        assert_eq!(t[1].program_id, PAY_ON_CHAIN_ID);

        let t = t22.close_and_revoke(&k(1), &k(3), &k(4));
        assert_eq!(t[0].program_id, PAY_ON_CHAIN_ID);
        assert_eq!(t[1].program_id, TOKEN_2022_PROGRAM_ID, "revoke follows too");
    }
}
