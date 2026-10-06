#!/usr/bin/env bash
# Flashes the startup-timing-bench build to each board, then captures its boot several times.
# Each capture is a reset read, so every log is one boot. Build it first, from firmware/:
#   cargo build --release --offline --features startup-timing-bench
# Usage: tools/startup-timing.sh <out-dir> <boots> <by-id port>...
# `tools/startup-timing-summary.py <out-dir>` reads the logs. A capture's reset can swap the
# boards' ttyACM numbers under espflash, so the summary names each log's board by the MAC its
# [MESH] line prints, not by the port it was opened on.
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
elf=$root/firmware/target/xtensa-esp32s3-none-elf/release/octowhere
out=$1
boots=$2
shift 2
mkdir -p "$out"
for port in "$@"; do
    name=$(basename "$port" | sed -E 's/.*unit_.*:(..):(..)-if00/\1\2/')
    (cd "$root/firmware" && espflash flash --partition-table partitions.csv --port "$port" "$elf")
    for i in $(seq 1 "$boots"); do
        timeout 13 espflash monitor --non-interactive -L defmt --elf "$elf" --port "$port" \
            > "$out/$name-$i.log" 2>&1 || true
    done
done
