use anchor_lang::prelude::*;

declare_id!("CZYS5ezTK4uP23AxJfTmsbSJXAMnmgar45uXzeBce8Cb");

#[program]
pub mod pay_on_chain {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        msg!("Greetings from: {:?}", ctx.program_id);
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize {}
