# C1 compass draw on the target, 2026-09-26

The C1 compass was measured on the ESP32-S3 at 240 MHz. The source is
`bench/compass-c1` at `99f3624`, based on committed master `b0e1bef`. Commit
`dd701be` carries the C1 source snapshot; `fbbf6fd` adds the feature-gated
measurement, and `99f3624` primes both framebuffers before the partial-draw
case. The raw defmt capture is
[compass-c1-draw-2026-09-26.log](compass-c1-draw-2026-09-26.log).

Each case draws the same state 120 times into alternating PSRAM framebuffers.
Core 1 flushes each resulting frame to the panel. Full cases redraw and flush
the entire frame. Before the one-degree case, both framebuffers are drawn with
the old heading. That case uses the real damage from a 47° to 48° stage update,
20,624 pixels. Brightness is set to 180/255. The timer surrounds
`Stage::draw` only. It excludes `Stage::step`, touch, swap wait, TE wait and SPI
transfer. The readings and gestures are synthetic fixtures. The benchmark
diverts normal bring-up after starting the display core.

| State | Median draw (ms) | 95th percentile (ms) |
| --- | ---: | ---: |
| Heading, settled | 22.857 | 24.344 |
| Interference, settled | 25.444 | 26.278 |
| Calibrating, settled | 19.902 | 22.199 |
| Top edge up, settled | 20.267 | 21.619 |
| NO DATA, settled | 17.137 | 17.663 |
| Entry blocks, 40 ms | 18.316 | 18.878 |
| Entry dark, 100 ms | 17.785 | 18.224 |
| Entry tiles, 150 ms | 20.493 | 21.768 |
| Entry pixels, 275 ms | 20.827 | 22.153 |
| Entry settled field, 400 ms | 22.974 | 24.411 |
| Swipe in progress | 20.407 | 21.521 |
| One-degree heading redraw | 11.151 | 11.206 |

All full-draw medians exceed the 16.667 ms frame period. The one-degree redraw
fits. This is a recorded overshoot, not a reason to defer other work. Repeating
one stage state does not measure the actual entry sequence or its end-to-end
frame pacing. Physical-panel legibility and touch were not checked in this run.

Build from a worktree at `bench/compass-c1` with `cargo build --release
--offline --features compass-c1-bench`. Flash its ELF with `espflash flash
--partition-table partitions.csv --port
/dev/serial/by-id/usb-Espressif_USB_JTAG_serial_debug_unit_44:1B:F6:86:1A:38-if00
target/xtensa-esp32s3-none-elf/release/octowhere`. Capture defmt through the
same port until `[COMPASS C1 BENCH] done`, then reflash the ordinary release
image. The benchmark image was 1,169,792 bytes; the normal image was 1,168,448
bytes. Neither flash command requested a settings-partition erase. The normal
image booted before and after the benchmark, with POWER, CLOCK, TOUCH, MOTION,
MAGNET and GNSS all answering. The restored boot capture is
[compass-c1-restored-boot-2026-09-26.log](compass-c1-restored-boot-2026-09-26.log).
