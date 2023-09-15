use super::radio::{PhyRxTx, RfConfig, RxQuality, TxConfig};
use super::region::constants::DEFAULT_DBM;
use super::Timings;

use lora_phy::mod_params::{BoardType, ChipType, RadioError};
use lora_phy::mod_traits::RadioKind;
use lora_phy::{DelayUs, LoRa};

/// LoRa radio using the physical layer API in the external lora-phy crate
pub struct LoRaRadio<RK, DLY>
where
    RK: RadioKind,
    DLY: DelayUs,
{
    pub(crate) lora: LoRa<RK, DLY>,
    rx_pkt_params: Option<lora_phy::mod_params::PacketParams>,
}
impl<RK, DLY> LoRaRadio<RK, DLY>
where
    RK: RadioKind,
    DLY: DelayUs,
{
    pub fn new(lora: LoRa<RK, DLY>) -> Self {
        Self { lora, rx_pkt_params: None }
    }
}

/// Provide the timing values for boards supported by the external lora-phy crate
impl<RK, DLY> Timings for LoRaRadio<RK, DLY>
where
    RK: RadioKind,
    DLY: DelayUs,
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

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Error {
    Radio(RadioError),
    NoRxParams,
}

impl From<RadioError> for Error {
    fn from(err: RadioError) -> Self {
        Error::Radio(err)
    }
}

/// Provide the LoRa physical layer rx/tx interface for boards supported by the external lora-phy
/// crate
impl<RK, DLY> PhyRxTx for LoRaRadio<RK, DLY>
where
    RK: RadioKind,
    DLY: DelayUs,
{
    type PhyError = Error;

    async fn tx(&mut self, config: TxConfig, buffer: &[u8]) -> Result<u32, Self::PhyError> {
        let mdltn_params = self.lora.create_modulation_params(
            config.rf.spreading_factor,
            config.rf.bandwidth,
            config.rf.coding_rate,
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

    async fn setup_rx(&mut self, config: RfConfig) -> Result<(), Self::PhyError> {
        let mdltn_params = self.lora.create_modulation_params(
            config.spreading_factor,
            config.bandwidth,
            config.coding_rate,
            config.frequency,
        )?;
        let rx_pkt_params =
            self.lora.create_rx_packet_params(8, false, 255, true, true, &mdltn_params)?;
        self.lora.prepare_for_rx(&mdltn_params, &rx_pkt_params, None, None, false).await?;
        self.rx_pkt_params = Some(rx_pkt_params);
        Ok(())
    }

    async fn rx(
        &mut self,
        receiving_buffer: &mut [u8],
    ) -> Result<(usize, RxQuality), Self::PhyError> {
        if let Some(rx_params) = &self.rx_pkt_params {
            match self.lora.rx(rx_params, receiving_buffer).await {
                Ok((received_len, rx_pkt_status)) => {
                    Ok((
                        received_len as usize,
                        RxQuality::new(rx_pkt_status.rssi, rx_pkt_status.snr as i8), // downcast snr
                    ))
                }
                Err(err) => Err(err.into()),
            }
        } else {
            Err(Error::NoRxParams)
        }
    }
}
