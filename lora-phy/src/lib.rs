#![no_std]
#![allow(async_fn_in_trait)]
#![warn(missing_docs)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

#[cfg(feature = "lorawan-radio")]
#[cfg_attr(docsrs, doc(cfg(feature = "lorawan-radio")))]
/// Provides an implementation of the async LoRaWAN device trait.
pub mod lorawan_radio;

/// The read/write interface between an embedded framework/MCU combination and a LoRa chip
pub(crate) mod interface;
/// InterfaceVariant implementations using `embedded-hal`.
pub mod iv;
/// Parameters used across the lora-phy crate to support various use cases
pub mod mod_params;
/// Traits implemented externally or internally to support control of LoRa chips
pub mod mod_traits;
/// Specific implementation to support Semtech Sx126x chips
pub mod sx126x;
/// Specific implementation to support Semtech Sx127x chips
pub mod sx127x;

pub use crate::mod_params::RxMode;

pub use embedded_hal_async::delay::DelayNs;
use interface::*;
use mod_params::*;
use mod_traits::*;

/// Provides the physical layer API to support LoRa chips
pub struct LoRa<RK, DLY>
where
    RK: RadioKind,
    DLY: DelayNs,
{
    radio_kind: RK,
    delay: DLY,
    radio_mode: RadioMode,
    enable_public_network: bool,
    cold_start: bool,
    calibrate_image: bool,
}

impl<RK, DLY> LoRa<RK, DLY>
where
    RK: RadioKind,
    DLY: DelayNs,
{
    /// Build and return a new instance of the LoRa physical layer API to control an initialized LoRa radio
    pub async fn new(radio_kind: RK, enable_public_network: bool, delay: DLY) -> Result<Self, RadioError> {
        let mut lora = Self {
            radio_kind,
            delay,
            radio_mode: RadioMode::Sleep,
            enable_public_network,
            cold_start: true,
            calibrate_image: true,
        };
        lora.init().await?;

        Ok(lora)
    }

    /// Wait for an IRQ event to occur
    pub async fn wait_for_irq(&mut self) -> Result<(), RadioError> {
        self.radio_kind.await_irq().await
    }

    /// Process an IRQ event and return the new state of the radio
    pub async fn process_irq_event(&mut self) -> Result<Option<IrqState>, RadioError> {
        self.radio_kind.process_irq_event(self.radio_mode, None, false).await
    }

    /// Create modulation parameters for a communication channel
    pub fn create_modulation_params(
        &mut self,
        spreading_factor: SpreadingFactor,
        bandwidth: Bandwidth,
        coding_rate: CodingRate,
        frequency_in_hz: u32,
    ) -> Result<ModulationParams, RadioError> {
        self.radio_kind
            .create_modulation_params(spreading_factor, bandwidth, coding_rate, frequency_in_hz)
    }

    /// Create packet parameters for a send operation on a communication channel
    pub fn create_tx_packet_params(
        &mut self,
        preamble_length: u16,
        implicit_header: bool,
        crc_on: bool,
        iq_inverted: bool,
        modulation_params: &ModulationParams,
    ) -> Result<PacketParams, RadioError> {
        self.radio_kind.create_packet_params(
            preamble_length,
            implicit_header,
            0,
            crc_on,
            iq_inverted,
            modulation_params,
        )
    }

    /// Create packet parameters for a receive operation on a communication channel
    pub fn create_rx_packet_params(
        &mut self,
        preamble_length: u16,
        implicit_header: bool,
        max_payload_length: u8,
        crc_on: bool,
        iq_inverted: bool,
        modulation_params: &ModulationParams,
    ) -> Result<PacketParams, RadioError> {
        self.radio_kind.create_packet_params(
            preamble_length,
            implicit_header,
            max_payload_length,
            crc_on,
            iq_inverted,
            modulation_params,
        )
    }

    /// Initialize a Semtech chip as the radio for LoRa physical layer communications
    pub async fn init(&mut self) -> Result<(), RadioError> {
        self.cold_start = true;
        self.radio_kind.reset(&mut self.delay).await?;
        self.radio_kind.ensure_ready(self.radio_mode).await?;
        self.radio_kind.set_standby().await?;
        self.radio_mode = RadioMode::Standby;
        self.do_cold_start().await
    }

    async fn do_cold_start(&mut self) -> Result<(), RadioError> {
        self.radio_kind.init_lora(self.enable_public_network).await?;
        self.radio_kind.set_tx_power_and_ramp_time(0, None, false).await?;
        self.radio_kind.set_irq_params(Some(self.radio_mode)).await?;
        self.cold_start = false;
        self.calibrate_image = true;
        Ok(())
    }

    /// Place the LoRa physical layer in standby mode
    pub async fn enter_standby(&mut self) -> Result<(), RadioError> {
        self.radio_kind.set_standby().await
    }

    /// Place the LoRa physical layer in low power mode, specifying cold or warm start (if the Semtech chip supports it)
    pub async fn sleep(&mut self, warm_start_if_possible: bool) -> Result<(), RadioError> {
        if self.radio_mode != RadioMode::Sleep {
            self.radio_kind.ensure_ready(self.radio_mode).await?;
            self.radio_kind
                .set_sleep(warm_start_if_possible, &mut self.delay)
                .await?;
            if !warm_start_if_possible {
                self.cold_start = true;
            }
            self.radio_mode = RadioMode::Sleep;
        }
        Ok(())
    }

    /// Prepare the Semtech chip for a send operation
    pub async fn prepare_for_tx(
        &mut self,
        mdltn_params: &ModulationParams,
        tx_pkt_params: &mut PacketParams,
        output_power: i32,
        buffer: &[u8],
    ) -> Result<(), RadioError> {
        self.prepare_modem(mdltn_params).await?;

        self.radio_kind.set_modulation_params(mdltn_params).await?;
        self.radio_kind
            .set_tx_power_and_ramp_time(output_power, Some(mdltn_params), true)
            .await?;
        self.radio_kind.ensure_ready(self.radio_mode).await?;
        if self.radio_mode != RadioMode::Standby {
            self.radio_kind.set_standby().await?;
            self.radio_mode = RadioMode::Standby;
        }

        tx_pkt_params.set_payload_length(buffer.len())?;
        self.radio_kind.set_packet_params(tx_pkt_params).await?;
        self.radio_kind.set_channel(mdltn_params.frequency_in_hz).await?;
        self.radio_kind.set_payload(buffer).await?;
        self.radio_mode = RadioMode::Transmit;
        self.radio_kind.set_irq_params(Some(self.radio_mode)).await?;
        Ok(())
    }

    /// Execute a send operation
    pub async fn tx(&mut self) -> Result<(), RadioError> {
        if let RadioMode::Transmit = self.radio_mode {
            self.radio_kind.do_tx().await?;
            loop {
                self.wait_for_irq().await?;
                match self.radio_kind.process_irq_event(self.radio_mode, None, true).await {
                    Ok(Some(IrqState::Done | IrqState::PreambleReceived)) => {
                        self.radio_mode = RadioMode::Standby;
                        return Ok(());
                    }
                    Ok(None) => continue,
                    Err(err) => {
                        self.radio_kind.ensure_ready(self.radio_mode).await?;
                        self.radio_kind.set_standby().await?;
                        self.radio_mode = RadioMode::Standby;
                        return Err(err);
                    }
                }
            }
        } else {
            Err(RadioError::InvalidRadioMode)
        }
    }

    /// Prepare radio to receive a frame in either single or continuous packet mode.
    /// Notes:
    /// * sx126x SetRx(0 < timeout < MAX) will listen util LoRa packet header is detected,
    /// therefore we only use 0 (Single Mode) and MAX (continuous) values.
    /// TODO: Find a way to express timeout for sx126x, allowing waiting for packet upto 262s
    /// TODO: Allow DutyCycle as well?
    pub async fn prepare_for_rx(
        &mut self,
        listen_mode: RxMode,
        mdltn_params: &ModulationParams,
        rx_pkt_params: &PacketParams,
    ) -> Result<(), RadioError> {
        defmt::trace!("RX mode: {}", listen_mode);
        self.prepare_modem(mdltn_params).await?;

        self.radio_kind.set_modulation_params(mdltn_params).await?;
        self.radio_kind.set_packet_params(rx_pkt_params).await?;
        self.radio_kind.set_channel(mdltn_params.frequency_in_hz).await?;
        self.radio_mode = listen_mode.into();
        self.radio_kind.set_irq_params(Some(self.radio_mode)).await?;
        Ok(())
    }

    /// Start receiving and wait for result
    pub async fn rx(
        &mut self,
        packet_params: &PacketParams,
        receiving_buffer: &mut [u8],
    ) -> Result<(u8, PacketStatus), RadioError> {
        if let RadioMode::Receive(listen_mode) = self.radio_mode {
            self.radio_kind.do_rx(listen_mode).await?;
            loop {
                self.wait_for_irq().await?;
                match self.radio_kind.process_irq_event(self.radio_mode, None, true).await {
                    Ok(Some(actual_state)) => match actual_state {
                        IrqState::PreambleReceived => continue,
                        IrqState::Done => {
                            let received_len = self.radio_kind.get_rx_payload(packet_params, receiving_buffer).await?;
                            let rx_pkt_status = self.radio_kind.get_rx_packet_status().await?;
                            return Ok((received_len, rx_pkt_status));
                        }
                    },
                    Ok(None) => continue,
                    Err(err) => {
                        // if in rx continuous mode, allow the caller to determine whether to keep receiving
                        if self.radio_mode != RadioMode::Receive(RxMode::Continuous) {
                            self.radio_kind.ensure_ready(self.radio_mode).await?;
                            self.radio_kind.set_standby().await?;
                            self.radio_mode = RadioMode::Standby;
                        }
                        return Err(err);
                    }
                }
            }
        } else {
            Err(RadioError::InvalidRadioMode)
        }
    }

    /// Prepare the Semtech chip for a channel activity detection operation
    pub async fn prepare_for_cad(&mut self, mdltn_params: &ModulationParams) -> Result<(), RadioError> {
        self.prepare_modem(mdltn_params).await?;

        self.radio_kind.set_modulation_params(mdltn_params).await?;
        self.radio_kind.set_channel(mdltn_params.frequency_in_hz).await?;
        self.radio_mode = RadioMode::ChannelActivityDetection;
        self.radio_kind.set_irq_params(Some(self.radio_mode)).await?;
        Ok(())
    }

    /// Start channel activity detection operation and return the result
    pub async fn cad(&mut self, mdltn_params: &ModulationParams) -> Result<bool, RadioError> {
        if self.radio_mode == RadioMode::ChannelActivityDetection {
            self.radio_kind.do_cad(mdltn_params).await?;
            self.wait_for_irq().await?;
            let mut cad_activity_detected = false;
            match self
                .radio_kind
                .process_irq_event(self.radio_mode, Some(&mut cad_activity_detected), true)
                .await
            {
                Ok(Some(IrqState::Done)) => Ok(cad_activity_detected),
                Err(err) => {
                    self.radio_kind.ensure_ready(self.radio_mode).await?;
                    self.radio_kind.set_standby().await?;
                    self.radio_mode = RadioMode::Standby;
                    Err(err)
                }
                Ok(_) => unreachable!(),
            }
        } else {
            Err(RadioError::InvalidRadioMode)
        }
    }

    /// Place radio in continuous wave mode, generally for regulatory testing
    ///
    /// SemTech app note AN1200.26 “Semtech LoRa FCC 15.247 Guidance” covers usage.
    ///
    /// Presumes that init() is called before this function
    pub async fn continuous_wave(
        &mut self,
        mdltn_params: &ModulationParams,
        output_power: i32,
    ) -> Result<(), RadioError> {
        self.prepare_modem(mdltn_params).await?;

        let tx_pkt_params = self
            .radio_kind
            .create_packet_params(0, false, 16, false, false, mdltn_params)?;
        self.radio_kind.set_packet_params(&tx_pkt_params).await?;
        self.radio_kind.set_modulation_params(mdltn_params).await?;
        self.radio_kind
            .set_tx_power_and_ramp_time(output_power, Some(mdltn_params), true)
            .await?;

        self.radio_kind.ensure_ready(self.radio_mode).await?;
        if self.radio_mode != RadioMode::Standby {
            self.radio_kind.set_standby().await?;
            self.radio_mode = RadioMode::Standby;
        }
        self.radio_kind.set_channel(mdltn_params.frequency_in_hz).await?;
        self.radio_mode = RadioMode::Transmit;
        self.radio_kind.set_irq_params(Some(self.radio_mode)).await?;
        self.radio_kind.set_tx_continuous_wave_mode().await
    }

    async fn prepare_modem(&mut self, mdltn_params: &ModulationParams) -> Result<(), RadioError> {
        self.radio_kind.ensure_ready(self.radio_mode).await?;
        if self.radio_mode != RadioMode::Standby {
            self.radio_kind.set_standby().await?;
            self.radio_mode = RadioMode::Standby;
        }

        if self.cold_start {
            self.do_cold_start().await?;
        }

        if self.calibrate_image {
            self.radio_kind.calibrate_image(mdltn_params.frequency_in_hz).await?;
            self.calibrate_image = false;
        }

        Ok(())
    }
}
