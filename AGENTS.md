# AGENTS.md

This repository contains `no_std` Rust firmware for the Waveshare ESP32-S3-Touch-AMOLED-1.75.
The board has a 466x466 round CO5300 AMOLED, CST9217 touch controller, QMI8658 IMU, PCF85063A
RTC, AXP2101 PMIC, TCA9554 I/O expander, LC76G GNSS module, and an SX1272-based LoRa module.

Hardware topology, pin routing, initialization findings, and saved reference documents are in
[`docs/hardware-notes.md`](docs/hardware-notes.md) and
[`docs/datasheets/README.md`](docs/datasheets/README.md). Check those before changing board
initialization or peripheral mappings.

## Build and test

The `esp` toolchain from [`rust-toolchain.toml`](rust-toolchain.toml) and the target from
[`.cargo/config.toml`](.cargo/config.toml) are selected automatically.

```text
cargo build --release --offline
cargo clippy --release --offline -- -D warnings
cargo +stable test --manifest-path host-tests/Cargo.toml \
  --target x86_64-unknown-linux-gnu --locked
```

`cargo run --release` uses the configured `espflash` runner to flash and monitor the board.
Feature-specific builds pass their features to Cargo, for example:
`cargo run --release --features gnss-raw-log`.

The host harness and its scope are described in [`host-tests/README.md`](host-tests/README.md).
The root package targets the board; the host harness is the place for pure-logic tests.

The data-cache settings in [`.cargo/config.toml`](.cargo/config.toml) affect drawing and SPI flush
timings. Change them only with a measurement.

## Rendering architecture

`async_main` on core 0 produces frames and `second_core` on core 1 owns the display SPI/DMA path.
They exchange two PSRAM framebuffers through `util::Swap`.

- Core 0 reads touch and sensor state, draws into its current framebuffer, records `dirty`, and
  hands the state to core 1.
- Core 1 waits for display TE with a timeout, flushes the handed-off regions through
  `Co5300Display`, and returns the other framebuffer.
- `needs_full_redraw` identifies the regions that must be redrawn in the alternate framebuffer.
  A buffer is not assumed to retain the pixels drawn into the other buffer.
- Partial flushing is active. A full redraw is selected only when the accumulated damage is full.

`util::Swap` is lock-free and has `unsafe impl Send/Sync`. A started `SwapThreadFuture` must be
allowed to complete; dropping it poisons the thread and a later `get()` panics.

The display path is split across:

- [`src/drivers/qspi_bus.rs`](src/drivers/qspi_bus.rs), which owns QSPI command transfers.
- [`src/drivers/co5300.rs`](src/drivers/co5300.rs), which initializes the panel, handles TE,
  address windows, brightness, and double-buffered DMA pixel streaming.
- [`src/drivers/framebuffer.rs`](src/drivers/framebuffer.rs), which stores draw-target pixels in
  PSRAM and aligns partial flushes to the controller's pixel granularity.

`chrome::Color` selects the framebuffer and panel colour format. Keep the format consistent through
the QSPI, display, framebuffer, and UI layers.

## Memory

- The current internal heap is 260 KiB, allocated by `esp_alloc::heap_allocator!` in `main`.
- PSRAM is registered in the separate `PSRAM_HEAP` static. Framebuffers must be allocated with
  `FB::alloc(&PSRAM_HEAP)` rather than the global allocator.
- The current RGB565 configuration uses 466 × 466 × 2 = 434,312 bytes per framebuffer, with two
  framebuffers.
- Core 1 uses the 8 KiB `CORE1_STACK` static.

Check the allocator and framebuffer definitions in [`src/main.rs`](src/main.rs) and
[`src/chrome.rs`](src/chrome.rs) when changing memory placement.

## Fonts and layout

The active UI uses the compile-time fontdue renderer in [`src/chrome.rs`](src/chrome.rs), with the
Marathon Shapiro and PPFraktion font data under `assets/`. `embedded-layout` supplies the current
text alignment helpers. The legacy u8g2 conversion files under `assets/` are not part of the active
renderer.

The `fontdue-target-bench` features run the target font benchmark from `main`; keep benchmark-only
paths out of normal firmware behavior.

## Dependencies and conventions

The root manifest owns dependency versions, features, and git patches. It currently patches the
fontdue pair and `tca9554`; `lc76g` and `sx127xlora` are local crates. Check [`Cargo.toml`](Cargo.toml)
before relying on a fork-only API or changing a dependency.

`src/lib.rs` deliberately carries `#![expect(unused)]` while the UI is being built. `PERF:` comments
mark measured or suspected hot spots and open questions; they are context, not a task list.
