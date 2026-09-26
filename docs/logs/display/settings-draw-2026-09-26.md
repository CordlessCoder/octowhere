# Settings draw on the target, 2026-09-26

The S1 overview and D3 inner screens were measured on the ESP32-S3 at 240 MHz. The source is
`bench/settings-draw` at `51e7c2e`, based on `d856420`, with the settings implementation
snapshot at `691cf6c`. The feature is `settings-draw-bench`; ordinary builds leave it off.
The raw defmt capture is [settings-draw-2026-09-26.log](settings-draw-2026-09-26.log).

Each case draws the same settled state 120 times into alternating PSRAM framebuffers while core 1
flushes the resulting damage to the panel. Full cases redraw and flush the whole frame. Row
cases redraw through a clip and flush that row. The timer surrounds `Stage::draw` only: it does
not include `Stage::step`, the touch read, the swap wait or the SPI transfer. The readings are
fixed synthetic data, and the gestures that reach each screen are scripted.

| State | Median draw (ms) | 95th percentile (ms) |
| --- | ---: | ---: |
| S1 page 1, full | 31.576 | 32.556 |
| S1 brightness row | 6.842 | 6.859 |
| S1 page 2, full | 30.863 | 32.377 |
| S1 battery row | 6.938 | 6.969 |
| D3 device top | 24.426 | 25.422 |
| D3 device end | 33.706 | 34.078 |
| D3 replay GOOD | 26.543 | 27.501 |
| D3 replay failure | 26.729 | 28.133 |
| D3 offset | 26.935 | 28.241 |
| D3 zone | 27.397 | 27.782 |
| D3 brightness | 26.299 | 27.315 |
| D3 timeout | 24.030 | 24.704 |

Every full draw exceeds the 16.667 ms frame period. The two settled-row repaints fit it. This
is a recorded overshoot, not a reason to defer the remaining design work. The measurement does
not cover entry, page-swipe or editor-transition frames, or physical-panel legibility and touch.
Those still need direct checks.

Build the bench with `cargo build --release --offline --features settings-draw-bench` from a
worktree at `bench/settings-draw`. Flash its ELF with `espflash flash --partition-table
partitions.csv --port /dev/serial/by-id/usb-Espressif_USB_JTAG_serial_debug_unit_44:1B:F6:86:1A:38-if00
target/xtensa-esp32s3-none-elf/release/octowhere`, then capture defmt through the same port
until `[SETTINGS BENCH] done`. Reflash the ordinary release image afterwards. The benchmark
image was 1,165,472 bytes; the restored ordinary image was 1,163,584 bytes. The flash commands
did not request a settings-partition erase.
