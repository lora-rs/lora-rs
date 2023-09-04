use super::radio::{PhyRxTx, RfConfig, RxQuality, TxConfig};
use super::region::constants::DEFAULT_DBM;
use super::Timings;

use lora_phy::mod_params::{BoardType, ChipType, RadioError};
use lora_phy::mod_traits::RadioKind;
use lora_phy::LoRa;

/// LoRa radio using the physical layer API in the external lora-phy crate
pub struct LoRaRadio<RK> {
    pub(crate) lora: LoRa<RK>,
}

impl<RK> LoRaRadio<RK>
where
    RK: RadioKind + 'static,
{
    pub fn new(lora: LoRa<RK>) -> Self {
        Self { lora }
    }
}

/// Provide the timing values for boards supported by the external lora-phy crate
impl<RK> Timings for LoRaRadio<RK>
where
    RK: RadioKind + 'static,
{
    fn get_rx_window_offset_ms(&self) -> i32 {
        match self.lora.get_board_type() {
            BoardType::Rak4631Sx1262 => -15,
            BoardType::Stm32l0Sx1276 => -15,
            BoardType::Stm32wlSx1262 => -50,
            _ => -50,
        }
    }
    fn get_rx_window_duration_ms(&self) -> u32 {
        match self.lora.get_board_type() {
            BoardType::Rak4631Sx1262 => 1050,
            BoardType::Stm32l0Sx1276 => 1003,
            BoardType::Stm32wlSx1262 => 1050,
            _ => 1050,
        }
    }
}

/// Provide the LoRa physical layer rx/tx interface for boards supported by the external lora-phy
/// crate
impl<RK> PhyRxTx for LoRaRadio<RK>
where
    RK: RadioKind + 'static,
{
    type PhyError = RadioError;

    async fn tx(&mut self, config: TxConfig, buffer: &[u8]) -> Result<u32, Self::PhyError> {
        let mdltn_params = self.lora.create_modulation_params(
            config.rf.spreading_factor.into(),
            config.rf.bandwidth.into(),
            config.rf.coding_rate.into(),
            config.rf.frequency,
        )?;
        let mut tx_pkt_params =
            self.lora.create_tx_packet_params(8, false, true, false, &mdltn_params)?;
        let pw = match self.lora.get_board_type().into() {
            ChipType::Sx1276 | ChipType::Sx1277 | ChipType::Sx1278 | ChipType::Sx1279 => {
                if config.pw > DEFAULT_DBM {
                    DEFAULT_DBM
                } else {
                    config.pw
                }
            }
            _ => config.pw,
        };
        self.lora.prepare_for_tx(&mdltn_params, pw.into(), false).await?;
        self.lora.tx(&mdltn_params, &mut tx_pkt_params, buffer, 0xffffff).await?;
        Ok(0)
    }

    async fn rx(
        &mut self,
        config: RfConfig,
        receiving_buffer: &mut [u8],
    ) -> Result<(usize, RxQuality), Self::PhyError> {
        let mdltn_params = self.lora.create_modulation_params(
            config.spreading_factor.into(),
            config.bandwidth.into(),
            config.coding_rate.into(),
            config.frequency,
        )?;
        let rx_pkt_params = self.lora.create_rx_packet_params(
            8,
            false,
            receiving_buffer.len() as u8,
            true,
            true,
            &mdltn_params,
        )?;
        self.lora
            .prepare_for_rx(
                &mdltn_params,
                &rx_pkt_params,
                None,
                true, // RX continuous
                false,
                4,
                0x00ffffffu32,
            )
            .await?;
        match self.lora.rx(&rx_pkt_params, receiving_buffer).await {
            Ok((received_len, rx_pkt_status)) => {
                Ok((
                    received_len as usize,
                    RxQuality::new(rx_pkt_status.rssi, rx_pkt_status.snr as i8), // downcast snr
                ))
            }
            Err(err) => Err(err),
        }
    }
}
