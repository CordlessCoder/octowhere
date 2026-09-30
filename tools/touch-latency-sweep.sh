#!/usr/bin/env bash
# Runs the synthetic finger on the old touch handoff and the new, and summarises each.
#
#     tools/touch-latency-sweep.sh <port> <out-dir> [seconds]
#
# Each variant is built, flashed and read for `seconds` (200 by default) from a reset, and its
# log and summary land in <out-dir>/<variant>.log and .txt. Stop any other reader of the port
# first. Opening the port resets the board.
set -euo pipefail
port=$1
out=$2
seconds=${3:-200}
elf=target/xtensa-esp32s3-none-elf/release/octowhere
mkdir -p "$out"
declare -A variants=(
    [old-handoff]="touch-latency-fifo"
    [new]="touch-latency-synthetic"
)
for name in old-handoff new; do
    cargo build --release --features "${variants[$name]}"
    espflash flash --partition-table partitions.csv --port "$port" --non-interactive "$elf"
    timeout "$seconds" espflash monitor --non-interactive -L defmt --elf "$elf" --port "$port" \
        > "$out/$name.log" 2>&1 || true
    uv run tools/touch-latency-summary.py "$out/$name.log" > "$out/$name.txt"
done
