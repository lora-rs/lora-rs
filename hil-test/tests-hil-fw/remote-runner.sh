#!/usr/bin/env bash
# cargo runner: run the test ELF on the lora-hil Pi's DUT through probe-rs
# remote (probe-rs-serve.service on the Pi, port 3000; client uploads the
# ELF over the websocket). embedded-test's host side lives inside probe-rs,
# so all libtest args (--list, --exact, --format ...) pass straight through.
# Both ends run probe-rs 0.32.0 built with --features remote (not in the
# release binaries); rebuild from the probe-rs checkout on lounas if either
# end is reinstalled.
#
# Fallback if the server is down: scp "$1" lora-hil:/tmp/hil-test.elf &&
# ssh lora-hil probe-rs run --chip STM32WL55JCIx /tmp/hil-test.elf <args>
set -euo pipefail
# --probe pins the WL55's onboard STLINK-V3: the rig carries a second probe
# (the B-L072Z DUT's ST-LINK/V2-1) and an unpinned run grabs an arbitrary one.
exec "$HOME/.cargo/bin/probe-rs" run --chip STM32WL55JCIx \
    --probe 0483:374e:002700443234511733353533 \
    --host ws://192.168.1.190:3000 \
    --token "$(cat "$HOME/.config/probe-rs/remote-token")" \
    "$@"
