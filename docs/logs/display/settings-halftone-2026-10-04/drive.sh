#!/bin/bash
# Flashes an ELF built with `--features touch-inject,timing-log` to the board with hardware
# address 44:1B:F6:86:<mac>, logs it while opening the settings panel, dragging between its
# pages six times, closing it and opening it again, and writes <out>.log and <out>.marks, the
# log's count of timing lines when the drive starts and ends. From the repository root:
#
#   docs/logs/display/settings-halftone-2026-10-04/drive.sh 1A:38 <elf> <out>
set -u
mac=$1 elf=$2 out=$3
port=/dev/serial/by-id/usb-Espressif_USB_JTAG_serial_debug_unit_44:1B:F6:86:$mac-if00
probe=303a:1001:44:1B:F6:86:$mac
touch() {
    uv run --quiet tools/touch-inject.py "$@" --probe "$probe" --elf "$elf" 2>&1 | tail -1
    sleep 1
}
lines() { grep -ac 'timing draw' "$out.log"; }
espflash flash --partition-table partitions.csv --port "$port" "$elf" 2>&1 | tail -1
timeout 75 espflash monitor --non-interactive -L defmt --elf "$elf" --port "$port" > "$out.log" 2>&1 &
until grep -aq 'answered:' "$out.log" 2>/dev/null; do sleep 1; done
sleep 12
echo "== drive $(lines)" > "$out.marks"
touch swipe 233 80 233 420 --ms 300
sleep 2
for _ in 1 2 3; do
    touch swipe 380 250 80 250 --ms 400
    touch swipe 80 250 380 250 --ms 400
done
touch swipe 233 400 233 60 --ms 300
sleep 1
touch swipe 233 80 233 420 --ms 300
sleep 2
echo "== end $(lines)" >> "$out.marks"
wait
