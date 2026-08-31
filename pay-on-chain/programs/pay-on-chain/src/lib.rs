//! Pay-as-you-go metering for site content.
//!
//! The payer approves this program's contract PDA as delegate on their token
//! account for a limit they choose. The site's server then meters page views
//! without the payer present, and transfers only once the unpaid balance is
//! worth a transaction. See `state-machine.plantuml` at the repository root,
//! which is the design document this implements.
//!
//! Two amounts that are easy to confuse:
//!   * the *spending limit* caps `used` and is what the payer authorizes;
//!   * the *collection threshold* is the smallest unpaid balance worth
//!     transferring, and exists only to amortize transaction cost.

use anchor_lang::prelude::*;
use anchor_spl::token_interface::{
    self, Mint, TokenAccount, TokenInterface, TransferChecked,
};

pub mod constants;
pub mod errors;
pub mod state;

use crate::constants::*;
use crate::errors::PayError;
use crate::state::*;

declare_id!("F8UDAGgxVTm8Vmh4RmskpMBCFqhRvuTqbDxDCj8UMedL");

#[program]
pub mod pay_on_chain {
    use super::*;

    /// Stand up a site's pricing. Signed by the server authority.
    pub fn initialize_site(
        ctx: Context<InitializeSite>,
        page_price: u64,
        collection_threshold: u64,
        min_limit: u64,
    ) -> Result<()> {
        require!(page_price > 0, PayError::ZeroPagePrice);
        // A minimum limit at or below the threshold would let a payer sign up
        // for less than a single collection, so the first settle could never
        // fire within their limit.
        require!(
            min_limit > collection_threshold,
            PayError::MinimumBelowThreshold
        );

        let site = &mut ctx.accounts.site;
        site.authority = ctx.accounts.authority.key();
        site.mint = ctx.accounts.mint.key();
        site.treasury = ctx.accounts.treasury.key();
        site.page_price = page_price;
        site.collection_threshold = collection_threshold;
        site.min_limit = min_limit;
        site.bump = ctx.bumps.site;
        Ok(())
    }

    /// Create a payer's contract with this site.
    ///
    /// The client must place an SPL `approve` naming this contract PDA as
    /// delegate *earlier in the same transaction*; this instruction verifies
    /// it rather than trusting the client to have done it.
    pub fn open_contract(ctx: Context<OpenContract>, limit: u64) -> Result<()> {
        let site = &ctx.accounts.site;
        require!(limit >= site.min_limit, PayError::LimitBelowMinimum);

        let contract_key = ctx.accounts.contract.key();
        require_delegate(&ctx.accounts.payer_token_account, &contract_key, limit)?;

        let contract = &mut ctx.accounts.contract;
        contract.site = site.key();
        contract.payer = ctx.accounts.payer.key();
        contract.limit = limit;
        contract.used = 0;
        contract.paid = 0;
        contract.bump = ctx.bumps.contract;
        Ok(())
    }

    /// Bump usage for `page_views` and, if that carries the unpaid balance to
    /// the collection threshold, transfer the whole unpaid balance in the same
    /// instruction. Increment and transfer therefore succeed or fail together.
    pub fn meter_and_settle(ctx: Context<MeterAndSettle>, page_views: u32) -> Result<()> {
        let site = &ctx.accounts.site;

        let charge = site
            .page_price
            .checked_mul(page_views as u64)
            .ok_or(PayError::MathOverflow)?;
        let new_used = ctx.accounts.contract
            .used
            .checked_add(charge)
            .ok_or(PayError::MathOverflow)?;
        require!(new_used <= ctx.accounts.contract.limit, PayError::LimitReached);

        let unpaid = new_used
            .checked_sub(ctx.accounts.contract.paid)
            .ok_or(PayError::MathOverflow)?;

        let mut transferred = 0u64;
        if unpaid >= site.collection_threshold {
            let site_key = site.key();
            let payer_key = ctx.accounts.payer.key();
            let bump = ctx.accounts.contract.bump;
            let seeds: &[&[u8]] = &[
                CONTRACT_SEED,
                site_key.as_ref(),
                payer_key.as_ref(),
                &[bump],
            ];

            token_interface::transfer_checked(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    TransferChecked {
                        from: ctx.accounts.payer_token_account.to_account_info(),
                        mint: ctx.accounts.mint.to_account_info(),
                        to: ctx.accounts.treasury.to_account_info(),
                        // The contract PDA is the delegate the payer approved.
                        authority: ctx.accounts.contract.to_account_info(),
                    },
                    &[seeds],
                ),
                unpaid,
                ctx.accounts.mint.decimals,
            )?;

            transferred = unpaid;
            ctx.accounts.contract.paid = new_used;
        }

        ctx.accounts.contract.used = new_used;

        emit!(Metered {
            contract: ctx.accounts.contract.key(),
            page_views,
            used: new_used,
            paid: ctx.accounts.contract.paid,
            transferred,
        });
        Ok(())
    }

    /// Renew with a fresh limit.
    ///
    /// Usage already paid for is forgiven from the counter, so the payer
    /// starts the new period owing only the residue that was too small to
    /// collect. A matching SPL `approve` for the new limit must precede this
    /// instruction in the transaction.
    pub fn renew_contract(ctx: Context<RenewContract>, new_limit: u64) -> Result<()> {
        let site = &ctx.accounts.site;
        require!(new_limit >= site.min_limit, PayError::LimitBelowMinimum);

        let carried = ctx.accounts.contract
            .used
            .checked_sub(ctx.accounts.contract.paid)
            .ok_or(PayError::MathOverflow)?;
        require!(new_limit >= carried, PayError::LimitBelowUsage);

        let contract_key = ctx.accounts.contract.key();
        // Nothing is paid against the new limit yet, so the allowance has to
        // cover all of it.
        require_delegate(&ctx.accounts.payer_token_account, &contract_key, new_limit)?;

        let contract = &mut ctx.accounts.contract;
        contract.used = carried;
        contract.paid = 0;
        contract.limit = new_limit;

        emit!(Renewed {
            contract: contract_key,
            limit: new_limit,
            carried,
        });
        Ok(())
    }

    /// Delete the contract. Any residue is below the collection threshold by
    /// construction, so it is left uncollected rather than transferred.
    /// The payer may revoke the delegate in the same transaction.
    pub fn close_contract(ctx: Context<CloseContract>) -> Result<()> {
        let contract = &ctx.accounts.contract;
        emit!(Closed {
            contract: contract.key(),
            forgiven: contract.unpaid(),
        });
        Ok(())
    }
}

/// The payer's token account must name `expected` as delegate with at least
/// `needed` still allowed. This is what makes an absent payer chargeable, so
/// it is checked on chain rather than assumed.
fn require_delegate(
    token_account: &InterfaceAccount<TokenAccount>,
    expected: &Pubkey,
    needed: u64,
) -> Result<()> {
    let delegate: Option<Pubkey> = token_account.delegate.into();
    let delegate = delegate.ok_or(PayError::DelegateNotSet)?;
    require_keys_eq!(delegate, *expected, PayError::DelegateMismatch);
    require!(
        token_account.delegated_amount >= needed,
        PayError::DelegateAllowanceTooLow
    );
    Ok(())
}

#[derive(Accounts)]
pub struct InitializeSite<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(
        init,
        payer = authority,
        space = 8 + Site::INIT_SPACE,
        seeds = [SITE_SEED, authority.key().as_ref()],
        bump
    )]
    pub site: Account<'info, Site>,
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(constraint = treasury.mint == mint.key())]
    pub treasury: InterfaceAccount<'info, TokenAccount>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct OpenContract<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub site: Account<'info, Site>,
    #[account(
        init,
        payer = payer,
        space = 8 + Contract::INIT_SPACE,
        seeds = [CONTRACT_SEED, site.key().as_ref(), payer.key().as_ref()],
        bump
    )]
    pub contract: Account<'info, Contract>,
    #[account(
        constraint = payer_token_account.owner == payer.key(),
        constraint = payer_token_account.mint == site.mint,
    )]
    pub payer_token_account: InterfaceAccount<'info, TokenAccount>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct MeterAndSettle<'info> {
    #[account(has_one = authority, has_one = mint)]
    pub site: Account<'info, Site>,
    /// The server meters; the payer is not present.
    pub authority: Signer<'info>,
    /// CHECK: identity only, tied to the contract by `has_one` and used as a seed.
    pub payer: UncheckedAccount<'info>,
    #[account(
        mut,
        has_one = site,
        has_one = payer,
        seeds = [CONTRACT_SEED, site.key().as_ref(), payer.key().as_ref()],
        bump = contract.bump
    )]
    pub contract: Account<'info, Contract>,
    #[account(
        mut,
        constraint = payer_token_account.owner == payer.key(),
        constraint = payer_token_account.mint == site.mint,
    )]
    pub payer_token_account: InterfaceAccount<'info, TokenAccount>,
    #[account(mut, address = site.treasury)]
    pub treasury: InterfaceAccount<'info, TokenAccount>,
    pub mint: InterfaceAccount<'info, Mint>,
    pub token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct RenewContract<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub site: Account<'info, Site>,
    #[account(
        mut,
        has_one = site,
        has_one = payer,
        seeds = [CONTRACT_SEED, site.key().as_ref(), payer.key().as_ref()],
        bump = contract.bump
    )]
    pub contract: Account<'info, Contract>,
    #[account(
        constraint = payer_token_account.owner == payer.key(),
        constraint = payer_token_account.mint == site.mint,
    )]
    pub payer_token_account: InterfaceAccount<'info, TokenAccount>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CloseContract<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub site: Account<'info, Site>,
    #[account(
        mut,
        close = payer,
        has_one = site,
        has_one = payer,
        seeds = [CONTRACT_SEED, site.key().as_ref(), payer.key().as_ref()],
        bump = contract.bump
    )]
    pub contract: Account<'info, Contract>,
}

#[event]
pub struct Metered {
    pub contract: Pubkey,
    pub page_views: u32,
    pub used: u64,
    pub paid: u64,
    pub transferred: u64,
}

#[event]
pub struct Renewed {
    pub contract: Pubkey,
    pub limit: u64,
    pub carried: u64,
}

#[event]
pub struct Closed {
    pub contract: Pubkey,
    pub forgiven: u64,
}
