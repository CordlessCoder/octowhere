#!/usr/bin/env bash
# Reads each board's marks after every cold start, which a serial capture would turn into a
# reset. Power the boards off and on as often as wanted, a few seconds off and at least 10 s on;
# each time a board's port comes back, this reads its marks over the USB JTAG, which does not
# reset it, until they show its start-up over, into <out-dir>/<MAC>-<n>.log. A port can come
# back more than once in a power-up, so a fixed wait read too early. Ctrl-C stops it.
# The boards must run the startup-timing-bench build in firmware/target.
# Usage: tools/startup-timing-cold.sh <out-dir> <MAC>...   e.g. 1A38 1C1C
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
elf=$root/firmware/target/xtensa-esp32s3-none-elf/release/octowhere
out=$1
shift
mkdir -p "$out"
watch() {
    local name=$1
    # A watch started again goes on from the reads already there.
    local n
    n=$(find "$out" -name "$name-*.log" | wc -l)
    local mac=44:1B:F6:86:${name:0:2}:${name:2:2}
    local port=/dev/serial/by-id/usb-Espressif_USB_JTAG_serial_debug_unit_$mac-if00
    while true; do
        while [ -e "$port" ]; do sleep 0.2; done
        while [ ! -e "$port" ]; do sleep 0.2; done
        n=$((n + 1))
        local tries=0
        while true; do
            sleep 3
            tries=$((tries + 1))
            uv run "$root/tools/startup-timing-read.py" --elf "$elf" --probe "303a:1001:$mac" \
                > "$out/$name-$n.log" 2>&1 || true
            if grep -q "startup over" "$out/$name-$n.log"; then
                echo "$name cold start $n read"
                break
            fi
            if [ "$tries" -ge 10 ]; then
                echo "$name cold start $n: no end of start-up after $tries reads, see $out/$name-$n.log"
                break
            fi
        done
    done
}
for name in "$@"; do
    watch "$name" &
done
wait
