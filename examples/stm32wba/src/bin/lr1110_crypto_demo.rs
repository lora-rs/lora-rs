//! LR1110 Crypto Engine Demo
//!
//! This example demonstrates the hardware cryptographic capabilities of the LR1110:
//! - AES-128 encryption and decryption
//! - AES-CMAC computation and verification
//! - Key management (set, derive, store/restore)
//! - Hardware random number generation
//!
//! The LR1110 has a built-in crypto engine that can securely store keys
//! and perform cryptographic operations without exposing key material.
//!
//! Hardware connections for STM32WBA65RI:
//! - SPI2_SCK:  PB10
//! - SPI2_MISO: PA9
//! - SPI2_MOSI: PC3
//! - SPI2_NSS:  PD14 (manual control via GPIO)
//! - LR1110_RESET: PB2
//! - LR1110_BUSY:  PB13 (BUSY signal, active high)
//! - LR1110_DIO1:  PB14 (with EXTI interrupt)

#![no_std]
#![no_main]

#[path = "../iv.rs"]
mod iv;

use defmt::*;
use embassy_executor::Spawner;
use embassy_stm32::exti::ExtiInput;
use embassy_stm32::gpio::{Level, Output, Pull, Speed};
use embassy_stm32::rcc::{
    AHB5Prescaler, AHBPrescaler, APBPrescaler, PllDiv, PllMul, PllPreDiv, PllSource, Sysclk, VoltageScale,
};
use embassy_stm32::spi::{Config as SpiConfig, Spi};
use embassy_stm32::time::Hertz;
use embassy_stm32::{bind_interrupts, Config};
use embassy_time::Delay;
use embedded_hal_bus::spi::ExclusiveDevice;
use lora_phy::lr1110::{self as lr1110_module, TcxoCtrlVoltage};
use lora_phy::lr1110::variant::Lr1110 as Lr1110Chip;
use lora_phy::mod_traits::RadioKind;
use lora_phy::lr1110::radio_kind_params::{
    CryptoElement, CryptoKeyId, CryptoStatus,
    CRYPTO_KEY_LENGTH, CRYPTO_MIC_LENGTH, CRYPTO_NONCE_LENGTH,
};
use {defmt_rtt as _, panic_probe as _};

use self::iv::Stm32wbaLr1110InterfaceVariant;

// Bind EXTI interrupts for PB13 (BUSY) and PB14 (DIO1)
bind_interrupts!(struct Irqs {
    EXTI13 => embassy_stm32::exti::InterruptHandler<embassy_stm32::interrupt::typelevel::EXTI13>;
    EXTI14 => embassy_stm32::exti::InterruptHandler<embassy_stm32::interrupt::typelevel::EXTI14>;
});

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    // Initialize STM32WBA65RI peripherals
    let mut config = Config::default();

    // Configure PLL1 for 96 MHz system clock
    config.rcc.pll1 = Some(embassy_stm32::rcc::Pll {
        source: PllSource::HSI,
        prediv: PllPreDiv::DIV1,
        mul: PllMul::MUL30,
        divr: Some(PllDiv::DIV5),
        divq: None,
        divp: Some(PllDiv::DIV30),
        frac: Some(0),
    });

    config.rcc.ahb_pre = AHBPrescaler::DIV1;
    config.rcc.apb1_pre = APBPrescaler::DIV1;
    config.rcc.apb2_pre = APBPrescaler::DIV1;
    config.rcc.apb7_pre = APBPrescaler::DIV1;
    config.rcc.ahb5_pre = AHB5Prescaler::DIV4;
    config.rcc.voltage_scale = VoltageScale::RANGE1;
    config.rcc.sys = Sysclk::PLL1_R;

    let p = embassy_stm32::init(config);

    info!("===========================================");
    info!("LR1110 Crypto Engine Demo");
    info!("===========================================");

    // Configure SPI2 for LR1110
    let mut spi_config = SpiConfig::default();
    spi_config.frequency = Hertz(8_000_000);

    let spi = Spi::new(
        p.SPI2,
        p.PB10,
        p.PC3,
        p.PA9,
        p.GPDMA1_CH0,
        p.GPDMA1_CH1,
        spi_config,
    );

    let nss = Output::new(p.PD14, Level::High, Speed::VeryHigh);
    let spi_device = ExclusiveDevice::new(spi, nss, Delay).unwrap();

    // Configure LR1110 control pins
    let reset = Output::new(p.PB2, Level::High, Speed::Low);
    let busy = ExtiInput::new(p.PB13, p.EXTI13, Pull::None, Irqs);
    let dio1 = ExtiInput::new(p.PB14, p.EXTI14, Pull::Down, Irqs);

    let rf_switch_rx: Option<Output<'_>> = None;
    let rf_switch_tx: Option<Output<'_>> = None;

    let iv = Stm32wbaLr1110InterfaceVariant::new(
        reset,
        busy,
        dio1,
        rf_switch_rx,
        rf_switch_tx,
    )
    .unwrap();

    let lr_config = lr1110_module::Config {
        chip: Lr1110Chip::new(),
        tcxo_ctrl: Some(TcxoCtrlVoltage::Ctrl3V0),
        use_dcdc: true,
        rx_boost: false,
    };

    let mut radio = lr1110_module::Lr1110::new(spi_device, iv, lr_config);

    // Reset and initialize the radio
    info!("Initializing LR1110...");
    radio.reset(&mut Delay).await.unwrap();
    embassy_time::Timer::after_millis(100).await;

    // =========================================================================
    // Demo 1: Hardware Random Number Generation
    // =========================================================================
    info!("-------------------------------------------");
    info!("Demo 1: Hardware Random Number Generator");
    info!("-------------------------------------------");

    info!("Generating random numbers using LR1110 HRNG:");
    for i in 0..5 {
        match radio.get_random_number().await {
            Ok(random) => {
                info!("  Random #{}: 0x{:08X}", i + 1, random);
            }
            Err(e) => {
                error!("  Failed to get random number: {:?}", e);
            }
        }
    }

    // =========================================================================
    // Demo 2: Select Crypto Element
    // =========================================================================
    info!("-------------------------------------------");
    info!("Demo 2: Select Crypto Element");
    info!("-------------------------------------------");

    info!("Selecting internal crypto engine...");
    match radio.crypto_select(CryptoElement::CryptoEngine).await {
        Ok(_) => info!("  Internal crypto engine selected"),
        Err(e) => error!("  Failed to select crypto element: {:?}", e),
    }

    // =========================================================================
    // Demo 3: Set an AES Key
    // =========================================================================
    info!("-------------------------------------------");
    info!("Demo 3: AES Key Management");
    info!("-------------------------------------------");

    // Example AES-128 key (16 bytes)
    // In production, use a securely generated key!
    let test_key: [u8; CRYPTO_KEY_LENGTH] = [
        0x2B, 0x7E, 0x15, 0x16, 0x28, 0xAE, 0xD2, 0xA6,
        0xAB, 0xF7, 0x15, 0x88, 0x09, 0xCF, 0x4F, 0x3C,
    ];

    info!("Setting AES key in GP0 slot...");
    info!("  Key: {:02X}", test_key);

    match radio.crypto_set_key(CryptoKeyId::Gp0, &test_key).await {
        Ok(status) => {
            info!("  Set key status: {:?}", status);
        }
        Err(e) => {
            error!("  Failed to set key: {:?}", e);
        }
    }

    // =========================================================================
    // Demo 4: AES Encryption
    // =========================================================================
    info!("-------------------------------------------");
    info!("Demo 4: AES Encryption");
    info!("-------------------------------------------");

    // Plaintext must be multiple of 16 bytes for AES
    let plaintext: [u8; 16] = [
        0x32, 0x43, 0xF6, 0xA8, 0x88, 0x5A, 0x30, 0x8D,
        0x31, 0x31, 0x98, 0xA2, 0xE0, 0x37, 0x07, 0x34,
    ];
    let mut ciphertext = [0u8; 16];

    info!("Encrypting data with AES-128...");
    info!("  Plaintext:  {:02X}", plaintext);

    match radio.crypto_aes_encrypt(CryptoKeyId::Gp0, &plaintext, &mut ciphertext).await {
        Ok(status) => {
            info!("  Encryption status: {:?}", status);
            if status == CryptoStatus::Success {
                info!("  Ciphertext: {:02X}", ciphertext);
            }
        }
        Err(e) => {
            error!("  Encryption failed: {:?}", e);
        }
    }

    // =========================================================================
    // Demo 5: AES Decryption
    // =========================================================================
    info!("-------------------------------------------");
    info!("Demo 5: AES Decryption");
    info!("-------------------------------------------");

    let mut decrypted = [0u8; 16];

    info!("Decrypting ciphertext...");
    info!("  Ciphertext: {:02X}", ciphertext);

    match radio.crypto_aes_decrypt(CryptoKeyId::Gp0, &ciphertext, &mut decrypted).await {
        Ok(status) => {
            info!("  Decryption status: {:?}", status);
            if status == CryptoStatus::Success {
                info!("  Decrypted:  {:02X}", decrypted);

                // Verify decryption matches original plaintext
                if decrypted == plaintext {
                    info!("  Verification: PASSED - Decrypted matches original!");
                } else {
                    error!("  Verification: FAILED - Mismatch!");
                }
            }
        }
        Err(e) => {
            error!("  Decryption failed: {:?}", e);
        }
    }

    // =========================================================================
    // Demo 6: AES-CMAC (Message Authentication Code)
    // =========================================================================
    info!("-------------------------------------------");
    info!("Demo 6: AES-CMAC Message Authentication");
    info!("-------------------------------------------");

    let message: [u8; 20] = [
        0x48, 0x65, 0x6C, 0x6C, 0x6F, 0x20, 0x4C, 0x6F,  // "Hello Lo"
        0x52, 0x61, 0x57, 0x41, 0x4E, 0x21, 0x00, 0x00,  // "RaWAN!.."
        0x01, 0x02, 0x03, 0x04,                          // Additional data
    ];

    info!("Computing AES-CMAC (MIC) over message...");
    info!("  Message: {:02X}", message);

    let mut mic: [u8; CRYPTO_MIC_LENGTH] = [0u8; CRYPTO_MIC_LENGTH];

    match radio.crypto_compute_aes_cmac(CryptoKeyId::Gp0, &message).await {
        Ok((status, computed_mic)) => {
            info!("  CMAC status: {:?}", status);
            if status == CryptoStatus::Success {
                mic = computed_mic;
                info!("  MIC (4 bytes): {:02X}", mic);
            }
        }
        Err(e) => {
            error!("  CMAC computation failed: {:?}", e);
        }
    }

    // =========================================================================
    // Demo 7: Verify AES-CMAC
    // =========================================================================
    info!("-------------------------------------------");
    info!("Demo 7: AES-CMAC Verification");
    info!("-------------------------------------------");

    info!("Verifying MIC over original message...");

    match radio.crypto_verify_aes_cmac(CryptoKeyId::Gp0, &message, &mic).await {
        Ok(status) => {
            match status {
                CryptoStatus::Success => {
                    info!("  MIC verification: PASSED");
                }
                CryptoStatus::ErrorFailCmac => {
                    warn!("  MIC verification: FAILED (CMAC mismatch)");
                }
                _ => {
                    warn!("  MIC verification status: {:?}", status);
                }
            }
        }
        Err(e) => {
            error!("  MIC verification failed: {:?}", e);
        }
    }

    // Try with a tampered message
    let mut tampered_message = message;
    tampered_message[0] ^= 0xFF;  // Flip bits in first byte

    info!("Verifying MIC over TAMPERED message...");

    match radio.crypto_verify_aes_cmac(CryptoKeyId::Gp0, &tampered_message, &mic).await {
        Ok(status) => {
            match status {
                CryptoStatus::Success => {
                    error!("  MIC verification: PASSED (unexpected!)");
                }
                CryptoStatus::ErrorFailCmac => {
                    info!("  MIC verification: FAILED as expected (tamper detected!)");
                }
                _ => {
                    warn!("  MIC verification status: {:?}", status);
                }
            }
        }
        Err(e) => {
            error!("  MIC verification failed: {:?}", e);
        }
    }

    // =========================================================================
    // Demo 8: Key Derivation
    // =========================================================================
    info!("-------------------------------------------");
    info!("Demo 8: Key Derivation");
    info!("-------------------------------------------");

    // Nonce for key derivation (16 bytes)
    let nonce: [u8; CRYPTO_NONCE_LENGTH] = [
        0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07,
        0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F,
    ];

    info!("Deriving new key from GP0 -> GP1 with nonce...");
    info!("  Nonce: {:02X}", nonce);

    match radio.crypto_derive_key(CryptoKeyId::Gp0, CryptoKeyId::Gp1, &nonce).await {
        Ok(status) => {
            info!("  Key derivation status: {:?}", status);
            if status == CryptoStatus::Success {
                info!("  New derived key stored in GP1 slot");

                // Encrypt with derived key to prove it works
                let test_data: [u8; 16] = [0x00; 16];
                let mut encrypted = [0u8; 16];

                match radio.crypto_aes_encrypt(CryptoKeyId::Gp1, &test_data, &mut encrypted).await {
                    Ok(enc_status) => {
                        if enc_status == CryptoStatus::Success {
                            info!("  Derived key verified - encryption successful");
                            info!("  Test encryption: {:02X}", encrypted);
                        }
                    }
                    Err(e) => {
                        error!("  Derived key test failed: {:?}", e);
                    }
                }
            }
        }
        Err(e) => {
            error!("  Key derivation failed: {:?}", e);
        }
    }

    // =========================================================================
    // Summary
    // =========================================================================
    info!("===========================================");
    info!("Crypto Demo Complete!");
    info!("===========================================");
    info!("");
    info!("The LR1110 crypto engine provides:");
    info!("  - Hardware AES-128 encryption/decryption");
    info!("  - AES-CMAC for message authentication");
    info!("  - Secure key storage (keys never leave chip)");
    info!("  - Key derivation for session keys");
    info!("  - Hardware random number generation");
    info!("");
    info!("These features enable secure LoRaWAN");
    info!("communication without exposing key material.");
    info!("===========================================");

    // Keep running
    loop {
        embassy_time::Timer::after_secs(60).await;
    }
}
