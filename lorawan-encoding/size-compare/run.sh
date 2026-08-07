#!/usr/bin/env bash
# Builds each configuration for thumbv7em-none-eabi, links it with
# --gc-sections, and reports the text size: the flash cost of the
# parse/decrypt workload through the borrowed-view API.
set -euo pipefail
cd "$(dirname "$0")"

TARGET=thumbv7em-none-eabi
LIB=target/$TARGET/release/libsize_compare.a
LLD=$(rustc --print sysroot)/lib/rustlib/x86_64-unknown-linux-gnu/bin/rust-lld

measure() {
    local lto=$1 feature=$2
    shift 2
    CARGO_PROFILE_RELEASE_LTO=$lto cargo build --quiet --offline --release --target $TARGET \
        --no-default-features --features "$feature" 2>/dev/null
    local keep=()
    for sym in "$@"; do
        keep+=(-u "$sym")
    done
    $LLD -flavor gnu --gc-sections -e "$1" "${keep[@]}" -o "/tmp/size-$feature-$lto.elf" "$LIB"
    size "/tmp/size-$feature-$lto.elf" | awk 'NR == 2 { print $1 }'
}

printf "%-6s %-10s %s\n" cfg "lto=fat" "lto=off"
for cfg in new1 new3; do
    case $cfg in
        new1) syms=(work_slice_new) ;;
        new3) syms=(work_slice_new work_array_new work_wrapper_new) ;;
    esac
    printf "%-6s %-10s %s\n" "$cfg" \
        "$(measure fat "$cfg" "${syms[@]}")" \
        "$(measure off "$cfg" "${syms[@]}")"
done
