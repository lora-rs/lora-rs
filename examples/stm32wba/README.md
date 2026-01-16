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
| SPI2_NSS | PD14 | SPI Chip Select (manual GPIO) |
| SPI2_SCK | PB10 | SPI Clock |
| SPI2_MISO | PA9 | SPI Master In Slave Out |
| SPI2_MOSI | PC3 | SPI Master Out Slave In |
| LR1110_RESET | PB2 | Radio Reset (active low) |
| LR1110_BUSY | PB13 | Radio BUSY signal (active high when processing) |
| LR1110_DIO1 | PB14 | Radio IRQ (with EXTI) |
| RF_SWITCH_RX | - | RX Antenna Switch (optional) |
| RF_SWITCH_TX | - | TX Antenna Switch (optional) |

**Note:** The LR1110 uses DIO0 as a BUSY signal. The BUSY pin goes HIGH when the chip is processing a command and is not ready to accept new commands. Always wait for BUSY to go LOW before sending the next SPI command.

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

### lr_fhss_ping

Demonstrates LR-FHSS (Long Range Frequency Hopping Spread Spectrum) transmission using the LR1110 radio. This example matches the configuration from Semtech's SWDM001 LR-FHSS demo package.

**Features:**
- LR-FHSS modulation (GMSK 488 bps)
- Frequency hopping with 3.906 kHz grid spacing
- Coding rate 5/6 with 2 header blocks
- 136.719 kHz bandwidth
- Applies High ACP workaround from SWDR001
- Compatible with Semtech's SWDM001 demo receivers

**Run:**
```bash
cargo run --release --bin lr_fhss_ping
```

**Reference:**
- [SWDM001 - LR-FHSS Demo Package](https://www.semtech.com)
- [SWDR001 - LR11xx Driver](https://www.semtech.com)

### lr1110_system_info

Demonstrates reading system information from the LR1110 radio using the SWDR001-based system functions.

**Features:**
- Hardware and firmware version identification
- Chip type detection (LR1110, LR1120, LR1121)
- Unique device identifier (UID) and Join EUI
- Temperature sensor reading
- Battery voltage reading
- Hardware random number generation
- Error status checking

**Run:**
```bash
cargo run --release --bin lr1110_system_info
```

### lr1110_gnss_scan

Demonstrates using the built-in GNSS scanner of the LR1110 radio to detect GPS and BeiDou satellites.

**Features:**
- Configure GNSS constellations (GPS and/or BeiDou)
- Set assistance position for improved performance
- Autonomous GNSS scanning
- Read detected satellite information (ID, C/N ratio, Doppler)
- Capture NAV message for LoRa Cloud position solving

**Run:**
```bash
cargo run --release --bin lr1110_gnss_scan
```

**Note:** For best results, ensure the antenna has a clear view of the sky. The NAV message output can be sent to [LoRa Cloud](https://www.loracloud.com) for position solving.

**Reference:**
- [SWDR001 - LR11xx Driver](https://www.semtech.com)
- [LoRa Cloud Geolocation](https://www.loracloud.com/documentation/geolocation)

### lr1110_wifi_scan

Demonstrates using the built-in WiFi passive scanner of the LR1110 radio to detect nearby access points.

**Features:**
- Configure scan parameters (signal type, channels, scan mode)
- Scan for WiFi 802.11 b/g/n access points
- Read detected AP information (MAC address, RSSI, channel, signal type)
- Cumulative timing information for power consumption estimation
- Results can be used for WiFi-based geolocation via LoRa Cloud

**Run:**
```bash
cargo run --release --bin lr1110_wifi_scan
```

**Note:** The WiFi scanner is a passive receiver that detects beacon frames from nearby access points. No WiFi transmission occurs. The MAC addresses and RSSI values can be sent to [LoRa Cloud](https://www.loracloud.com) for WiFi-based position solving.

**Reference:**
- [SWDR001 - LR11xx Driver](https://www.semtech.com)
- [LoRa Cloud WiFi Geolocation](https://www.loracloud.com/documentation/geolocation)

### lr1110_ranging_demo

Demonstrates RTToF (Round-Trip Time of Flight) ranging between two LR1110-based devices. This is a Rust implementation of Semtech's lr11xx_ranging_demo.

**Features:**
- Two-device ranging: Manager (initiator) and Subordinate (responder)
- LoRa initialization handshake for device synchronization
- RTToF ranging with 39-channel frequency hopping (US915 band)
- Distance calculation with median filtering for noise reduction
- RSSI measurement for path loss analysis
- Packet Error Rate (PER) calculation

**Hardware Requirements:**
- Two STM32WBA65RI + LR1110 boards
- Both configured with the same pin connections

**How It Works:**
1. **Initialization Phase (LoRa):**
   - Manager sends an initialization packet with ranging address
   - Subordinate responds with its RSSI measurement
   - Both devices synchronize on the first frequency channel

2. **Ranging Phase (RTToF):**
   - Devices hop through 39 frequency channels
   - Manager sends RTToF request, Subordinate responds
   - Manager measures round-trip time and calculates distance
   - Each channel provides one distance measurement

3. **Result Processing:**
   - Median filtering removes outliers
   - Final distance is reported along with RSSI and PER

**Run as Manager (Device 1):**
```bash
cargo run --release --bin lr1110_ranging_demo --features manager
```

**Run as Subordinate (Device 2):**
```bash
cargo run --release --bin lr1110_ranging_demo
```

**Output Example (Manager):**
```
=== Ranging Session Complete ===
Successful measurements: 35/39
Packet Error Rate: 10%
Median distance: 15 meters
Manager RSSI: -45 dBm
Subordinate RSSI: -47 dBm
================================
```

**Configuration:**
Edit the constants at the top of `lr1110_ranging_demo.rs`:
- `RF_FREQUENCY` - Base frequency for initialization (default: 915 MHz)
- `TX_OUTPUT_POWER_DBM` - Transmit power (default: 14 dBm)
- `LORA_SF` - Spreading factor (default: SF8)
- `LORA_BW` - Bandwidth (default: 500 kHz)
- `RANGING_ADDRESS` - Device pairing address

**Frequency Bands:**
The demo includes frequency hopping tables for multiple regions:
- US915 (902-928 MHz) - Default
- EU868 (863-870 MHz)
- CN490 (490-510 MHz)
- ISM 2.4 GHz

To change the frequency band, modify the `ranging_channels::US915` reference in the code.

**Reference:**
- [lr11xx_ranging_demo](https://github.com/Lora-net/SWDM001) - Original C implementation
- [LR1110 Ranging Application Note](https://www.semtech.com)

### lr1110_firmware_update

Demonstrates how to update the LR1110 firmware using the bootloader interface. This is a Rust implementation based on Semtech's SWTL001 firmware updater tool.

**Features:**
- Reset chip and enter bootloader mode
- Validate bootloader version and chip type
- Read device identifiers (PIN, Chip EUI, Join EUI)
- Erase flash memory
- Write encrypted firmware in 256-byte chunks
- Reboot and verify new firmware version

**Firmware Update Process:**
1. Chip is reset and enters bootloader mode
2. Bootloader version is validated against firmware type
3. Device identifiers are logged for reference
4. Entire flash is erased (required before write)
5. Firmware image is written in 64-word (256-byte) chunks
6. Chip reboots and new firmware version is verified

**Obtaining Firmware Images:**

Firmware images are pre-encrypted by Semtech. To obtain them:

1. Download the SWTL001 package from [Semtech](https://www.semtech.com)
2. Find the firmware header file in `application/inc/` (e.g., `lr1110_transceiver_0401.h`)
3. Convert the C array to Rust format:

```c
// C format (from SWTL001):
const uint32_t lr11xx_firmware_image[] = {
    0x3dd0a84a, 0xd225a051, 0x3b4ab123, ...
};
```

```rust
// Rust format (for this example):
const FIRMWARE_IMAGE: &[u32] = &[
    0x3dd0a84a, 0xd225a051, 0x3b4ab123, ...
];
```

4. Update `FIRMWARE_TYPE` and `EXPECTED_FIRMWARE_VERSION` constants

**Supported Firmware Types:**
- LR1110 Transceiver (bootloader 0x6500)
- LR1110 Modem V1 (bootloader 0x6500)
- LR1120 Transceiver (bootloader 0x2000)
- LR1121 Transceiver (bootloader 0x2100)
- LR1121 Modem V2 (bootloader 0x2100)

**Run:**
```bash
cargo run --release --bin lr1110_firmware_update
```

**Output Example:**
```
==============================================
LR1110 Firmware Update Example
==============================================
Starting firmware update...
Firmware type: Lr1110Transceiver
Expected version: 0x0401
Firmware size: 51234 words (204936 bytes)
Step 1: Resetting chip to enter bootloader mode...
Step 2: Reading bootloader version...
  Hardware version: 0x22
  Chip type: 0xDF
  Bootloader version: 0x6500
  Bootloader version validated OK
Step 3: Reading device identifiers...
  PIN: 12345678
  Chip EUI: 0011223344556677
  Join EUI: AABBCCDDEEFF0011
Step 4: Erasing flash memory...
  Flash erased successfully
Step 5: Writing firmware image...
  Progress: 10% (5123/51234 words)
  ...
  Progress: 100% (51234/51234 words)
  Firmware written successfully
Step 6: Rebooting to execute new firmware...
Step 7: Verifying firmware version...
  Firmware version: 0x0401
  Firmware version verified OK
==============================================
FIRMWARE UPDATE SUCCESSFUL!
==============================================
```

**Warning:**
- The firmware erase operation is DESTRUCTIVE and cannot be undone
- Make sure you have the correct firmware for your chip type
- Do not power off during the update process

**Reference:**
- [SWTL001 - LR11xx Firmware Update Tool](https://www.semtech.com)
- [AN1200.57 - LR1110 Upgrade of the Program Memory](https://www.semtech.com)

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
