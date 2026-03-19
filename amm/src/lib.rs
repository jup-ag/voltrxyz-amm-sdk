use constants::{VAULT_LP_MINT_SEED, VOLTR_VAULT_PROGRAM};
use solana_pubkey::Pubkey;

pub mod constants;
mod errors;
mod math;
pub use math::*;
pub use state::Vault;

pub mod state;

pub fn derive_vault_lp_mint_pda(vault_key: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[VAULT_LP_MINT_SEED, vault_key.as_ref()],
        &VOLTR_VAULT_PROGRAM,
    )
    .0
}
