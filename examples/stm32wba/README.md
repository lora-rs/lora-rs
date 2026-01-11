# STM32WBA65RI + LR1110 LoRa Examples

This directory contains examples for using the LR1110 LoRa radio with the STM32WBA65RI microcontroller.

## Hardware Requirements

- **STM32WBA65RI** microcontroller (or compatible STM32WBA variant)
- **LR1110** LoRa radio module (or LR1120/LR1121)
- SPI connection between MCU and radio
- Optional: RF switches for antenna control

## Pin Connections

The examples use the following default pin configuration (adjust in code as needed):

| Function | Pin | Description |
|----------|-----|-------------|
| SPI1_NSS | PA4 | SPI Chip Select |
| SPI1_SCK | PB4 | SPI Clock |
| SPI1_MISO | PA11 | SPI Master In Slave Out |
| SPI1_MOSI | PA12 | SPI Master Out Slave In |
| LR1110_RESET | PB2 | Radio Reset (active low) |
| LR1110_DIO1 | PB1 | Radio IRQ (with EXTI) |
| RF_SWITCH_RX | PC6 | RX Antenna Switch (optional) |
| RF_SWITCH_TX | PC7 | TX Antenna Switch (optional) |

## Examples

### lora_p2p_send

Demonstrates sending LoRa packets in point-to-point mode.

**Features:**
- Initializes LR1110 with TCXO and DCDC
- Sends a test message "Hello from STM32WBA + LR1110!"
- Uses SF10, BW125kHz, CR 4/5
- Transmit power: 14 dBm
- Frequency: 915 MHz (US915 - adjust for your region)

**Run:**
```bash
cargo run --release --bin lora_p2p_send
```

### lora_p2p_receive

Demonstrates receiving LoRa packets in continuous mode.

**Features:**
- Continuous reception mode
- Displays received payload, RSSI, and SNR
- Matches parameters with send example
- Attempts ASCII string decode

**Run:**
```bash
cargo run --release --bin lora_p2p_receive
```

## Building

### Prerequisites

1. Install Rust toolchain:
   ```bash
   rustup target add thumbv8m.main-none-eabihf
   ```

2. Install probe-rs for flashing:
   ```bash
   cargo install probe-rs-tools
   ```

3. Connect your debug probe (ST-Link, J-Link, etc.)

### Build Commands

Build all examples:
```bash
cargo build --release
```

Build specific example:
```bash
cargo build --release --bin lora_p2p_send
```

Flash and run:
```bash
cargo run --release --bin lora_p2p_send
```

## Configuration

### LoRa Frequency

**IMPORTANT:** Set the correct frequency for your region in the source code:

```rust
const LORA_FREQUENCY_IN_HZ: u32 = 915_000_000; // US915
```

Common regions:
- **US915**: 902-928 MHz
- **EU868**: 863-870 MHz
- **AS923**: 915-928 MHz
- **AU915**: 915-928 MHz

### TCXO Voltage

Adjust the TCXO voltage based on your LR1110 board:

```rust
tcxo_ctrl: Some(TcxoCtrlVoltage::Ctrl1V8), // 1.8V TCXO
```

Options: `Ctrl1V6`, `Ctrl1V7`, `Ctrl1V8`, `Ctrl2V2`, `Ctrl2V4`, `Ctrl2V7`, `Ctrl3V0`, `Ctrl3V3`

### Power Amplifier

The LR1110 has three power amplifiers:
- **LP (Low Power)**: -17 to +14 dBm
- **HP (High Power)**: -9 to +22 dBm (default)
- **HF (High Frequency)**: -18 to +13 dBm (2.4 GHz)

To change PA selection:
```rust
chip: lora_phy::lr1110::Lr1110::with_pa(PaSelection::Lp),
```

## Troubleshooting

### Build Errors

1. **Missing target**: Run `rustup target add thumbv8m.main-none-eabihf`
2. **probe-rs not found**: Install with `cargo install probe-rs-tools`
3. **Wrong chip**: Update `.cargo/config.toml` with your exact chip variant

### Runtime Issues

1. **No output**: Check defmt-rtt connection with `probe-rs attach`
2. **SPI errors**: Verify pin connections and SPI clock speed
3. **No TX/RX**: Check frequency matches regional regulations
4. **IRQ not working**: Ensure DIO1 has EXTI configured correctly

### Common Issues

- **CRC errors**: Modulation parameters must match between TX and RX
- **No reception**: Check frequency, spreading factor, and bandwidth match
- **Low range**: Verify antenna connection and impedance matching

## License

MIT OR Apache-2.0

## References

- [LR1110 Datasheet](https://www.semtech.com/products/wireless-rf/lora-edge/lr1110)
- [STM32WBA Reference Manual](https://www.st.com/en/microcontrollers-microprocessors/stm32wba-series.html)
- [lora-rs Documentation](https://github.com/lora-rs/lora-rs)
