# Semtech LR1110 development kit examples (NUCLEO-L476RG + LR1110MB1DxS)

Examples for the standard Semtech LR1110 development kit arrangement: an
**LR1110MB1DxS** shield (from the LR1110DVK1TxKS kits) seated on a
**NUCLEO-L476RG**'s Arduino headers. No jumper wires are needed; the shield's
Arduino-header pinout maps to the pins used in the examples:

| Function | Arduino | STM32L476 pin |
|----------|---------|---------------|
| SPI SCK | D13 | PA5 |
| SPI MISO | D12 | PA6 |
| SPI MOSI | D11 | PA7 |
| NSS | D7 | PA8 |
| NRESET | A0 | PA0 |
| BUSY | D3 | PB3 |
| IRQ (DIO9) | D5 | PB4 |

The RF switch is wired to the LR1110's own RFSW0/RFSW1 DIOs and configured
through `SetDioAsRfSwitch`; the TCXO runs from DIO3 at 3.0 V. Both settings
follow Semtech's shield reference configuration (SWSD001).

## Examples

- `lora_p2p_send` / `lora_p2p_receive` — raw LoRa PHY point-to-point between
  two kits (or any other LoRa radio with matching parameters).
- `lora_lorawan` — LoRaWAN OTAA join. Set the region, DevEUI/AppEUI/AppKey
  for your network before flashing.

## Running

```sh
cargo run --release --bin lora_p2p_send
```

probe-rs cannot autodetect the STM32L476; the runner in
`.cargo/config.toml` passes `--chip STM32L476RGTx` explicitly.

## Notes

- The frequencies/region default to US915; adjust for your region.
- The shield's low-power PA path tops out at +14 dBm below 400 MHz. The
  driver's PA tables also support the high-power PA (up to +22 dBm) via
  `PaSelection::Hp`, subject to regulatory limits.
- If your LR1110 still runs old transceiver firmware (`get_version`;
  boards have shipped with 0x0303 from 2020), consider updating it with
  Semtech's images from the Lora-net/radio_firmware_images repository.
