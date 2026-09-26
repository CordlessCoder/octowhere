#!/bin/sh
# Flash the clock bench and save its log.
#   tools/clock-bench.sh PORT OUT.log [SECONDS]
# Build first: cargo build --release --features clock-draw-bench
# Summarise with: uv run tools/clock-bench-summary.py OUT.log
set -eu
port=$1 out=$2 secs=${3:-70}
elf=target/xtensa-esp32s3-none-elf/release/octowhere
if pgrep espflash >/dev/null; then echo "espflash is already running" >&2; exit 1; fi
espflash flash --partition-table partitions.csv --port "$port" "$elf"
# A reset read: the first two seconds or so after the reset are lost.
timeout "$secs" espflash monitor --non-interactive -L defmt --elf "$elf" --port "$port" > "$out" 2>&1 || true
