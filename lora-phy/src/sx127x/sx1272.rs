use crate::mod_params::{ModulationParams, PacketParams, RadioError};
use crate::mod_traits::InterfaceVariant;
use crate::sx127x::radio_kind_params::{coding_rate_value, spreading_factor_value, RampTime, Register, Sx127xVariant};
use crate::sx127x::{Sx127x, SX1272_RSSI_OFFSET};
use embedded_hal_async::spi::SpiDevice;
use lora_modulation::Bandwidth;

/// Sx1272 implements the Sx127xVariant trait
pub struct Sx1272;

impl Sx127xVariant for Sx1272 {
    fn bandwidth_value(bw: Bandwidth) -> Result<u8, RadioError> {
        match bw {
            Bandwidth::_125KHz => Ok(0x00),
            Bandwidth::_250KHz => Ok(0x01),
            Bandwidth::_500KHz => Ok(0x02),
            _ => Err(RadioError::UnavailableBandwidth),
        }
    }

    fn reg_txco() -> Register {
        Register::RegTcxoSX1272
    }

    async fn set_tx_power<SPI: SpiDevice<u8>, IV: InterfaceVariant>(
        radio: &mut Sx127x<SPI, IV, Self>,
        p_out: i32,
        tx_boost: bool,
    ) -> Result<(), RadioError> {
        // SX1272 has two output pins:
        // 1) RFO: (-1 to +14 dBm)
        // 2) PA_BOOST: (+2 to +17 dBm and +5 to 20 +dBm)

        // RegPaConfig - 0x32
        // [7] - PaSelect (0: RFO, 1: PA_BOOST)
        // [6:4] - Unused: 0
        // [3:0] - Output power in dB steps

        // RegPaDac - 0x5a (SX1272)
        // [7:3] - Reserved (0x10 as default)
        // [2:0] - PaDac: 0x04 default, 0x07 - enable +20 dBm on PA_BOOST

        // TODO: Shall we also touch OCP settings?
        if tx_boost {
            // Deal with two ranges, +17dBm enables extra boost
            if p_out > 17 {
                // PA_BOOST out: +5 .. +20 dBm
                let val = (p_out.clamp(5, 20) - 5) as u8 & 0x0f;
                radio.write_register(Register::RegPaConfig, (1 << 7) | val).await?;
                radio.write_register(Register::RegPaDacSX1272, 0x87).await?;
            } else {
                // PA_BOOST out: +2 .. +17 dBm
                let val = (p_out.clamp(2, 17) - 2) as u8 & 0x0f;
                radio.write_register(Register::RegPaConfig, (1 << 7) | val).await?;
                radio.write_register(Register::RegPaDacSX1272, 0x84).await?;
            }
        } else {
            // RFO out: -1 to +14 dBm
            let val = (p_out.clamp(-1, 14) + 1) as u8 & 0x0f;
            radio.write_register(Register::RegPaConfig, val).await?;
            radio.write_register(Register::RegPaDacSX1272, 0x84).await?;
        }

        Ok(())
    }

    fn ramp_value(ramp_time: RampTime) -> u8 {
        // Sx1272 - default: 0x19
        // [4]: LowPnTxPllOff - use higher power, lower phase noise PLL
        //      only when the transmitter is used (default: 1)
        //      0 - Standard PLL used in Rx mode, Lower PN PLL in Tx
        //      1 - Standard PLL used in both Tx and Rx modes
        ramp_time.value() | (1 << 4)
    }

    async fn set_modulation_params<SPI: SpiDevice<u8>, IV: InterfaceVariant>(
        radio: &mut Sx127x<SPI, IV, Self>,
        mdltn_params: &ModulationParams,
    ) -> Result<(), RadioError> {
        let bw_val = Self::bandwidth_value(mdltn_params.bandwidth)?;
        let sf_val = spreading_factor_value(mdltn_params.spreading_factor)?;

        let cfg1 = radio.read_register(Register::RegModemConfig1).await?;
        let ldro = mdltn_params.low_data_rate_optimize;
        let cr_val = coding_rate_value(mdltn_params.coding_rate)?;
        let val = (cfg1 & 0b110) | (bw_val << 6) | (cr_val << 3) | ldro;
        radio.write_register(Register::RegModemConfig1, val).await?;
        let cfg2 = radio.read_register(Register::RegModemConfig2).await?;
        let val = (cfg2 & 0b1111) | (sf_val << 4);
        radio.write_register(Register::RegModemConfig2, val).await?;
        Ok(())
    }

    async fn set_packet_params<SPI: SpiDevice<u8>, IV: InterfaceVariant>(
        radio: &mut Sx127x<SPI, IV, Self>,
        pkt_params: &PacketParams,
    ) -> Result<(), RadioError>
    where
        Self: Sized,
    {
        let modemcfg1 = radio.read_register(Register::RegModemConfig1).await?;

        let hdr = pkt_params.implicit_header as u8;
        let crc = pkt_params.crc_on as u8;

        let cfg1 = (modemcfg1 & 0b1111_1001) | (hdr << 2) | (crc << 1);
        radio.write_register(Register::RegModemConfig1, cfg1).await?;
        Ok(())
    }

    async fn rssi_offset<SPI: SpiDevice<u8>, IV: InterfaceVariant>(
        _: &mut Sx127x<SPI, IV, Self>,
    ) -> Result<i16, RadioError> {
        Ok(SX1272_RSSI_OFFSET)
    }

    async fn set_tx_continuous_wave_mode<SPI: SpiDevice<u8>, IV: InterfaceVariant>(
        _: &mut Sx127x<SPI, IV, Self>,
    ) -> Result<(), RadioError> {
        todo!()
    }
}
