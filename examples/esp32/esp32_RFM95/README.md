# ESP32 + RFM95 LoRaWAN
## Example code for connecting to and transmitting data using LoRaWAN for ESP32 (2016 original ESP32) together with an RFM95(W) Lora Module (868MHz)

## This example works as of 2025-05-29 - no guaratees of working code provided after this date.

### IMPORTANT:
The RFM95(W) module is a bit wonky, as it does not have it's RFO (low transmission power) amplifer connected.
As such, you MUST set ***tx_boost*** to **true** in order for it to be able to transmit anything at all.
**tx_boost** enables the chip's **PA BOOST** (high transmission power) amplifer.
This is not documented on the datasheet.
More info on this issue can be found [here](https://www.disk91.com/2019/technology/lora/hoperf-rfm95-and-arduino-a-low-cost-lorawan-solution/) and [here](https://github.com/StuartsProjects/SX12XX-LoRa/issues/21#issuecomment-708568174) 
