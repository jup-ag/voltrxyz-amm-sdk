use solana_pubkey::Pubkey;

use crate::errors::VoltrAmmError;

const DISCRIMINATOR_SIZE: usize = 8;

fn read_slice(data: &[u8], start: usize, end: usize) -> Result<&[u8], VoltrAmmError> {
    data.get(start..end)
        .ok_or(VoltrAmmError::InvalidAccountData.into())
}

fn read_array<const N: usize>(data: &[u8], offset: usize) -> Result<[u8; N], VoltrAmmError> {
    let end = offset.checked_add(N).ok_or(VoltrAmmError::MathOverflow)?;
    read_slice(data, offset, end)?
        .try_into()
        .map_err(|_| VoltrAmmError::InvalidAccountData)
}

fn read_pubkey(data: &[u8], offset: usize) -> Result<Pubkey, VoltrAmmError> {
    Ok(Pubkey::new_from_array(read_array::<32>(data, offset)?))
}

fn read_u8(data: &[u8], offset: usize) -> Result<u8, VoltrAmmError> {
    data.get(offset)
        .copied()
        .ok_or(VoltrAmmError::InvalidAccountData.into())
}

fn read_u16(data: &[u8], offset: usize) -> Result<u16, VoltrAmmError> {
    Ok(u16::from_le_bytes(read_array::<2>(data, offset)?))
}

fn read_u64(data: &[u8], offset: usize) -> Result<u64, VoltrAmmError> {
    Ok(u64::from_le_bytes(read_array::<8>(data, offset)?))
}

fn read_u128(data: &[u8], offset: usize) -> Result<u128, VoltrAmmError> {
    Ok(u128::from_le_bytes(read_array::<16>(data, offset)?))
}

#[derive(Clone, Debug)]
pub struct Vault {
    pub asset: VaultAsset,
    pub lp: VaultLp,
    pub vault_configuration: VaultConfiguration,
    pub fee_configuration: FeeConfiguration,
    pub fee_update: FeeUpdate,
    pub fee_state: FeeState,
    pub dead_weight: u64,
    pub high_water_mark: HighWaterMark,
    pub last_updated_ts: u64,
    pub locked_profit_state: LockedProfitState,
}

impl Vault {
    pub fn load(account_data: &[u8]) -> Result<Self, VoltrAmmError> {
        let d = DISCRIMINATOR_SIZE;

        let asset = VaultAsset::load(read_slice(account_data, d + 96, d + 264)?)?;
        let lp = VaultLp::load(read_slice(account_data, d + 264, d + 360)?)?;
        let vault_configuration =
            VaultConfiguration::load(read_slice(account_data, d + 424, d + 504)?)?;
        let fee_configuration =
            FeeConfiguration::load(read_slice(account_data, d + 504, d + 552)?)?;
        let fee_update = FeeUpdate::load(read_slice(account_data, d + 552, d + 568)?)?;
        let fee_state = FeeState::load(read_slice(account_data, d + 568, d + 608)?)?;
        let dead_weight = read_u64(account_data, d + 608)?;
        let high_water_mark = HighWaterMark::load(read_slice(account_data, d + 616, d + 648)?)?;
        let last_updated_ts = read_u64(account_data, d + 648)?;
        let locked_profit_state =
            LockedProfitState::load(read_slice(account_data, d + 664, d + 680)?)?;

        Ok(Vault {
            asset,
            lp,
            vault_configuration,
            fee_configuration,
            fee_update,
            fee_state,
            dead_weight,
            high_water_mark,
            last_updated_ts,
            locked_profit_state,
        })
    }

    pub fn get_total_asset_value(&self) -> u64 {
        self.asset.total_value
    }

    pub fn get_total_accumulated_lp_fees(&self) -> Result<u64, VoltrAmmError> {
        self.fee_state
            .accumulated_lp_admin_fees
            .checked_add(self.fee_state.accumulated_lp_manager_fees)
            .and_then(|s| s.checked_add(self.fee_state.accumulated_lp_protocol_fees))
            .ok_or(VoltrAmmError::MathOverflow.into())
    }

    pub fn get_total_lp_supply_incl_fees(
        &self,
        total_lp_supply_excl_fees: u64,
    ) -> Result<u64, VoltrAmmError> {
        self.get_total_accumulated_lp_fees()?
            .checked_add(total_lp_supply_excl_fees)
            .and_then(|s| s.checked_add(self.dead_weight))
            .ok_or(VoltrAmmError::MathOverflow.into())
    }

    pub fn get_total_fee_configuration_management_fee(&self) -> Result<u16, VoltrAmmError> {
        self.fee_configuration
            .admin_management_fee
            .checked_add(self.fee_configuration.manager_management_fee)
            .and_then(|s| s.checked_add(self.fee_configuration.protocol_management_fee))
            .ok_or(VoltrAmmError::MathOverflow.into())
    }

    pub fn get_unlocked_asset_value(&self, current_ts: u64) -> Result<u64, VoltrAmmError> {
        let locked_profit = self.locked_profit_state.calculate_locked_profit(
            self.vault_configuration.locked_profit_degradation_duration,
            current_ts,
        )?;
        self.asset
            .total_value
            .checked_sub(locked_profit)
            .ok_or(VoltrAmmError::MathOverflow.into())
    }

    pub fn get_total_fee_configuration_performance_fee(&self) -> Result<u16, VoltrAmmError> {
        self.fee_configuration
            .admin_performance_fee
            .checked_add(self.fee_configuration.manager_performance_fee)
            .and_then(|s| s.checked_add(self.fee_configuration.protocol_performance_fee))
            .ok_or(VoltrAmmError::MathOverflow.into())
    }
}

#[derive(Clone, Debug)]
pub struct VaultAsset {
    pub mint: Pubkey,
    pub idle_ata: Pubkey,
    pub total_value: u64,
    pub idle_ata_auth_bump: u8,
}

impl VaultAsset {
    pub fn load(data: &[u8]) -> Result<Self, VoltrAmmError> {
        Ok(VaultAsset {
            mint: read_pubkey(data, 0)?,
            idle_ata: read_pubkey(data, 32)?,
            total_value: read_u64(data, 64)?,
            idle_ata_auth_bump: read_u8(data, 72)?,
        })
    }
}

#[derive(Clone, Debug)]
pub struct VaultLp {
    pub mint: Pubkey,
    pub mint_bump: u8,
    pub mint_auth_bump: u8,
}

impl VaultLp {
    pub fn load(data: &[u8]) -> Result<Self, VoltrAmmError> {
        Ok(VaultLp {
            mint: read_pubkey(data, 0)?,
            mint_bump: read_u8(data, 32)?,
            mint_auth_bump: read_u8(data, 33)?,
        })
    }
}

#[derive(Clone, Debug)]
pub struct VaultConfiguration {
    pub max_cap: u64,
    pub start_at_ts: u64,
    pub locked_profit_degradation_duration: u64,
    pub withdrawal_waiting_period: u64,
    pub disabled_operations: u16,
}

impl VaultConfiguration {
    pub fn load(data: &[u8]) -> Result<Self, VoltrAmmError> {
        Ok(VaultConfiguration {
            max_cap: read_u64(data, 0)?,
            start_at_ts: read_u64(data, 8)?,
            locked_profit_degradation_duration: read_u64(data, 16)?,
            withdrawal_waiting_period: read_u64(data, 24)?,
            disabled_operations: read_u16(data, 32)?,
        })
    }
}

#[derive(Clone, Debug)]
pub struct FeeConfiguration {
    pub manager_performance_fee: u16,
    pub admin_performance_fee: u16,
    pub manager_management_fee: u16,
    pub admin_management_fee: u16,
    pub redemption_fee: u16,
    pub issuance_fee: u16,
    pub protocol_performance_fee: u16,
    pub protocol_management_fee: u16,
}

impl FeeConfiguration {
    pub fn load(data: &[u8]) -> Result<Self, VoltrAmmError> {
        Ok(FeeConfiguration {
            manager_performance_fee: read_u16(data, 0)?,
            admin_performance_fee: read_u16(data, 2)?,
            manager_management_fee: read_u16(data, 4)?,
            admin_management_fee: read_u16(data, 6)?,
            redemption_fee: read_u16(data, 8)?,
            issuance_fee: read_u16(data, 10)?,
            protocol_performance_fee: read_u16(data, 12)?,
            protocol_management_fee: read_u16(data, 14)?,
        })
    }
}

#[derive(Clone, Debug)]
pub struct FeeUpdate {
    pub last_performance_fee_update_ts: u64,
    pub last_management_fee_update_ts: u64,
}

impl FeeUpdate {
    pub fn load(data: &[u8]) -> Result<Self, VoltrAmmError> {
        Ok(FeeUpdate {
            last_performance_fee_update_ts: read_u64(data, 0)?,
            last_management_fee_update_ts: read_u64(data, 8)?,
        })
    }
}

#[derive(Clone, Debug)]
pub struct FeeState {
    pub accumulated_lp_manager_fees: u64,
    pub accumulated_lp_admin_fees: u64,
    pub accumulated_lp_protocol_fees: u64,
}

impl FeeState {
    pub fn load(data: &[u8]) -> Result<Self, VoltrAmmError> {
        Ok(FeeState {
            accumulated_lp_manager_fees: read_u64(data, 0)?,
            accumulated_lp_admin_fees: read_u64(data, 8)?,
            accumulated_lp_protocol_fees: read_u64(data, 16)?,
        })
    }
}

#[derive(Clone, Debug)]
pub struct HighWaterMark {
    pub highest_asset_per_lp_decimal_bits: u128,
    pub last_updated_ts: u64,
}

impl HighWaterMark {
    pub fn load(data: &[u8]) -> Result<Self, VoltrAmmError> {
        Ok(HighWaterMark {
            highest_asset_per_lp_decimal_bits: read_u128(data, 0)?,
            last_updated_ts: read_u64(data, 16)?,
        })
    }
}

#[derive(Clone, Debug)]
pub struct LockedProfitState {
    pub last_updated_locked_profit: u64,
    pub last_report: u64,
}

impl LockedProfitState {
    pub fn load(data: &[u8]) -> Result<Self, VoltrAmmError> {
        Ok(LockedProfitState {
            last_updated_locked_profit: read_u64(data, 0)?,
            last_report: read_u64(data, 8)?,
        })
    }

    pub fn calculate_locked_profit(
        &self,
        locked_profit_degradation_duration: u64,
        current_time: u64,
    ) -> Result<u64, VoltrAmmError> {
        let duration = current_time.saturating_sub(self.last_report) as u128;
        let degradation_duration = locked_profit_degradation_duration as u128;

        if duration > degradation_duration || degradation_duration == 0 {
            return Ok(0);
        }

        let locked_profit = (self.last_updated_locked_profit as u128)
            .checked_mul(degradation_duration.saturating_sub(duration))
            .and_then(|v| v.checked_div(degradation_duration))
            .ok_or(VoltrAmmError::MathOverflow)?;

        Ok(u64::try_from(locked_profit).map_err(|_| VoltrAmmError::MathOverflow)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vault_load_rejects_short_account_data() {
        assert!(Vault::load(&[]).is_err());
    }

    #[test]
    fn vault_asset_load_rejects_short_data() {
        assert!(VaultAsset::load(&[0u8; 8]).is_err());
    }
}
