use crate::mod_params::{ModulationParams, PacketParams, RadioError};
use crate::mod_traits::InterfaceVariant;
use crate::sx127x::radio_kind_params::{
    coding_rate_denominator_value, spreading_factor_value, OcpTrim, PaConfig, PaDac, RampTime, Register, Sx127xVariant,
};
use crate::sx127x::{
    Sx127x, FREQUENCY_SYNTHESIZER_STEP, SX1276_RF_MID_BAND_THRESH, SX1276_RSSI_OFFSET_HF, SX1276_RSSI_OFFSET_LF,
};
use embedded_hal_async::spi::SpiDevice;
use lora_modulation::Bandwidth;

/// Sx1276 implements the Sx127xVariant trait
pub struct Sx1276;

impl Sx127xVariant for Sx1276 {
    fn bandwidth_value(bw: Bandwidth) -> Result<u8, RadioError> {
        match bw {
            Bandwidth::_7KHz => Ok(0x00),
            Bandwidth::_10KHz => Ok(0x01),
            Bandwidth::_15KHz => Ok(0x02),
            Bandwidth::_20KHz => Ok(0x03),
            Bandwidth::_31KHz => Ok(0x04),
            Bandwidth::_41KHz => Ok(0x05),
            Bandwidth::_62KHz => Ok(0x06),
            Bandwidth::_125KHz => Ok(0x07),
            Bandwidth::_250KHz => Ok(0x08),
            Bandwidth::_500KHz => Ok(0x09),
        }
    }

    fn reg_txco() -> Register {
        Register::RegTcxoSX1276
    }

    async fn set_tx_power<SPI: SpiDevice<u8>, IV: InterfaceVariant>(
        radio: &mut Sx127x<SPI, IV, Self>,
        p_out: i32,
        tx_boost: bool,
    ) -> Result<(), RadioError> {
        let pa_reg = Register::RegPaDacSX1276;
        if tx_boost {
            // Output via PA_BOOST: [2, 20] dBm
            let txp = p_out.clamp(2, 20);

            // Pout=17-(15-OutputPower)
            let output_power: i32 = txp - 2;

            if txp > 17 {
                radio.write_register(pa_reg, PaDac::_20DbmOn.value()).await?;
                radio.set_ocp(OcpTrim::_240Ma).await?;
            } else {
                radio.write_register(pa_reg, PaDac::_20DbmOff.value()).await?;
                radio.set_ocp(OcpTrim::_100Ma).await?;
            }
            radio
                .write_register(Register::RegPaConfig, PaConfig::PaBoost.value() | (output_power as u8))
                .await?;
        } else {
            // Clamp output: [-4, 14] dBm
            let txp = p_out.clamp(-4, 14);

            // Pmax=10.8+0.6*MaxPower, where MaxPower is set below as 7 and therefore Pmax is 15
            // Pout=Pmax-(15-OutputPower)
            let output_power: i32 = txp;

            radio.write_register(pa_reg, PaDac::_20DbmOff.value()).await?;
            radio.set_ocp(OcpTrim::_100Ma).await?;
            radio
                .write_register(
                    Register::RegPaConfig,
                    PaConfig::MaxPower7NoPaBoost.value() | (output_power as u8),
                )
                .await?;
        }
        Ok(())
    }

    fn ramp_value(ramp_time: RampTime) -> u8 {
        // Sx1276 - default: 0x09
        // [4]: reserved (0x00)
        ramp_time as u8
    }

    async fn set_modulation_params<SPI: SpiDevice<u8>, IV: InterfaceVariant>(
        radio: &mut Sx127x<SPI, IV, Self>,
        mdltn_params: &ModulationParams,
    ) -> Result<(), RadioError> {
        let bw_val = Self::bandwidth_value(mdltn_params.bandwidth)?;
        let sf_val = spreading_factor_value(mdltn_params.spreading_factor)?;
        let coding_rate_denominator_val = coding_rate_denominator_value(mdltn_params.coding_rate)?;

        let mut config_2 = radio.read_register(Register::RegModemConfig2).await?;
        config_2 = (config_2 & 0x0fu8) | ((sf_val << 4) & 0xf0u8);
        radio.write_register(Register::RegModemConfig2, config_2).await?;

        let mut config_1 = radio.read_register(Register::RegModemConfig1).await?;
        config_1 = (config_1 & 0x0fu8) | (bw_val << 4);
        radio.write_register(Register::RegModemConfig1, config_1).await?;

        let cr = coding_rate_denominator_val - 4;
        config_1 = radio.read_register(Register::RegModemConfig1).await?;
        config_1 = (config_1 & 0xf1u8) | (cr << 1);
        radio.write_register(Register::RegModemConfig1, config_1).await?;

        let mut ldro_agc_auto_flags = 0x00u8; // LDRO and AGC Auto both off
        if mdltn_params.low_data_rate_optimize != 0 {
            ldro_agc_auto_flags = 0x08u8; // LDRO on and AGC Auto off
        }
        let mut config_3 = radio.read_register(Register::RegModemConfig3).await?;
        config_3 = (config_3 & 0xf3u8) | ldro_agc_auto_flags;
        radio.write_register(Register::RegModemConfig3, config_3).await
    }

    async fn set_packet_params<SPI: SpiDevice<u8>, IV: InterfaceVariant>(
        radio: &mut Sx127x<SPI, IV, Self>,
        pkt_params: &PacketParams,
    ) -> Result<(), RadioError> {
        let mut config_1 = radio.read_register(Register::RegModemConfig1).await?;

        if pkt_params.implicit_header {
            config_1 |= 0x01u8;
        } else {
            config_1 &= 0xfeu8;
        }
        radio.write_register(Register::RegModemConfig1, config_1).await?;

        let mut config_2 = radio.read_register(Register::RegModemConfig2).await?;
        if pkt_params.crc_on {
            config_2 |= 0x04u8;
        } else {
            config_2 &= 0xfbu8;
        }
        radio.write_register(Register::RegModemConfig2, config_2).await?;
        Ok(())
    }

    async fn rssi_offset<SPI: SpiDevice<u8>, IV: InterfaceVariant>(
        radio: &mut Sx127x<SPI, IV, Self>,
    ) -> Result<i16, RadioError> {
        let frequency_in_hz = {
            let msb = radio.read_register(Register::RegFrfMsb).await? as u32;
            let mid = radio.read_register(Register::RegFrfMid).await? as u32;
            let lsb = radio.read_register(Register::RegFrfLsb).await? as u32;
            let frf = (msb << 16) + (mid << 8) + lsb;
            (frf as f64 * FREQUENCY_SYNTHESIZER_STEP) as u32
        };

        if frequency_in_hz > SX1276_RF_MID_BAND_THRESH {
            Ok(SX1276_RSSI_OFFSET_HF)
        } else {
            Ok(SX1276_RSSI_OFFSET_LF)
        }
    }

    async fn set_tx_continuous_wave_mode<SPI: SpiDevice<u8>, IV: InterfaceVariant>(
        radio: &mut Sx127x<SPI, IV, Self>,
    ) -> Result<(), RadioError> {
        radio.intf.iv.enable_rf_switch_rx().await?;
        let pa_config = radio.read_register(Register::RegPaConfig).await?;
        let new_pa_config = pa_config | 0b1000_0000;
        radio.write_register(Register::RegPaConfig, new_pa_config).await?;
        radio.write_register(Register::RegOpMode, 0b1100_0011).await?;
        let modem_config = radio.read_register(Register::RegModemConfig2).await?;
        let new_modem_config = modem_config | 0b0000_1000;
        radio
            .write_register(Register::RegModemConfig2, new_modem_config)
            .await?;
        Ok(())
    }
}
