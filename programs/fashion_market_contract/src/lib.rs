use anchor_lang::prelude::*;

declare_id!("B5AfjkkfsNFZzuk3Yjd2vFkQmkbKEdcoi5vtztCmtqeM");

#[program]
pub mod fashion_market_contract {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        msg!("Greetings from: {:?}", ctx.program_id);
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize {}
