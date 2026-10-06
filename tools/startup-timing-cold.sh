#!/usr/bin/env bash
# Reads each board's marks after every cold start, which a serial capture would turn into a
# reset. Power the boards off and on as often as wanted, a few seconds off and at least 10 s on;
# each time a board's port comes back, this waits for its start-up to end and reads its marks
# over the USB JTAG, which does not reset it, into <out-dir>/<MAC>-<n>.log. Ctrl-C stops it.
# The boards must run the startup-timing-bench build in firmware/target.
# Usage: tools/startup-timing-cold.sh <out-dir> <MAC>...   e.g. 1A38 1C1C
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
elf=$root/firmware/target/xtensa-esp32s3-none-elf/release/octowhere
out=$1
shift
mkdir -p "$out"
watch() {
    local name=$1 n=0
    local mac=44:1B:F6:86:${name:0:2}:${name:2:2}
    local port=/dev/serial/by-id/usb-Espressif_USB_JTAG_serial_debug_unit_$mac-if00
    while true; do
        while [ -e "$port" ]; do sleep 0.2; done
        while [ ! -e "$port" ]; do sleep 0.2; done
        sleep 9
        n=$((n + 1))
        if uv run "$root/tools/startup-timing-read.py" --elf "$elf" --probe "303a:1001:$mac" \
            > "$out/$name-$n.log" 2>&1; then
            echo "$name cold start $n read"
        else
            echo "$name cold start $n: read failed, see $out/$name-$n.log"
        fi
    done
}
for name in "$@"; do
    watch "$name" &
done
wait
