//! Pay-as-you-go metering for site content.
//!
//! The reader approves this program's meter PDA as delegate on their token
//! account for a limit they choose. The site's server then meters items
//! without the reader present, and transfers only once the unpaid balance is
//! worth a transaction. See `state-machine.plantuml` at the repository root,
//! which is the design document this implements.
//!
//! Two amounts that are easy to confuse:
//!   * the *spending limit* caps `used` and is what the reader authorizes;
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
        item_price: u64,
        collection_threshold: u64,
        min_limit: u64,
    ) -> Result<()> {
        require!(item_price > 0, PayError::ZeroItemPrice);
        // A minimum limit at or below the threshold would let a reader sign up
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
        site.item_price = item_price;
        site.collection_threshold = collection_threshold;
        site.min_limit = min_limit;
        site.bump = ctx.bumps.site;
        Ok(())
    }

    /// Create a reader's meter with this site.
    ///
    /// The client must place an SPL `approve` naming this meter PDA as
    /// delegate *earlier in the same transaction*; this instruction verifies
    /// it rather than trusting the client to have done it.
    pub fn open_meter(ctx: Context<OpenMeter>, limit: u64) -> Result<()> {
        let site = &ctx.accounts.site;
        require!(limit >= site.min_limit, PayError::LimitBelowMinimum);

        let meter_key = ctx.accounts.meter.key();
        require_delegate(&ctx.accounts.reader_token_account, &meter_key, limit)?;

        let meter = &mut ctx.accounts.meter;
        meter.site = site.key();
        meter.reader = ctx.accounts.reader.key();
        meter.limit = limit;
        meter.used = 0;
        meter.paid = 0;
        meter.bump = ctx.bumps.meter;
        Ok(())
    }

    /// Bump usage for `items` and, if that carries the unpaid balance to
    /// the collection threshold, transfer the whole unpaid balance in the same
    /// instruction. Increment and transfer therefore succeed or fail together.
    pub fn meter_and_settle(ctx: Context<MeterAndSettle>, items: u32) -> Result<()> {
        let site = &ctx.accounts.site;

        let charge = site
            .item_price
            .checked_mul(items as u64)
            .ok_or(PayError::MathOverflow)?;
        let new_used = ctx.accounts.meter
            .used
            .checked_add(charge)
            .ok_or(PayError::MathOverflow)?;
        require!(new_used <= ctx.accounts.meter.limit, PayError::LimitReached);

        let unpaid = new_used
            .checked_sub(ctx.accounts.meter.paid)
            .ok_or(PayError::MathOverflow)?;

        let mut transferred = 0u64;
        if unpaid >= site.collection_threshold {
            let site_key = site.key();
            let reader_key = ctx.accounts.reader.key();
            let bump = ctx.accounts.meter.bump;
            let seeds: &[&[u8]] = &[
                METER_SEED,
                site_key.as_ref(),
                reader_key.as_ref(),
                &[bump],
            ];

            token_interface::transfer_checked(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    TransferChecked {
                        from: ctx.accounts.reader_token_account.to_account_info(),
                        mint: ctx.accounts.mint.to_account_info(),
                        to: ctx.accounts.treasury.to_account_info(),
                        // The meter PDA is the delegate the reader approved.
                        authority: ctx.accounts.meter.to_account_info(),
                    },
                    &[seeds],
                ),
                unpaid,
                ctx.accounts.mint.decimals,
            )?;

            transferred = unpaid;
            ctx.accounts.meter.paid = new_used;
        }

        ctx.accounts.meter.used = new_used;

        emit!(Metered {
            meter: ctx.accounts.meter.key(),
            items,
            used: new_used,
            paid: ctx.accounts.meter.paid,
            transferred,
        });
        Ok(())
    }

    /// Renew with a fresh limit.
    ///
    /// Usage already paid for is forgiven from the counter, so the reader
    /// starts the new period owing only the residue that was too small to
    /// collect. A matching SPL `approve` for the new limit must precede this
    /// instruction in the transaction.
    pub fn renew_meter(ctx: Context<RenewMeter>, new_limit: u64) -> Result<()> {
        let site = &ctx.accounts.site;
        require!(new_limit >= site.min_limit, PayError::LimitBelowMinimum);

        let carried = ctx.accounts.meter
            .used
            .checked_sub(ctx.accounts.meter.paid)
            .ok_or(PayError::MathOverflow)?;
        require!(new_limit >= carried, PayError::LimitBelowUsage);

        let meter_key = ctx.accounts.meter.key();
        // Nothing is paid against the new limit yet, so the allowance has to
        // cover all of it.
        require_delegate(&ctx.accounts.reader_token_account, &meter_key, new_limit)?;

        let meter = &mut ctx.accounts.meter;
        meter.used = carried;
        meter.paid = 0;
        meter.limit = new_limit;

        emit!(Renewed {
            meter: meter_key,
            limit: new_limit,
            carried,
        });
        Ok(())
    }

    /// Delete the meter. Any residue is below the collection threshold by
    /// construction, so it is left uncollected rather than transferred.
    /// The reader may revoke the delegate in the same transaction.
    pub fn close_meter(ctx: Context<CloseMeter>) -> Result<()> {
        let meter = &ctx.accounts.meter;
        emit!(Closed {
            meter: meter.key(),
            forgiven: meter.unpaid(),
        });
        Ok(())
    }
}

/// The reader's token account must name `expected` as delegate with at least
/// `needed` still allowed. This is what makes an absent reader chargeable, so
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
pub struct OpenMeter<'info> {
    #[account(mut)]
    pub reader: Signer<'info>,
    pub site: Account<'info, Site>,
    #[account(
        init,
        payer = reader,
        space = 8 + Meter::INIT_SPACE,
        seeds = [METER_SEED, site.key().as_ref(), reader.key().as_ref()],
        bump
    )]
    pub meter: Account<'info, Meter>,
    #[account(
        constraint = reader_token_account.owner == reader.key(),
        constraint = reader_token_account.mint == site.mint,
    )]
    pub reader_token_account: InterfaceAccount<'info, TokenAccount>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct MeterAndSettle<'info> {
    #[account(has_one = authority, has_one = mint)]
    pub site: Account<'info, Site>,
    /// The server meters; the reader is not present.
    pub authority: Signer<'info>,
    /// CHECK: identity only, tied to the meter by `has_one` and used as a seed.
    pub reader: UncheckedAccount<'info>,
    #[account(
        mut,
        has_one = site,
        has_one = reader,
        seeds = [METER_SEED, site.key().as_ref(), reader.key().as_ref()],
        bump = meter.bump
    )]
    pub meter: Account<'info, Meter>,
    #[account(
        mut,
        constraint = reader_token_account.owner == reader.key(),
        constraint = reader_token_account.mint == site.mint,
    )]
    pub reader_token_account: InterfaceAccount<'info, TokenAccount>,
    #[account(mut, address = site.treasury)]
    pub treasury: InterfaceAccount<'info, TokenAccount>,
    pub mint: InterfaceAccount<'info, Mint>,
    pub token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct RenewMeter<'info> {
    #[account(mut)]
    pub reader: Signer<'info>,
    pub site: Account<'info, Site>,
    #[account(
        mut,
        has_one = site,
        has_one = reader,
        seeds = [METER_SEED, site.key().as_ref(), reader.key().as_ref()],
        bump = meter.bump
    )]
    pub meter: Account<'info, Meter>,
    #[account(
        constraint = reader_token_account.owner == reader.key(),
        constraint = reader_token_account.mint == site.mint,
    )]
    pub reader_token_account: InterfaceAccount<'info, TokenAccount>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CloseMeter<'info> {
    #[account(mut)]
    pub reader: Signer<'info>,
    pub site: Account<'info, Site>,
    #[account(
        mut,
        close = reader,
        has_one = site,
        has_one = reader,
        seeds = [METER_SEED, site.key().as_ref(), reader.key().as_ref()],
        bump = meter.bump
    )]
    pub meter: Account<'info, Meter>,
}

#[event]
pub struct Metered {
    pub meter: Pubkey,
    pub items: u32,
    pub used: u64,
    pub paid: u64,
    pub transferred: u64,
}

#[event]
pub struct Renewed {
    pub meter: Pubkey,
    pub limit: u64,
    pub carried: u64,
}

#[event]
pub struct Closed {
    pub meter: Pubkey,
    pub forgiven: u64,
}
