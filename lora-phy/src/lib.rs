#![no_std]
#![feature(async_fn_in_trait)]
#![allow(incomplete_features)]
#![warn(missing_docs)]
#![doc = include_str!("../README.md")]

/// The read/write interface between an embedded framework/MCU combination and a LoRa chip
pub(crate) mod interface;
/// Parameters used across the lora-phy crate to support various use cases
pub mod mod_params;
/// Traits implemented externally or internally to support control of LoRa chips
pub mod mod_traits;
/// Specific implementation to support Semtech Sx126x chips
pub mod sx1261_2;
/// Specific implementation to support Semtech Sx127x chips
pub mod sx1276_7_8_9;

pub use embedded_hal_async::delay::DelayNs;
use interface::*;
use mod_params::*;
use mod_traits::*;

// Maximum value for symbol timeout across known LoRa chips
const MAX_LORA_SYMB_NUM_TIMEOUT: u32 = 248;

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
    rx_continuous: bool,
    polling_timeout_in_ms: Option<u32>,
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
            rx_continuous: false,
            polling_timeout_in_ms: None,
            cold_start: true,
            calibrate_image: true,
        };
        lora.init().await?;

        Ok(lora)
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
        self.rx_continuous = false;
        self.do_cold_start().await
    }

    async fn do_cold_start(&mut self) -> Result<(), RadioError> {
        self.radio_kind.init_rf_switch().await?;
        self.radio_kind.set_lora_modem(self.enable_public_network).await?;
        self.radio_kind.set_oscillator().await?;
        self.radio_kind.set_regulator_mode().await?;
        self.radio_kind.set_tx_rx_buffer_base_address(0, 0).await?;
        self.radio_kind
            .set_tx_power_and_ramp_time(0, None, false, false)
            .await?;
        self.radio_kind.set_irq_params(Some(self.radio_mode)).await?;
        self.radio_kind.update_retention_list().await?;
        self.cold_start = false;
        self.calibrate_image = true;
        Ok(())
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
        output_power: i32,
        tx_boosted_if_possible: bool,
    ) -> Result<(), RadioError> {
        self.rx_continuous = false;

        self.prepare_modem(mdltn_params).await?;

        self.radio_kind.set_modulation_params(mdltn_params).await?;
        self.radio_kind
            .set_tx_power_and_ramp_time(output_power, Some(mdltn_params), tx_boosted_if_possible, true)
            .await
    }

    /// Execute a send operation
    pub async fn tx(
        &mut self,
        mdltn_params: &ModulationParams,
        tx_pkt_params: &mut PacketParams,
        buffer: &[u8],
        timeout_in_ms: u32,
    ) -> Result<(), RadioError> {
        self.rx_continuous = false;
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
        self.radio_kind.do_tx(timeout_in_ms).await?;
        match self
            .radio_kind
            .process_irq(
                self.radio_mode,
                self.rx_continuous,
                TargetIrqState::Done,
                &mut self.delay,
                None,
                None,
            )
            .await
        {
            Ok(TargetIrqState::Done) => {
                self.radio_mode = RadioMode::Standby;
                Ok(())
            }
            Err(err) => {
                self.radio_kind.ensure_ready(self.radio_mode).await?;
                self.radio_kind.set_standby().await?;
                self.radio_mode = RadioMode::Standby;
                Err(err)
            }
            Ok(_) => unreachable!(),
        }
    }

    /// Prepare the Semtech chip for a receive operation (single shot, continuous, or duty cycled) and initiate the operation
    #[allow(clippy::too_many_arguments)]
    pub async fn prepare_for_rx(
        &mut self,
        mdltn_params: &ModulationParams,
        rx_pkt_params: &PacketParams,
        window_in_secs: Option<u8>, // None for Rx continuous
        duty_cycle_params: Option<&DutyCycleParams>,
        rx_boosted_if_supported: bool,
    ) -> Result<(), RadioError> {
        let mut symbol_timeout: u32 = 0;
        match window_in_secs {
            Some(window) => {
                let sf = u32::from(mdltn_params.spreading_factor);
                let bw = u32::from(mdltn_params.bandwidth);
                let symbols_per_sec = bw / (0x01u32 << sf); // symbol rate in symbols/sec = (BW in Hz) / (2 raised to the SF power)
                let window_in_ms: u32 = (window as u32).checked_mul(1000).unwrap();
                symbol_timeout = (window_in_ms - 200).checked_mul(symbols_per_sec).unwrap() / 1000; // leave a gap (subtract 200ms) to allow time to set up another window
                if symbol_timeout > MAX_LORA_SYMB_NUM_TIMEOUT {
                    symbol_timeout = MAX_LORA_SYMB_NUM_TIMEOUT;
                }
                self.rx_continuous = false;
                // provide a safety net polling timeout while allowing reception of a packet which starts within the window but exceeds the window size
                self.polling_timeout_in_ms = Some(window_in_ms.checked_mul(5).unwrap());
            }
            None => {
                self.rx_continuous = true;
                self.polling_timeout_in_ms = None;
            }
        }

        self.prepare_modem(mdltn_params).await?;

        self.radio_kind.set_modulation_params(mdltn_params).await?;
        self.radio_kind.set_packet_params(rx_pkt_params).await?;
        self.radio_kind.set_channel(mdltn_params.frequency_in_hz).await?;
        self.radio_mode = match duty_cycle_params {
            Some(&_duty_cycle) => RadioMode::ReceiveDutyCycle,
            None => RadioMode::Receive,
        };
        self.radio_kind.set_irq_params(Some(self.radio_mode)).await?;
        self.radio_kind
            .do_rx(
                rx_pkt_params,
                duty_cycle_params,
                self.rx_continuous,
                rx_boosted_if_supported,
                symbol_timeout as u16,
            )
            .await
    }

    /// Obtain the results of a read operation
    pub async fn rx(
        &mut self,
        rx_pkt_params: &PacketParams,
        receiving_buffer: &mut [u8],
    ) -> Result<(u8, PacketStatus), RadioError> {
        let IrqState::RxDone(len, status) = self.rx_until_state(rx_pkt_params, receiving_buffer, TargetIrqState::Done).await? else {
            unreachable!();
        };
        Ok((len, status))
    }

    /// Obtain the results of a read operation
    pub async fn rx_until_state(
        &mut self,
        rx_pkt_params: &PacketParams,
        receiving_buffer: &mut [u8],
        target_rx_state: TargetIrqState,
    ) -> Result<IrqState, RadioError> {
        match self
            .radio_kind
            .process_irq(
                self.radio_mode,
                self.rx_continuous,
                target_rx_state,
                &mut self.delay,
                self.polling_timeout_in_ms,
                None,
            )
            .await
        {
            Ok(actual_state) => match actual_state {
                TargetIrqState::PreambleReceived => Ok(IrqState::PreambleReceived),
                TargetIrqState::Done => {
                    let received_len = self.radio_kind.get_rx_payload(rx_pkt_params, receiving_buffer).await?;
                    let rx_pkt_status = self.radio_kind.get_rx_packet_status().await?;
                    Ok(IrqState::RxDone(received_len, rx_pkt_status))
                }
            },
            Err(err) => {
                // if in rx continuous mode, allow the caller to determine whether to keep receiving
                if !self.rx_continuous {
                    self.radio_kind.ensure_ready(self.radio_mode).await?;
                    self.radio_kind.set_standby().await?;
                    self.radio_mode = RadioMode::Standby;
                }
                Err(err)
            }
        }
    }

    /// Prepare the Semtech chip for a channel activity detection operation and initiate the operation
    pub async fn prepare_for_cad(
        &mut self,
        mdltn_params: &ModulationParams,
        rx_boosted_if_supported: bool,
    ) -> Result<(), RadioError> {
        self.rx_continuous = false;

        self.prepare_modem(mdltn_params).await?;

        self.radio_kind.set_modulation_params(mdltn_params).await?;
        self.radio_kind.set_channel(mdltn_params.frequency_in_hz).await?;
        self.radio_mode = RadioMode::ChannelActivityDetection;
        self.radio_kind.set_irq_params(Some(self.radio_mode)).await?;
        self.radio_kind.do_cad(mdltn_params, rx_boosted_if_supported).await
    }

    /// Obtain the results of a channel activity detection operation
    pub async fn cad(&mut self) -> Result<bool, RadioError> {
        let mut cad_activity_detected = false;
        match self
            .radio_kind
            .process_irq(
                self.radio_mode,
                self.rx_continuous,
                TargetIrqState::Done,
                &mut self.delay,
                None,
                Some(&mut cad_activity_detected),
            )
            .await
        {
            Ok(TargetIrqState::Done) => Ok(cad_activity_detected),
            Err(err) => {
                self.radio_kind.ensure_ready(self.radio_mode).await?;
                self.radio_kind.set_standby().await?;
                self.radio_mode = RadioMode::Standby;
                Err(err)
            }
            Ok(_) => unreachable!(),
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
        tx_boosted_if_possible: bool,
    ) -> Result<(), RadioError> {
        self.rx_continuous = false;

        self.prepare_modem(mdltn_params).await?;

        let tx_pkt_params = self
            .radio_kind
            .create_packet_params(0, false, 16, false, false, mdltn_params)?;
        self.radio_kind.set_packet_params(&tx_pkt_params).await?;
        self.radio_kind.set_modulation_params(mdltn_params).await?;
        self.radio_kind
            .set_tx_power_and_ramp_time(output_power, Some(mdltn_params), tx_boosted_if_possible, true)
            .await?;

        self.rx_continuous = false;
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

impl<RK, DLY> AsyncRng for LoRa<RK, DLY>
where
    RK: RngRadio,
    DLY: DelayNs,
{
    async fn get_random_number(&mut self) -> Result<u32, RadioError> {
        self.rx_continuous = false;
        self.radio_kind.ensure_ready(self.radio_mode).await?;
        if self.radio_mode != RadioMode::Standby {
            self.radio_kind.set_standby().await?;
            self.radio_mode = RadioMode::Standby;
        }
        if self.cold_start {
            self.do_cold_start().await?;
        }

        let random_number = self.radio_kind.get_random_number().await?;

        self.radio_kind.set_standby().await?;
        self.radio_mode = RadioMode::Standby;

        Ok(random_number)
    }
}
