//! Instructions that have to travel in a particular order.
//!
//! Convenience, not a gate: every builder in [`super::ix`] stays public and
//! nothing here is reachable only through these. They exist so the correct
//! thing is also the shortest thing to write.
//!
//! One ordering rule remains now that there is no delegate (SPEC §6.5): the
//! fund's token account has to exist before anything lands in it, so
//! `open_fund` precedes the deposit. The runtime refuses a transfer to an
//! account that does not exist yet with an error that names neither the fund
//! nor the deposit, which is why the pair is written down once, here.
//!
//! `open_meter` and `renew_meter` may sit anywhere in the same transaction
//! relative to the deposit, because neither checks the balance. The setup
//! transaction a reader's wallet signs (SPEC §4.9) is therefore
//! `open_fund_and_deposit` followed by `open_meter` for a new reader,
//! `deposit` then `open_meter` for a reader with a fund, and `deposit` then
//! `renew_meter` for one with a meter at this site.

use solana_instruction::Instruction;
use solana_pubkey::Pubkey;

use super::program::Program;

impl Program {
    /// Open the fund at `index`, then deposit `amount` into it from the
    /// reader's own `source` token account. Both signed by the reader.
    #[allow(clippy::too_many_arguments)]
    pub fn open_fund_and_deposit(
        &self,
        reader: &Pubkey,
        mint: &Pubkey,
        index: u8,
        source: &Pubkey,
        amount: u64,
        decimals: u8,
    ) -> [Instruction; 2] {
        let (fund, _) = self.fund_address(reader, mint, index);
        [
            self.open_fund(reader, mint, index),
            self.deposit(source, reader, &fund, mint, amount, decimals),
        ]
    }
}

// --- the canonical deployment, on SPL Token ------------------------------

/// Open the fund, then deposit into it. Both signed by the reader.
pub fn open_fund_and_deposit(
    reader: &Pubkey,
    mint: &Pubkey,
    index: u8,
    source: &Pubkey,
    amount: u64,
    decimals: u8,
) -> [Instruction; 2] {
    Program::default().open_fund_and_deposit(reader, mint, index, source, amount, decimals)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ids::{PAY_ON_CHAIN_ID, TOKEN_2022_PROGRAM_ID, TOKEN_PROGRAM_ID};
    use crate::core::{ix, pda};

    fn k(b: u8) -> Pubkey {
        Pubkey::new_from_array([b; 32])
    }

    #[test]
    fn the_fund_is_opened_before_anything_lands_in_it() {
        let t = open_fund_and_deposit(&k(1), &k(2), 0, &k(3), 500, 6);
        assert_eq!(t[0].program_id, PAY_ON_CHAIN_ID, "open_fund is first");
        assert_eq!(t[1].program_id, TOKEN_PROGRAM_ID, "the deposit follows");
    }

    #[test]
    fn the_pair_matches_the_builders_it_wraps() {
        let (reader, mint, source) = (k(1), k(2), k(3));
        let (fund, _) = pda::fund_address(&reader, &mint, 4);
        let t = open_fund_and_deposit(&reader, &mint, 4, &source, 500, 6);
        assert_eq!(t[0], ix::open_fund(&reader, &mint, 4));
        assert_eq!(t[1], ix::deposit(&source, &reader, &fund, &mint, 500, 6));
    }

    /// Both halves follow the handle: the program half by its program id, the
    /// SPL half by the fund PDA it deposits into and the token program it
    /// goes to.
    #[test]
    fn a_pair_stays_within_one_deployment_and_one_token_program() {
        let mine = Program::new(k(9)).with_token_program(TOKEN_2022_PROGRAM_ID);
        let t = mine.open_fund_and_deposit(&k(1), &k(2), 0, &k(3), 500, 6);
        assert_eq!(t[0].program_id, k(9));
        assert_eq!(t[1].program_id, TOKEN_2022_PROGRAM_ID);
        let (fund, _) = mine.fund_address(&k(1), &k(2), 0);
        assert_eq!(t[1].accounts[2].pubkey, mine.fund_token_account(&fund, &k(2)));
    }
}
