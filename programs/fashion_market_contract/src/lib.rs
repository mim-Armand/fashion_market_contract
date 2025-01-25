use anchor_lang::prelude::*;
use anchor_lang::solana_program::{
    program::invoke,
    system_instruction,
};
use anchor_spl::token_2022::{self, Token2022, Transfer};
use anchor_spl::token_interface::{TokenAccount, Mint};
use anchor_lang::AccountsClose;

use crate::constant::PREFIX;

/// the program ID should be moved out eventually and set based on deployment env ( following best practices )
declare_id!("GfLfsgUP5dQ2gGN4DAPSGZErKSCVZzsVBtof7ZafUP3n");

#[program]
pub mod solana_nft_marketplace {
    use super::*;

    /// Creates a new listing, transferring NFT from usr --> vault (PDA).
    pub fn list_nft(ctx: Context<ListNFT>, price: u64) -> Result<()> {
        /// Transfer 1 NFT seller --> vault
        let transfer_ix = spl_token_2022::instruction::transfer_checked(
            &ctx.accounts.token_program.key(),
            &ctx.accounts.nft_account.key(),
            &ctx.accounts.mint.key(),
            &ctx.accounts.vault.key(),
            &ctx.accounts.seller.key(),
            &[],      // No additional signer
            1,        // 1 NFT
            0,        // indivisible NFT ( decimal points )
        )?;

        // Invoke / execute the transfer.
        invoke(
            &transfer_ix,
            &[
                ctx.accounts.seller.to_account_info(),
                ctx.accounts.vault.to_account_info(),
                ctx.accounts.token_program.to_account_info(),
                ctx.accounts.mint.to_account_info(),
                ctx.accounts.nft_account.to_account_info(),
            ],
        )?;

        // Init listing account data.
        let listing = &mut ctx.accounts.listing;
        listing.seller = *ctx.accounts.seller.key;
        listing.mint = ctx.accounts.mint.key();
        listing.price = price;
        listing.is_active = true;

        Ok(())
    }

    /// Remove NFT by transferring it back: vault (PDA) --> seller.
    pub fn remove_listed_nft(ctx: Context<RemoveListedNFT>) -> Result<()> {
        // Prep PDA seeds for authority sig
        let seeds = &[
            PREFIX.as_bytes(),
            b"vault",
            ctx.accounts.nft_account.mint.as_ref(),
            &[ctx.bumps.vault],
        ];
        let signer = &[&seeds[..]];

        // Transfer back NFT vault --> seller.
        let cpi_accounts = Transfer {
            from: ctx.accounts.vault.to_account_info(),
            to: ctx.accounts.nft_account.to_account_info(),
            authority: ctx.accounts.vault.to_account_info(),
        };
        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            cpi_accounts,
            signer,
        );
        token_2022::transfer(cpi_ctx, 1)?;

        // Close the listing, return rent lamports to seller.
        ctx.accounts.listing.close(ctx.accounts.seller.to_account_info())?;

        Ok(())
    }

    /// Buy NFT = SOL --> seller & NFT --> buyer.
    pub fn buy_nft(ctx: Context<BuyNFT>, vault_bump: u8) -> Result<()> {
        let listing = &mut ctx.accounts.listing;

        // Ensure the listing is still active.
        require!(listing.is_active, ErrorCode::InactiveListing);

        // Transfer SOL (price in SOL * lamports-per-SOL) buyer --> seller.
        // If the price is already in lamports, we need to remove the multiplication below.
        const LAMPORTS_PER_SOL: u64 = 1_000_000_000;
        let transfer_ix = system_instruction::transfer(
            &ctx.accounts.buyer.key(),
            &ctx.accounts.seller.key(),
            listing.price * LAMPORTS_PER_SOL,
        );
        invoke(
            &transfer_ix,
            &[
                ctx.accounts.buyer.to_account_info(),
                ctx.accounts.seller.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
        )?;

        // Transfer NFT = vault --> buyer account.
        let seeds = &[
            PREFIX.as_bytes(),
            b"vault",
            ctx.accounts.nft_account.mint.as_ref(),
            &[vault_bump],
        ];
        let signer = &[&seeds[..]];

        let cpi_accounts = Transfer {
            from: ctx.accounts.vault.to_account_info(),
            to: ctx.accounts.buyer_token_account.to_account_info(),
            authority: ctx.accounts.vault.to_account_info(),
        };
        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            cpi_accounts,
            signer,
        );
        token_2022::transfer(cpi_ctx, 1)?;

        // Mark the listing as inactive so it can't be purchased again.
        listing.is_active = false;

        Ok(())
    }
}

// --------------------------------------------------------------------
// Contexts & Accounts
// --------------------------------------------------------------------
#[derive(Accounts)]
pub struct ListNFT<'info> {
    /// Listing account stores seller, price, etc (on chain).
    #[account(init, payer = seller, space = 80 + 8)]
    pub listing: Account<'info, Listing>,

    #[account(mut)]
    pub seller: Signer<'info>,

    #[account(mut, owner = token_program.key())]
    pub nft_account: InterfaceAccount<'info, TokenAccount>,

    #[account(constraint = mint.key() == nft_account.mint)]
    pub mint: InterfaceAccount<'info, Mint>,

    ///  create NFT vault if not present.
    #[account(
        init_if_needed,
        token::mint = mint,
        payer = seller,
        token::authority = vault,
        seeds = [PREFIX.as_bytes(), b"vault", nft_account.mint.as_ref()],
        bump
    )]
    pub vault: InterfaceAccount<'info, TokenAccount>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token2022>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct RemoveListedNFT<'info> {
    #[account(mut)]
    pub seller: Signer<'info>,

    #[account(mut)]
    pub nft_account: InterfaceAccount<'info, TokenAccount>,

    /// Validate seller is the same as in the listing, & the mint matches.
    #[account(mut, has_one = seller, constraint = nft_account.mint == listing.mint)]
    pub listing: Account<'info, Listing>,

    #[account(constraint = mint.key() == nft_account.mint)]
    pub mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        seeds = [PREFIX.as_bytes(), b"vault", nft_account.mint.as_ref()],
        bump
    )]
    pub vault: InterfaceAccount<'info, TokenAccount>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token2022>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct BuyNFT<'info> {
    #[account(mut)]
    pub listing: Account<'info, Listing>,

    #[account(mut)]
    pub buyer: Signer<'info>,

    /// CHECK: Seller account. Validated to match `listing.seller`.
    #[account(mut)]
    pub seller: AccountInfo<'info>,

    #[account(mut)]
    pub nft_account: InterfaceAccount<'info, TokenAccount>,

    /// PDA vault holding NFT
    #[account(
        mut,
        seeds = [PREFIX.as_bytes(), b"vault", nft_account.mint.as_ref()],
        bump
    )]
    pub vault: InterfaceAccount<'info, TokenAccount>,

    pub token_program: Program<'info, Token2022>,
    pub system_program: Program<'info, System>,

    /// buyer account
    #[account(mut)]
    pub buyer_token_account: InterfaceAccount<'info, TokenAccount>,

    /// seller account
    #[account(mut)]
    pub seller_token_account: InterfaceAccount<'info, TokenAccount>,
}

// --------------------------------------------------------------------
// Data & Errors
// --------------------------------------------------------------------
#[account]
pub struct Listing {
    pub seller: Pubkey,
    pub price: u64,
    pub mint: Pubkey,
    pub is_active: bool,
}

#[error_code]
pub enum ErrorCode {
    #[msg("Listing is not active")]
    InactiveListing,
}

// --------------------------------------------------------------------
// Constants
// --------------------------------------------------------------------
pub mod constant {
    pub const PREFIX: &str = "MARKETPLACE";
}