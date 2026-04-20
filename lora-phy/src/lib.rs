#![no_std]
#![deny(rust_2018_idioms)]
#![warn(missing_docs)]
#![cfg_attr(docsrs, feature(doc_cfg))]

//! ## Feature flags
#![doc = document_features::document_features!(feature_label = r#"<span class="stab portability"><code>{feature}</code></span>"#)]
#![doc = include_str!("../README.md")]

// This must go FIRST so that all the other modules see its macros.
pub(crate) mod fmt;

#[cfg(feature = "lorawan-radio")]
#[cfg_attr(docsrs, doc(cfg(feature = "lorawan-radio")))]
/// Provides an implementation of the async LoRaWAN device trait.
pub mod lorawan_radio;

/// The read/write interface between an embedded framework/MCU combination and a LoRa chip
pub(crate) mod interface;
/// InterfaceVariant implementations using `embedded-hal`.
pub mod iv;
/// Specific implementation to support Semtech LR1110/LR1120/LR1121 chips
pub mod lr1110;
/// LR11xx-specific SPI interface (different protocol than SX126x/SX127x)
pub(crate) mod lr1110_interface;
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

/// Sync word for public LoRaWAN networks
const LORAWAN_PUBLIC_SYNCWORD: u8 = 0x34;

/// Sync word for private LoRaWAN networks
const LORAWAN_PRIVATE_SYNCWORD: u8 = 0x12;

/// Provides the physical layer API to support LoRa chips
pub struct LoRa<RK, DLY>
where
    RK: RadioKind,
    DLY: DelayNs,
{
    radio_kind: RK,
    delay: DLY,
    radio_mode: RadioMode,
    sync_word: u8,
    cold_start: bool,
    calibrate_image: bool,
}

impl<RK, DLY> LoRa<RK, DLY>
where
    RK: RadioKind,
    DLY: DelayNs,
{
    /// Build and return a new instance of the LoRa physical layer API with a specified sync word
    pub async fn with_syncword(radio_kind: RK, sync_word: u8, delay: DLY) -> Result<Self, RadioError> {
        let mut lora = Self {
            radio_kind,
            delay,
            radio_mode: RadioMode::Sleep,
            sync_word,
            cold_start: true,
            calibrate_image: true,
        };
        lora.init().await?;

        Ok(lora)
    }

    /// Build and return a new instance of the LoRa physical layer API to
    /// control an initialized LoRa radio for LoRaWAN public or private network.
    ///
    /// In order to configure radio to use non-LoRaWAN networks, use
    /// [`LoRa::with_syncword()`] which has `sync_word` argument.
    pub async fn new(radio_kind: RK, enable_public_network: bool, delay: DLY) -> Result<Self, RadioError> {
        let sync_word = if enable_public_network {
            LORAWAN_PUBLIC_SYNCWORD
        } else {
            LORAWAN_PRIVATE_SYNCWORD
        };
        Self::with_syncword(radio_kind, sync_word, delay).await
    }

    /// Wait for an IRQ event to occur
    pub async fn wait_for_irq(&mut self) -> Result<(), RadioError> {
        self.radio_kind.await_irq().await
    }

    /// Process an IRQ event and return the new state of the radio
    ///
    /// # Warning
    /// This function is not safe to drop or cancel, as it calls `process_irq_event`, which must run to completion to avoid radio lockups.
    /// Do not call this function within a select branch or in any context where it may be prematurely canceled.
    pub async fn process_irq_event(&mut self) -> Result<Option<IrqState>, RadioError> {
        self.radio_kind.process_irq_event(self.radio_mode, false).await
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

    /// Create packet parameters for a transmit operation on a communication channel
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

    /// Initialize the radio for LoRa physical layer communications
    pub async fn init(&mut self) -> Result<(), RadioError> {
        self.cold_start = true;
        self.radio_kind.reset(&mut self.delay).await?;
        self.radio_kind.ensure_ready(self.radio_mode).await?;
        self.radio_kind.set_standby().await?;
        self.radio_mode = RadioMode::Standby;
        self.do_cold_start().await
    }

    async fn do_cold_start(&mut self) -> Result<(), RadioError> {
        self.radio_kind.init_lora(self.sync_word).await?;
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

    /// Place the LoRa physical layer in low power mode, specifying cold or
    /// warm start (if chip supports it)
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

    /// Prepare the radio for a transmit operation
    pub async fn prepare_for_tx(
        &mut self,
        mdltn_params: &ModulationParams,
        tx_pkt_params: &mut PacketParams,
        output_power: i32,
        buffer: &[u8],
    ) -> Result<(), RadioError> {
        self.prepare_modem(mdltn_params.frequency_in_hz).await?;

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

    /// Execute a transmit operation
    ///
    /// # Warning
    /// This function is not safe to drop or cancel, as it calls `process_irq_event`, which must run to completion to avoid radio lockups.
    /// Do not call this function within a select branch or in any context where it may be prematurely canceled.
    pub async fn tx(&mut self) -> Result<(), RadioError> {
        if let RadioMode::Transmit = self.radio_mode {
            self.radio_kind.do_tx().await?;
            loop {
                self.wait_for_irq().await?;
                match self.radio_kind.process_irq_event(self.radio_mode, true).await {
                    // In Tx mode we do not have "special" events
                    Ok(Some(IrqState::Done | IrqState::Detect)) => {
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

    /// Configure radio for a receive operation
    pub async fn prepare_for_rx(
        &mut self,
        listen_mode: RxMode,
        mdltn_params: &ModulationParams,
        rx_pkt_params: &PacketParams,
    ) -> Result<(), RadioError> {
        trace!("RX mode: {}", listen_mode);
        self.prepare_modem(mdltn_params.frequency_in_hz).await?;

        self.radio_kind.set_modulation_params(mdltn_params).await?;
        self.radio_kind.set_packet_params(rx_pkt_params).await?;
        self.radio_kind.set_channel(mdltn_params.frequency_in_hz).await?;
        self.radio_mode = listen_mode.into();
        self.radio_kind.set_irq_params(Some(self.radio_mode)).await?;
        Ok(())
    }

    /// Switch radio to receive mode (prepared via [`LoRa::prepare_for_rx`]).
    /// Call [`LoRa::complete_rx`] to wait and handle result.
    pub async fn start_rx(&mut self) -> Result<(), RadioError> {
        if let RadioMode::Receive(listen_mode) = self.radio_mode {
            self.radio_kind.do_rx(listen_mode).await
        } else {
            Err(RadioError::InvalidRadioMode)
        }
    }

    /// Wait for a previously started receive to complete
    ///
    /// # Warning
    /// This function is not safe to drop or cancel, as it calls `process_irq_event`, which must run to completion to avoid radio lockups.
    /// Do not call this function within a select branch or in any context where it may be prematurely canceled.
    pub async fn complete_rx(
        &mut self,
        packet_params: &PacketParams,
        receiving_buffer: &mut [u8],
    ) -> Result<(u8, PacketStatus), RadioError> {
        if let RadioMode::Receive(_) = self.radio_mode {
            loop {
                match self.radio_kind.process_irq_event(self.radio_mode, true).await {
                    Ok(Some(actual_state)) => match actual_state {
                        IrqState::Done => {
                            let received_len = self.radio_kind.get_rx_payload(packet_params, receiving_buffer).await?;
                            let rx_pkt_status = self.radio_kind.get_rx_packet_status().await?;
                            return Ok((received_len, rx_pkt_status));
                        }
                        // Preamble was received, wait for next IRQ
                        IrqState::Detect => (),
                    },
                    Ok(None) => (),
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
                self.wait_for_irq().await?;
            }
        } else {
            Err(RadioError::InvalidRadioMode)
        }
    }

    /// Returns the current IRQ state
    pub async fn get_irq_state(&mut self) -> Result<Option<IrqState>, RadioError> {
        self.radio_kind.get_irq_state(self.radio_mode).await
    }

    /// Clears the IRQ status
    pub async fn clear_irq_status(&mut self) -> Result<(), RadioError> {
        self.radio_kind.clear_irq_status().await
    }

    /// Extracts the received payload and packet status after a completed RX IRQ event.
    /// Should be called after receiving `IrqState::Done`.
    pub async fn get_rx_result(
        &mut self,
        packet_params: &PacketParams,
        receiving_buffer: &mut [u8],
    ) -> Result<(u8, PacketStatus), RadioError> {
        if let RadioMode::Receive(_) = self.radio_mode {
            let received_len = self.radio_kind.get_rx_payload(packet_params, receiving_buffer).await?;
            let rx_pkt_status = self.radio_kind.get_rx_packet_status().await?;
            Ok((received_len, rx_pkt_status))
        } else {
            Err(RadioError::InvalidRadioMode)
        }
    }

    /// Start reception and wait for its completion by calling
    /// [`LoRa::start_rx`]  and [`LoRa::complete_rx`] in succession.
    pub async fn rx(
        &mut self,
        packet_params: &PacketParams,
        receiving_buffer: &mut [u8],
    ) -> Result<(u8, PacketStatus), RadioError> {
        self.start_rx().await?;
        self.complete_rx(packet_params, receiving_buffer).await
    }

    /// Start listening to a given frequency and [`Bandwidth`]
    pub async fn listen(&mut self, frequency_in_hz: u32, bandwidth: Bandwidth) -> Result<(), RadioError> {
        self.prepare_modem(frequency_in_hz).await?;

        self.radio_kind.set_channel(frequency_in_hz).await?;
        // We need to set the bandwidth, otherwise sx126x doesn't return
        // reasonable RSSI results. All other params are irrelevant with
        // regard to listening to measure RSSI.
        let modulation_params = self.radio_kind.create_modulation_params(
            SpreadingFactor::_7,
            bandwidth,
            CodingRate::_4_5,
            frequency_in_hz,
        )?;
        self.radio_kind.set_modulation_params(&modulation_params).await?;
        self.radio_mode = RadioMode::Listen;
        self.radio_kind.do_rx(RxMode::Continuous).await?;

        Ok(())
    }

    /// Get the current RSSI
    pub async fn get_rssi(&mut self) -> Result<i16, RadioError> {
        self.radio_kind.get_rssi().await
    }

    /// Prepare the radio for a channel activity detection (CAD) operation
    pub async fn prepare_for_cad(&mut self, mdltn_params: &ModulationParams) -> Result<(), RadioError> {
        self.prepare_modem(mdltn_params.frequency_in_hz).await?;

        self.radio_kind.set_modulation_params(mdltn_params).await?;
        self.radio_kind.set_channel(mdltn_params.frequency_in_hz).await?;
        self.radio_mode = RadioMode::ChannelActivityDetection;
        self.radio_kind.set_irq_params(Some(self.radio_mode)).await?;
        Ok(())
    }

    /// Start channel activity detection (CAD) operation and return the result
    ///
    /// # Warning
    /// This function is not safe to drop or cancel, as it calls `process_irq_event`, which must run to completion to avoid radio lockups.
    /// Do not call this function within a select branch or in any context where it may be prematurely canceled.
    pub async fn cad(&mut self, mdltn_params: &ModulationParams) -> Result<bool, RadioError> {
        if self.radio_mode == RadioMode::ChannelActivityDetection {
            self.radio_kind.do_cad(mdltn_params).await?;
            self.wait_for_irq().await?;
            match self.radio_kind.process_irq_event(self.radio_mode, true).await {
                Ok(Some(IrqState::Detect)) => Ok(true),
                Ok(Some(IrqState::Done)) => Ok(false),
                Ok(_) => {
                    if cfg!(feature = "stm32-subghz-irq-quirk") {
                        // STM32 subghz device triggers IRQ, but none of
                        // expected IRQ fields are enabled.
                        // Radio status flag in this case is 0xd2, signalling
                        // that radio is still in RX mode.
                        Ok(false)
                    } else {
                        unreachable!("Spurious IRQ detected!")
                    }
                }
                Err(err) => {
                    self.radio_kind.ensure_ready(self.radio_mode).await?;
                    self.radio_kind.set_standby().await?;
                    self.radio_mode = RadioMode::Standby;
                    Err(err)
                }
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
        self.prepare_modem(mdltn_params.frequency_in_hz).await?;

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

    async fn prepare_modem(&mut self, frequency_in_hz: u32) -> Result<(), RadioError> {
        self.radio_kind.ensure_ready(self.radio_mode).await?;
        if self.radio_mode != RadioMode::Standby {
            self.radio_kind.set_standby().await?;
            self.radio_mode = RadioMode::Standby;
        }

        if self.cold_start {
            self.do_cold_start().await?;
        }

        if self.calibrate_image {
            self.radio_kind.calibrate_image(frequency_in_hz).await?;
            self.calibrate_image = false;
        }

        Ok(())
    }
}
