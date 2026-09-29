//! Pay-as-you-go metering for site content.
//!
//! The reader's money moves once, into a *fund*: a token account the program
//! controls, one per reader, mint and index. Every site the reader meters
//! draws from that fund within a limit the reader set for that site, through
//! a *meter*; the site's server meters items without the reader present and
//! transfers only once the unpaid balance reaches the site's collection
//! threshold. The reader takes back whatever is left whenever they like.
//! `wasm-client/SPEC.md` §4.7 through §4.9 is the design this implements.
//!
//! Two amounts that are easy to confuse:
//!   * the *limit* caps a meter's `used` and is what the reader sets for a
//!     site;
//!   * the *collection threshold* is the smallest unpaid balance the site
//!     transfers at once (SPEC §4.6 says why it exists).
//!
//! Who is bounded by what, which is the sentence this program has to keep
//! true: a site can take at most its meter's limit before its expiry; a
//! browser key can take nothing; the program can take at most the fund's
//! balance; the reader sets all three.

use anchor_lang::prelude::*;
use anchor_spl::token_interface::{
    self, CloseAccount, Mint, TokenAccount, TokenInterface, TransferChecked,
};

pub mod constants;
pub mod errors;
pub mod state;

use crate::constants::*;
use crate::errors::PayError;
use crate::state::*;

declare_id!("F8UDAGgxVTm8Vmh4RmskpMBCFqhRvuTqbDxDCj8UMedL");

/// The fund's token account: the associated token account of the fund PDA.
/// Derivable from the fund, so the fund records nothing about it.
pub fn fund_token_address(fund: &Pubkey, mint: &Pubkey, token_program: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[fund.as_ref(), token_program.as_ref(), mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    )
    .0
}

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

    /// Create a fund and its token account. Signed and paid for by the reader.
    ///
    /// The caller chooses `index`; one already in use fails at account
    /// creation, so uniqueness is the runtime's guarantee and nothing here
    /// keeps a counter. The token account is created idempotently: anyone may
    /// create an associated token account for any owner, and a stranger doing
    /// so first must not be able to block the reader from opening the fund.
    pub fn open_fund(ctx: Context<OpenFund>, index: u8) -> Result<()> {
        let fund = &mut ctx.accounts.fund;
        fund.reader = ctx.accounts.reader.key();
        fund.mint = ctx.accounts.mint.key();
        fund.index = index;
        fund.meters = 0;
        fund.bump = ctx.bumps.fund;

        let create = anchor_lang::solana_program::instruction::Instruction {
            program_id: ASSOCIATED_TOKEN_PROGRAM_ID,
            accounts: vec![
                anchor_lang::solana_program::instruction::AccountMeta::new(
                    ctx.accounts.reader.key(),
                    true,
                ),
                anchor_lang::solana_program::instruction::AccountMeta::new(
                    ctx.accounts.fund_token_account.key(),
                    false,
                ),
                anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                    ctx.accounts.fund.key(),
                    false,
                ),
                anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                    ctx.accounts.mint.key(),
                    false,
                ),
                anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                    ctx.accounts.system_program.key(),
                    false,
                ),
                anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                    ctx.accounts.token_program.key(),
                    false,
                ),
            ],
            // 1 is CreateIdempotent; 0, Create, fails if the account exists.
            data: vec![1],
        };
        anchor_lang::solana_program::program::invoke(
            &create,
            &[
                ctx.accounts.reader.to_account_info(),
                ctx.accounts.fund_token_account.to_account_info(),
                ctx.accounts.fund.to_account_info(),
                ctx.accounts.mint.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
                ctx.accounts.token_program.to_account_info(),
                ctx.accounts.associated_token_program.to_account_info(),
            ],
        )?;
        Ok(())
    }

    /// Move `amount` out of the fund to any token account of its mint.
    /// Signed by the reader, whenever they like, meters open or not.
    pub fn withdraw(ctx: Context<Withdraw>, amount: u64) -> Result<()> {
        let fund = &ctx.accounts.fund;
        let reader = fund.reader;
        let mint = fund.mint;
        let index = [fund.index];
        let bump = [fund.bump];
        let seeds: &[&[u8]] = &[FUND_SEED, reader.as_ref(), mint.as_ref(), &index, &bump];

        token_interface::transfer_checked(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                TransferChecked {
                    from: ctx.accounts.fund_token_account.to_account_info(),
                    mint: ctx.accounts.mint.to_account_info(),
                    to: ctx.accounts.destination.to_account_info(),
                    authority: ctx.accounts.fund.to_account_info(),
                },
                &[seeds],
            ),
            amount,
            ctx.accounts.mint.decimals,
        )?;
        Ok(())
    }

    /// Close an empty fund with no meters, and its token account. Rent for
    /// both returns to the reader.
    pub fn close_fund(ctx: Context<CloseFund>) -> Result<()> {
        let fund = &ctx.accounts.fund;
        require!(fund.meters == 0, PayError::FundHasMeters);
        require!(
            ctx.accounts.fund_token_account.amount == 0,
            PayError::FundNotEmpty
        );

        let reader = fund.reader;
        let mint = fund.mint;
        let index = [fund.index];
        let bump = [fund.bump];
        let seeds: &[&[u8]] = &[FUND_SEED, reader.as_ref(), mint.as_ref(), &index, &bump];

        token_interface::close_account(CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            CloseAccount {
                account: ctx.accounts.fund_token_account.to_account_info(),
                destination: ctx.accounts.reader.to_account_info(),
                authority: ctx.accounts.fund.to_account_info(),
            },
            &[seeds],
        ))?;
        Ok(())
    }

    /// Open a meter at a site, drawing on a fund. Signed and paid for by the
    /// reader; names the browser key, the limit and the expiry.
    ///
    /// Checks no balance, on purpose: whether to let a reader set a limit
    /// above what they have deposited is the site's call (SPEC §6.3).
    pub fn open_meter(ctx: Context<OpenMeter>, key: Pubkey, limit: u64, expiry: i64) -> Result<()> {
        let site = &ctx.accounts.site;
        require_keys_eq!(ctx.accounts.fund.mint, site.mint, PayError::MintMismatch);
        require!(limit >= site.min_limit, PayError::LimitBelowMinimum);
        require!(
            expiry > Clock::get()?.unix_timestamp,
            PayError::ExpiryInPast
        );

        let meter = &mut ctx.accounts.meter;
        meter.site = site.key();
        meter.fund = ctx.accounts.fund.key();
        meter.key = key;
        meter.expiry = expiry;
        meter.limit = limit;
        meter.used = 0;
        meter.paid = 0;
        meter.bump = ctx.bumps.meter;

        let fund = &mut ctx.accounts.fund;
        fund.meters = fund.meters.checked_add(1).ok_or(PayError::MathOverflow)?;
        Ok(())
    }

    /// Bump usage for `items` and, if that carries the unpaid balance to the
    /// collection threshold, transfer the whole unpaid balance from the fund
    /// in the same instruction. Increment and transfer therefore succeed or
    /// fail together. Signed by the site authority; the reader is absent, and
    /// the fund's seeds, not the reader's signature, authorize the transfer.
    pub fn meter_and_settle(ctx: Context<MeterAndSettle>, items: u32) -> Result<()> {
        let now = Clock::get()?.unix_timestamp;
        require!(!ctx.accounts.meter.expired(now), PayError::Expired);

        let site = &ctx.accounts.site;
        let charge = site
            .item_price
            .checked_mul(items as u64)
            .ok_or(PayError::MathOverflow)?;
        let new_used = ctx
            .accounts
            .meter
            .used
            .checked_add(charge)
            .ok_or(PayError::MathOverflow)?;
        require!(new_used <= ctx.accounts.meter.limit, PayError::LimitReached);

        let unpaid = new_used
            .checked_sub(ctx.accounts.meter.paid)
            .ok_or(PayError::MathOverflow)?;

        let mut transferred = 0u64;
        if unpaid >= site.collection_threshold {
            let fund = &ctx.accounts.fund;
            let reader = fund.reader;
            let mint = fund.mint;
            let index = [fund.index];
            let bump = [fund.bump];
            let seeds: &[&[u8]] = &[FUND_SEED, reader.as_ref(), mint.as_ref(), &index, &bump];

            token_interface::transfer_checked(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    TransferChecked {
                        from: ctx.accounts.fund_token_account.to_account_info(),
                        mint: ctx.accounts.mint.to_account_info(),
                        to: ctx.accounts.treasury.to_account_info(),
                        authority: ctx.accounts.fund.to_account_info(),
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

    /// Renew with a fresh limit, and a key and expiry that may be new.
    ///
    /// Usage already paid for is forgiven from the counter, so the reader
    /// starts the new period owing only the residue that was too small to
    /// collect. Naming a new key is how a second device takes over: the old
    /// device's key is dead the moment this lands (SPEC §4.8). An expired
    /// meter may be renewed; that is what renewal is for.
    pub fn renew_meter(
        ctx: Context<RenewMeter>,
        key: Pubkey,
        new_limit: u64,
        expiry: i64,
    ) -> Result<()> {
        let site = &ctx.accounts.site;
        require_keys_eq!(ctx.accounts.fund.mint, site.mint, PayError::MintMismatch);
        require!(new_limit >= site.min_limit, PayError::LimitBelowMinimum);

        let carried = ctx
            .accounts
            .meter
            .used
            .checked_sub(ctx.accounts.meter.paid)
            .ok_or(PayError::MathOverflow)?;
        require!(new_limit >= carried, PayError::LimitBelowUsage);
        require!(
            expiry > Clock::get()?.unix_timestamp,
            PayError::ExpiryInPast
        );

        let meter = &mut ctx.accounts.meter;
        meter.key = key;
        meter.expiry = expiry;
        meter.used = carried;
        meter.paid = 0;
        meter.limit = new_limit;

        emit!(Renewed {
            meter: meter.key(),
            limit: new_limit,
            carried,
            expiry,
        });
        Ok(())
    }

    /// Delete the meter. Any residue is below the collection threshold by
    /// construction, so it is forgiven rather than transferred, and the rent
    /// returns to the reader.
    ///
    /// Signed by the reader or by the meter's key. The key signing is how a
    /// reader leaves a machine; since it holds no SOL, the site's server pays
    /// the fee (SPEC §4.8).
    pub fn close_meter(ctx: Context<CloseMeter>) -> Result<()> {
        let signer = ctx.accounts.signer.key();
        require!(
            signer == ctx.accounts.fund.reader || signer == ctx.accounts.meter.key,
            PayError::Unauthorized
        );

        let fund = &mut ctx.accounts.fund;
        fund.meters = fund.meters.checked_sub(1).ok_or(PayError::MathOverflow)?;

        let meter = &ctx.accounts.meter;
        emit!(Closed {
            meter: meter.key(),
            forgiven: meter.unpaid(),
        });
        Ok(())
    }
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
#[instruction(index: u8)]
pub struct OpenFund<'info> {
    #[account(mut)]
    pub reader: Signer<'info>,
    #[account(
        init,
        payer = reader,
        space = 8 + Fund::INIT_SPACE,
        seeds = [FUND_SEED, reader.key().as_ref(), mint.key().as_ref(), &[index]],
        bump
    )]
    pub fund: Account<'info, Fund>,
    /// CHECK: created here by the Associated Token Account program; the
    /// address is pinned to the fund's associated token account.
    #[account(
        mut,
        address = fund_token_address(&fund.key(), &mint.key(), &token_program.key())
    )]
    pub fund_token_account: UncheckedAccount<'info>,
    pub mint: InterfaceAccount<'info, Mint>,
    pub token_program: Interface<'info, TokenInterface>,
    /// CHECK: the Associated Token Account program, by address.
    #[account(address = ASSOCIATED_TOKEN_PROGRAM_ID)]
    pub associated_token_program: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Withdraw<'info> {
    pub reader: Signer<'info>,
    #[account(
        has_one = reader @ PayError::Unauthorized,
        seeds = [FUND_SEED, fund.reader.as_ref(), fund.mint.as_ref(), &[fund.index]],
        bump = fund.bump
    )]
    pub fund: Account<'info, Fund>,
    #[account(
        mut,
        constraint = fund_token_account.owner == fund.key(),
        constraint = fund_token_account.mint == fund.mint @ PayError::MintMismatch,
    )]
    pub fund_token_account: InterfaceAccount<'info, TokenAccount>,
    #[account(mut, constraint = destination.mint == fund.mint @ PayError::MintMismatch)]
    pub destination: InterfaceAccount<'info, TokenAccount>,
    #[account(address = fund.mint @ PayError::MintMismatch)]
    pub mint: InterfaceAccount<'info, Mint>,
    pub token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct CloseFund<'info> {
    #[account(mut)]
    pub reader: Signer<'info>,
    #[account(
        mut,
        close = reader,
        has_one = reader @ PayError::Unauthorized,
        seeds = [FUND_SEED, fund.reader.as_ref(), fund.mint.as_ref(), &[fund.index]],
        bump = fund.bump
    )]
    pub fund: Account<'info, Fund>,
    #[account(
        mut,
        address = fund_token_address(&fund.key(), &fund.mint, &token_program.key())
    )]
    pub fund_token_account: InterfaceAccount<'info, TokenAccount>,
    pub token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct OpenMeter<'info> {
    #[account(mut)]
    pub reader: Signer<'info>,
    pub site: Account<'info, Site>,
    #[account(
        mut,
        has_one = reader @ PayError::Unauthorized,
        seeds = [FUND_SEED, fund.reader.as_ref(), fund.mint.as_ref(), &[fund.index]],
        bump = fund.bump
    )]
    pub fund: Account<'info, Fund>,
    #[account(
        init,
        payer = reader,
        space = 8 + Meter::INIT_SPACE,
        seeds = [METER_SEED, site.key().as_ref(), fund.key().as_ref()],
        bump
    )]
    pub meter: Account<'info, Meter>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct MeterAndSettle<'info> {
    #[account(has_one = authority, has_one = mint)]
    pub site: Account<'info, Site>,
    /// The server meters; the reader is not present.
    pub authority: Signer<'info>,
    #[account(
        constraint = fund.mint == mint.key() @ PayError::MintMismatch,
        seeds = [FUND_SEED, fund.reader.as_ref(), fund.mint.as_ref(), &[fund.index]],
        bump = fund.bump
    )]
    pub fund: Account<'info, Fund>,
    #[account(
        mut,
        has_one = site,
        has_one = fund,
        seeds = [METER_SEED, site.key().as_ref(), fund.key().as_ref()],
        bump = meter.bump
    )]
    pub meter: Account<'info, Meter>,
    /// Any token account the fund owns in the site's mint. In practice the
    /// fund's associated token account; the fund's seeds are what can move it.
    #[account(
        mut,
        constraint = fund_token_account.owner == fund.key(),
        constraint = fund_token_account.mint == mint.key() @ PayError::MintMismatch,
    )]
    pub fund_token_account: InterfaceAccount<'info, TokenAccount>,
    #[account(mut, address = site.treasury)]
    pub treasury: InterfaceAccount<'info, TokenAccount>,
    pub mint: InterfaceAccount<'info, Mint>,
    pub token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct RenewMeter<'info> {
    pub reader: Signer<'info>,
    pub site: Account<'info, Site>,
    #[account(
        has_one = reader @ PayError::Unauthorized,
        seeds = [FUND_SEED, fund.reader.as_ref(), fund.mint.as_ref(), &[fund.index]],
        bump = fund.bump
    )]
    pub fund: Account<'info, Fund>,
    #[account(
        mut,
        has_one = site,
        has_one = fund,
        seeds = [METER_SEED, site.key().as_ref(), fund.key().as_ref()],
        bump = meter.bump
    )]
    pub meter: Account<'info, Meter>,
}

#[derive(Accounts)]
pub struct CloseMeter<'info> {
    /// The reader or the meter's key; `close_meter` checks which.
    pub signer: Signer<'info>,
    pub site: Account<'info, Site>,
    #[account(
        mut,
        seeds = [FUND_SEED, fund.reader.as_ref(), fund.mint.as_ref(), &[fund.index]],
        bump = fund.bump
    )]
    pub fund: Account<'info, Fund>,
    /// CHECK: receives the meter's rent; pinned to the fund's reader.
    #[account(mut, address = fund.reader @ PayError::Unauthorized)]
    pub reader: UncheckedAccount<'info>,
    #[account(
        mut,
        close = reader,
        has_one = site,
        has_one = fund,
        seeds = [METER_SEED, site.key().as_ref(), fund.key().as_ref()],
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
    pub expiry: i64,
}

#[event]
pub struct Closed {
    pub meter: Pubkey,
    pub forgiven: u64,
}
