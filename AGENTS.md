# AGENTS.md

This file provides guidance to Codex (Codex.ai/code) when working with code in this repository.

Bare-metal Rust firmware for the Waveshare ESP32-S3-Touch-AMOLED-1.75: a 466x466 round CO5300
AMOLED, CST9217 touch, QMI8658 IMU, PCF85063A RTC, AXP2101 PMIC and a TCA9554 IO expander. No RTOS
beyond `esp-rtos` + Embassy; no `std`.

## Build and flash

```
cargo build --release      # or plain `cargo build` — dev is opt-level "s" + thin LTO
cargo run --release        # .cargo/config.toml runner = espflash flash --monitor
cargo clippy --release
```

Target `xtensa-esp32s3-none-elf` is set in `.cargo/config.toml`, so no `--target` flag.
Root `cargo test` targets the board. Host checks live in `host-tests/` and compile
the production geometry, touch and synchronisation modules separately.

```
cargo +stable test --manifest-path host-tests/Cargo.toml --target x86_64-unknown-linux-gnu --locked
```

Toolchain: `rust-toolchain.toml` selects espup's `esp` toolchain. It supports the
nightly features used here. A locally built compiler is not required.

`ESP_HAL_CONFIG_DATA_CACHE_SIZE` and `..._LINE_SIZE` in `.cargo/config.toml` are worth ±15% on
draw and flush time. Change them only with a measurement.

## Rendering pipeline

Two cores run a producer/consumer pair over a pair of framebuffers, handed back and forth by
`util::Swap`. Everything below is the reason a change in one half breaks the other.

- **Core 0** (`main` in `src/main.rs`): reads touch, draws into the framebuffer it currently owns,
  then `swap().await`.
- **Core 1** (`second_core` task): owns the SPI/DMA peripheral and the `Co5300Display`. Waits on the
  TE pin for vsync, flushes its framebuffer, then `swap().await`.

`SwapState` carries the framebuffer, `dirty`, `needs_full_redraw` and `Timings` across the handoff.
The subtle invariant is `needs_full_redraw`: because the two buffers alternate, whatever core 0 drew
this frame is stale in the *other* buffer, so the set of regions it just touched must be repainted
next frame before anything new goes down. `update_text`/`update_touch` return exactly that set.

Core 1 flushes region-by-region when `dirty` is partial and whole-screen otherwise. **Core 0
currently forces `dirty.make_full()` every frame**, so the partial-flush path is live code that
nothing exercises; drop that call to re-enable it.

`util::Swap` is hand-rolled lock-free synchronisation with `unsafe impl Send/Sync`. Dropping a
`SwapThreadFuture` before completion poisons the `SwapThread` and the next `get()` panics.

## Display stack

Three layers, bottom up:

- `drivers/qspi_bus.rs` — `QspiBus` wraps `SpiDma` + a `DmaTxBuf` + the CS pin. Command sequences are
  `[QSPIOperation]` arrays executed blocking or async.
- `drivers/co5300.rs` — panel init, address windows, brightness, TE/vsync. `PixelStream` is the hot
  path: it double-buffers two `DmaTxBuf`s, so `flush_buf_async` overlaps the DMA write of one buffer
  with the caller filling the other, then swaps them. `Drop` raises CS.
- `drivers/framebuffer.rs` — `Framebuffer<N, WIDTH, HEIGHT, C>` in PSRAM, implements `DrawTarget`.
  `N` must equal `buffer_size::<C>(WIDTH, HEIGHT)`; a const assert catches it. `flush_region` snaps
  the rect to a 2x2 grid because the CO5300 rejects odd partial writes.

Colour format is a type parameter (`Co5300ColorMode`: `Rgb565`, `Rgb888`, `Gray8`) threaded through
all three layers. `chrome::Color` is the single alias that picks it — change it there.

`ui/dirty.rs`: `DirtyAreas` is a fixed grid of per-cell bounding boxes plus a `full` flag.
`chrome::Dirty` instantiates it 2x2.

## Memory

- Internal heaps: 72 KB reclaimed + 260 KB, via `esp_alloc::heap_allocator!` in `main`.
- PSRAM (octal, 80 MHz) is registered into a *separate* `PSRAM_HEAP` static, not the global
  allocator. Ask for it explicitly: `FB::alloc(&PSRAM_HEAP)`.
- Each framebuffer is 466·466·2 = 434,312 B, and there are two.
- Core 1's stack is a `static mut CORE1_STACK` of 8 KB.

## Fonts

Two renderers coexist:

- **u8g2** (`u8g2-fonts`) is what the UI uses — pre-rasterised bitmaps, fast, fixed sizes.
  `chrome::HEADING_FONT_FAST` / `MEDIUM_FONT_FAST`.
- **fontdue** rasterises outlines at runtime with the font parsed at compile time by
  `fontdue_macros::fontdue_font_from_file!`. `chrome::FontdueRenderer` implements
  `embedded_graphics::text::renderer::TextRenderer`. All call sites are commented out; it is kept for
  scalable text and antialiasing.

The `.u8g2` blobs are `include_bytes!`d. Regenerating one: TTF → BDF (FontForge) → `.c` with u8g2's
`bdfconv` → raw bytes by compiling the two-line `to_u8g2.cpp`/`u8g2_font_to_bytes.cpp` next to the
`.c` file and redirecting stdout. The `#include` in those files names the font being converted and
has to be edited per font.

## Patched dependencies

`[patch.crates-io]` redirects esp-hal, esp-rtos, esp-alloc, esp-println, esp-backtrace,
esp-bootloader-esp-idf and esp-rom-sys to `github.com/CordlessCoder/esp-hal`, plus forks of
`fontdue`, `fontdue-macros` and `tca9554`, and `u8g2-fonts` from `iriswebb`. The hot paths here
(`half_duplex_write_and_wait`, `DmaTxBuf` handling, `fontdue::raster::Raster`) depend on fork-only
APIs, so a feature that needs a HAL change goes into the fork first.

## Conventions

`src/lib.rs` carries `#![expect(unused)]` — dead code is expected while the UI is being built, and
unused warnings are silenced rather than fought.

`PERF:` comments mark measured or suspected hot spots and open questions about them. They are notes
to the next person, not TODOs to clear.
