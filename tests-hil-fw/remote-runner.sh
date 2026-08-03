#!/usr/bin/env bash
# cargo runner: ship the test ELF to the lora-hil Pi and run it there under
# probe-rs. embedded-test's host side lives inside probe-rs, so all libtest
# args (--list, --exact, --format ...) pass straight through.
set -euo pipefail
ELF="$1"
shift
scp -q "$ELF" lora-hil:/tmp/hil-test.elf
exec ssh lora-hil "probe-rs run --chip STM32WL55JCIx /tmp/hil-test.elf $(printf '%q ' "$@")"
