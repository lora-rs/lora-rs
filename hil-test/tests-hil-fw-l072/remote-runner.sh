#!/usr/bin/env bash
# cargo runner: run the test ELF on the lora-hil Pi's B-L072Z DUT through
# probe-rs remote (probe-rs-serve.service on the Pi, port 3000; client
# uploads the ELF over the websocket). --probe pins the board's onboard
# ST-LINK/V2-1; the rig also carries the WL55's STLINK-V3.
# Both ends run probe-rs 0.32.0 built with --features remote (not in the
# release binaries); rebuild from the probe-rs checkout on lounas if either
# end is reinstalled.
#
# Fallback if the server is down: scp "$1" lora-hil:/tmp/hil-test.elf &&
# ssh lora-hil probe-rs run --chip STM32L072CZTx /tmp/hil-test.elf <args>
set -euo pipefail
# --connect-under-reset: the board's V2J28-era ST-LINK fails plain attaches
# intermittently (JtagDbgPowerError/SwdApWait); under reset it is reliable.
exec "$HOME/.cargo/bin/probe-rs" run --chip STM32L072CZTx \
    --probe 0483:374b:066EFF495351677867143312 \
    --connect-under-reset \
    --host "${PROBE_RS_HOST:-ws://192.168.1.190:3000}" \
    --token "$(cat "${PROBE_RS_TOKEN_FILE:-$HOME/.config/probe-rs/remote-token}")" \
    "$@"
