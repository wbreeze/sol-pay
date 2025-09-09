use anchor_lang::prelude::*;
use std::cmp::min;

declare_id!("55ZhZPQ9A75RmDu18fZQLvyAMiooqDKyraYG4DqpNnk3");
type Amount = u32; // up to four SOL, four billion lamports is enough

#[program]
pub mod pay_on_chain {
    use super::*;

    pub fn initialize(
      ctx: Context<Initialize>,
      limit: Amount,
      increment: Amount,
      threshold: Amount
    ) -> Result<()> {
        msg!("Greetings from: {:?}", ctx.program_id);
        ctx.accounts.subscription.set_inner(PAYGo {
          subscriber: *ctx.accounts.subscriber.key,
          charge_limit: limit,
          already_charged: 0,
          used: 0,
          page_increment: increment,
          charge_threshold: threshold
        });
        Ok(())
    }

    pub fn maybe_collect(ctx: Context<PAYGoOp>) -> Result<()> {
      let payg = &mut ctx.accounts.paygo;
      let charge_amount = min(
        payg.charge_limit - payg.already_charged,
        payg.used - payg.already_charged
      );
      if payg.charge_threshold <= charge_amount {
        msg!("Charge: {:?} to {:?}", charge_amount, payg.subscriber);
        payg.already_charged += charge_amount;
      } else {
        msg!("No charges are sensible given:");
        msg!("limit {:?}, used {:?}, already charged {:?} threshold {:?}",
          payg.charge_limit, payg.used, payg.already_charged,
          payg.charge_threshold);
      }
      Ok(())
    }

    pub fn hit_page(ctx: Context<PAYGoOp>) -> Result<()> {
      let payg = &mut ctx.accounts.paygo;
      payg.used += payg.page_increment;
      if payg.charge_threshold <= payg.used - payg.already_charged {
        let _ = maybe_collect(ctx);
      }
      Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
  #[account(mut)]
  pub subscriber: Signer<'info>,

  #[account(
    init,
    payer = subscriber,
    space = 8 + 5 * 4 + 8,
  )]
  pub subscription: Account<'info, PAYGo>,
  pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct PAYGoOp<'info> {
  #[account(mut)]
  pub paygo: Account<'info, PAYGo>,
}

#[account]
pub struct PAYGo {
  pub charge_limit: Amount,
  pub already_charged: Amount,
  pub used: Amount,
  pub page_increment: Amount,
  pub charge_threshold: Amount,
  pub subscriber: Pubkey,
}
