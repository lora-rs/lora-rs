# ESP32 + RFM95 (SX1276) LoRaWAN
## Example code for connecting to and transmitting data using LoRaWAN for ESP32 (2016 original ESP32) together with an RFM95(W) (SX1276) Lora Module (868MHz)

### IMPORTANT:
The RFM95(W) module is a bit wonky, as it does not have it's RFO (low transmission power) amplifer connected.
As such, you MUST set ***tx_boost*** to **true** in order for it to be able to transmit anything at all.
**tx_boost** enables the chip's **PA BOOST** (high transmission power) amplifer.
This is not documented on the datasheet.
More info on this issue can be found [here](https://www.disk91.com/2019/technology/lora/hoperf-rfm95-and-arduino-a-low-cost-lorawan-solution/) and [here](https://github.com/StuartsProjects/SX12XX-LoRa/issues/21#issuecomment-708568174).

### Example pin configuration:
| ESP32 pin | RFM 95 pin | Function              |
| --------- | -----------| --------------------- |
| 17        | 6 (RESET)  | Reset                 |
| 5         | 4 (SCK)    | SPI Clock             |
| 18        | 5 (NSS)    | SPI Chip select       |
| 19        | 2 (MISO)   | SPI MISO              |
| 23        | 3 (MOSI)   | SPI MOSI              |
| 21        | 14 (DIO0)  | Tx Done interrupt     |
| 22        | 15 (DIO1)  | Rx Timeout interrupt  |
