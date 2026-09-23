# AGENTS.md

This repository contains `no_std` Rust firmware for the Waveshare ESP32-S3-Touch-AMOLED-1.75.
The board has a 466x466 round CO5300 AMOLED, CST9217 touch controller, QMI8658 IMU, BMM350
magnetometer, PCF85063A RTC, AXP2101 PMIC, TCA9554 I/O expander, LC76G GNSS module, and an
SX1272-based LoRa module.

Hardware topology, pin routing, initialization findings, and saved reference documents are in
[`docs/hardware-notes.md`](docs/hardware-notes.md) and
[`docs/datasheets/README.md`](docs/datasheets/README.md). Check those before changing board
initialization or peripheral mappings.

## Repository map

- `src/drivers/` owns the display path: QSPI, the CO5300 panel, and the PSRAM framebuffer.
- `src/peripherals/` owns the I2C devices: touch, power, RTC, magnetometer, and the shared-bus
  helper. The IMU comes from `ph-qmi8658` rather than a local module.
- `src/ui/` owns dirty tracking, geometry, touch input mapping, IMU presentation, and the screen
  prototypes.
- `src/chrome.rs` is the font and draw-target layer. `src/main.rs` holds both cores, the sensor
  task, and the frame loop.
- `src/board.rs` holds display geometry, the TCA9554 line indices, and the I2C addresses that
  external crates take. GPIO numbers are not there: they live at the binding sites in `main.rs`,
  and `docs/hardware-notes.md` has the pin map.
- `crates/` holds the local `lc76g`, `sx127x-lora` and `sx127x-common` crates.
- `host-tests/` is the std test harness.
- `docs/` holds hardware reference: the topology notes, the datasheet pack, and captured GNSS and
  LoRa traces under `docs/logs/`.
- `context/` holds the agent-facing documents below. This file stays at the root.

`context/` carries what is not in the code:

- [`context/BACKLOG.md`](context/BACKLOG.md) lists open work that is not in progress, and says
  where each entry's detail lives. Read it when choosing what to do next.
- [`context/GRAPHICS-PROTOTYPES.md`](context/GRAPHICS-PROTOTYPES.md) explains the three renderer
  architectures in `src/ui/prototypes.rs` and the `ACTIVE_ARCHITECTURE` constant that selects one.
- [`context/HARDWARE-VERIFICATION.md`](context/HARDWARE-VERIFICATION.md) lists open hardware
  questions from static review. They are questions, not confirmed defects.
- [`context/IMPLEMENTATION.md`](context/IMPLEMENTATION.md) is a finished multi-agent brief kept as
  a record. Its partition is historical.
- [`context/LORA-PROTOCOL.md`](context/LORA-PROTOCOL.md) is the agreed design for the location
  mesh: gossip digest, GPS-anchored TDMA, packet layout, crypto and pairing. Nothing in it is
  implemented yet.
- [`context/marathon-ui-cross-project-handoff.md`](context/marathon-ui-cross-project-handoff.md)
  is the design doctrine. See "Design language" below.
- [`context/palette-reference.md`](context/palette-reference.md) records the colour values from the
  reference board and the role each one plays in `src/chrome.rs`.

`IMPLEMENTATION.md` tells the reader to keep full-frame flushing until a hardware check passes. It
is a record of what that round was told, not current instruction, and the firmware now flushes
partially. The question is still open and lives in `HARDWARE-VERIFICATION.md`, which records that
no run of the check has been recorded.

## Build and test

The `esp` toolchain from [`rust-toolchain.toml`](rust-toolchain.toml) and the target from
[`.cargo/config.toml`](.cargo/config.toml) are selected automatically.

```text
cargo build --release --offline
cargo clippy --release --offline -- -D warnings
cargo +stable test --manifest-path host-tests/Cargo.toml \
  --target x86_64-unknown-linux-gnu --locked
cargo +stable clippy --manifest-path host-tests/Cargo.toml \
  --target x86_64-unknown-linux-gnu --locked --all-targets -- -D warnings
```

`cargo run --release` uses the configured `espflash` runner to flash and monitor the board.

Every release build emits one `linker_messages` warning about a LOAD segment with RWX permissions.
It is expected for this target and is not a regression.

The host harness pulls production modules in by `#[path]` rather than copying them, so a module
reachable from `host-tests/src/lib.rs` must keep compiling for std on x86. Adding a host test for
new logic means wiring that module in the same way. Board-only drivers stay out of the harness.
See [`host-tests/README.md`](host-tests/README.md).

The data-cache settings in [`.cargo/config.toml`](.cargo/config.toml) affect drawing and SPI flush
timings. Change them only with a measurement.

`cargo outdated --root-deps-only` runs here and is the drift check. It rebuilds a temporary manifest
with a stable cargo, so any `cargo-features` line in [`Cargo.toml`](Cargo.toml) stops it working.
Weigh that cost before adding one.

## Binary size

Measure the flash image with `espflash save-image`, not the section totals. `xtensa-esp-elf-size`
counts bytes that alignment padding absorbs, and the two disagree by a wide margin on this target.
The image is currently 436,736 bytes, 10.58% of the 4,128,768-byte app partition.

`panic = "immediate-abort"` is the size lever, and it is not taken. It needs
`cargo-features = ["panic-immediate-abort"]` restored to unlock it, which costs the drift check
above. The older route through `build-std-features` no longer exists; the toolchain rejects it and
names the profile setting instead.

It was measured at 43,632 bytes of code and data but only 9,568 bytes of image, 0.23% of the
partition. Most of the saving is in `.rodata`, which the linker pads to a 196,608-byte window
either way. The saving is headroom rather than occupancy: `.rodata` has 31,992 bytes left in that
window and would have 59,936 with the setting, and crossing the window costs a further 65,536 bytes
in one step. Reach for it if the image nears that boundary. It costs every panic message, location
and backtrace, so it is a poor trade before the feature set is complete.

## Measurement code

Code written to take a measurement is kept, not reverted. Commit it to its own `bench/<topic>`
branch, based on a committed revision rather than on uncommitted work, and build it in a temporary
`git worktree` so the main checkout is untouched. Gate it behind a cargo feature that diverts
`async_main`, as `fontdue-target-bench` does, so the branch still builds normally. Report the
branch and the command that reruns it alongside the numbers. Existing ones: `bench/f32-division`,
and `bench/fontdue`, which adds IRAM placement (`fontdue-iram`), a serial glyph dump
(`fontdue-bench-dump`, compared by `tools/compare-glyph-dumps.py`) and a compressed line store
decode bench (`fontdue-line-store`) to the font benchmark; `bench/opt-level`, which times the
draw alone under `timing-log` and builds the firmware at each candidate opt-level;
`bench/fontdue-pin`, which dumps every glyph's metrics and stops each run on a done marker with
`tools/flash-until.sh`; and `bench/gyro-cod`, which measures the gyro offset around the IMU's
on-demand calibration (`gyro-cod-bench`).

## Concurrency

Four parties run concurrently. Core 0 runs the frame loop, the sensor task and the motion task;
core 1 owns the display SPI/DMA path.

- `async_main` on core 0 owns touch and drawing. It reads touch directly, takes the latest sensor
  values from `SENSOR_STATE` and `MOTION_STATE`, draws into its current framebuffer, records
  `dirty`, and hands the state to core 1.
- `sensor_task`, also on core 0, owns the PMIC, RTC, GNSS and LoRa. It publishes a whole
  `SensorSnapshot` through the `SENSOR_STATE` signal.
- `motion_task`, also on core 0, owns the IMU and magnetometer, the compass calibration and the
  sensor fusion. It samples every 250 ms, or every 20 ms while the frame loop sets
  `COMPASS_ACTIVE`, and publishes a `MotionSnapshot` through `MOTION_STATE`. The frame loop
  never touches these devices.
- `second_core` on core 1 waits for display TE with a timeout, flushes the handed-off regions
  through `Co5300Display`, and returns the other framebuffer.

The I2C bus is an `embassy_sync` `Mutex<NoopRawMutex, _>`. Every I2C user must stay on core 0; a
`NoopRawMutex` gives no cross-core exclusion. Moving an I2C device to core 1 needs a different
mutex, not just a different spawn.

The two cores exchange two PSRAM framebuffers through `util::Swap`, which is lock-free and carries
`unsafe impl Send/Sync`. A started `SwapThreadFuture` must be allowed to complete; dropping it
poisons the thread and a later `get()` panics.

## Rendering

- `needs_full_redraw` identifies the regions that must be redrawn in the alternate framebuffer.
  A buffer is not assumed to retain the pixels drawn into the other buffer.
- Partial flushing is active. A full redraw is selected only when the accumulated damage is full.

The display path is split across:

- [`src/drivers/qspi_bus.rs`](src/drivers/qspi_bus.rs), which owns QSPI command transfers.
- [`src/drivers/co5300.rs`](src/drivers/co5300.rs), which initializes the panel, handles TE,
  address windows, brightness, and double-buffered DMA pixel streaming.
- [`src/drivers/framebuffer.rs`](src/drivers/framebuffer.rs), which stores draw-target pixels in
  PSRAM and aligns partial flushes to the controller's pixel granularity.

`chrome::Color` selects the framebuffer and panel colour format. Keep the format consistent through
the QSPI, display, framebuffer, and UI layers.

## Radio

The SX1272 sits on its own SPI bus with `NSS` on GPIO18 and `DIO0` on GPIO44. It is configured in
`async_main` as `Sx127xLoraConfig::for_variant::<Sx1272>()` with `auto_optimize` and `use_crc` on,
which leaves the driver defaults in place: 868 MHz, SF7, 125 kHz, CR 4/5, explicit header, 8-symbol
preamble, sync word `0x12`. Nothing has been tuned for range or airtime yet.

The antenna path is the part that catches people. A transmit and a receive select different RF
switch positions, and both are TCA9554 outputs rather than radio pins, so `LoraPath::transmit` and
`LoraPath::receive` each perform an I2C write before the radio call. Skipping one does not fail
loudly; it transmits or listens through the wrong path. That also couples the radio to the shared
I2C bus and its core-0 restriction, so any timing the protocol depends on includes an I2C
transaction.

What exists today is a link test, not a protocol. The `lora-link-tx` and `lora-link-rx` features
build the two ends inline in `sensor_task`, sending eight bytes of `OWLK` plus a big-endian
sequence number. Transmit waits on `DIO0`; receive polls `RxDone` in a 100-step loop. A two-board
round trip is recorded in `docs/logs/lora/round-trip-2026-09-22.log`, with RSSI around -100 dBm.
The receiver logged every other sequence number there, which is the two loops running free rather
than a link problem.

The protocol that replaces it is designed and written down in
[`context/LORA-PROTOCOL.md`](context/LORA-PROTOCOL.md). Read that before touching the radio. It
settles medium access, packet layout, freshness, crypto and pairing, and it names two firmware
changes it depends on: the radio moves to its own task, and I2C gets a single owning task.

The driver has more than the link test uses: channel activity detection, RSSI and SNR per packet,
frequency error, and a hardware random source. Channel activity detection is the one the protocol
has a planned use for, as a receive-power optimisation once slot timing is tight enough.

Radio hardware findings, including the RF switch requirement, the pin correction and the SX1272
errata workaround, are in [`docs/hardware-notes.md`](docs/hardware-notes.md).

## Memory

- The current internal heap is 260 KiB, allocated by `esp_alloc::heap_allocator!` in `main`.
- PSRAM is registered in the separate `PSRAM_HEAP` static. Framebuffers must be allocated with
  `FB::alloc(&PSRAM_HEAP)` rather than the global allocator.
- The current RGB565 configuration uses 466 × 466 × 2 = 434,312 bytes per framebuffer, with two
  framebuffers.
- Core 1 uses the 8 KiB `CORE1_STACK` static.

Check the allocator and framebuffer definitions in [`src/main.rs`](src/main.rs) and
[`src/chrome.rs`](src/chrome.rs) when changing memory placement.

## Cargo features

All default off. None belongs in normal firmware behavior.

- `damage-debug`, `timing-log` visualize or log dirty regions and frame timings.
- `gnss-raw-log` dumps raw NMEA; `gnss-full-power` skips the low-power GNSS configuration.
- `lora-link-tx` and `lora-link-rx` build the two ends of a link test. They are mutually exclusive
  and `main.rs` refuses both with a `compile_error!`.
- `fontdue-target-bench` diverts `async_main` into the on-target font benchmark, which never
  returns. `fontdue-target-bench-gcc` implies it and only tags the results with a different
  baseline label. Keep benchmark-only paths out of the normal frame loop.

## Fonts and layout

The active UI uses the compile-time fontdue renderer in [`src/chrome.rs`](src/chrome.rs), with the
Marathon Shapiro and PPFraktion font data under `assets/`. `embedded-layout` supplies the current
text alignment helpers. The legacy u8g2 conversion files under `assets/` are not part of the active
renderer.

## Design language

The current UI is a first attempt at the design language and is not the intended look. A proper
design is deferred until the functionality is further along. Treat the screens as a working surface
for exercising real features, not as something to polish. Do not spend effort on visual refinement
unless asked, and do not read the present layout as a decision. Functionality first.

[`context/marathon-ui-cross-project-handoff.md`](context/marathon-ui-cross-project-handoff.md) is
the doctrine those prototypes were designed from. It is project-agnostic and was carried in from an
earlier product, so it describes the visual language and the working method, not this board's
screens. Read it before changing how anything looks. These rules constrain the code directly, and
they hold even while the design is provisional:

- Colour tokens are a single source of truth. They live in [`src/chrome.rs`](src/chrome.rs) as
  `LIME`, `RED`, `ORANGE`, `PURPLE`, `GRAY`, `WHITE`, `BLACK`. Every one is a value from the
  reference board except `BLACK`, which stays pure for panel contrast. Define a new colour there,
  not at the call site, and take its value from
  [`context/palette-reference.md`](context/palette-reference.md) rather than inventing one.
- `RED` means a fault. Do not spend it on a data series, an idle state or a prompt.
- A screen must not imply data, capability or state the system does not have. No invented sensor
  readings, no status word without a condition behind it, no control with no implementation path.
  Synthetic values are for visual exploration and must be recognisable as synthetic. The map
  markers and coordinates in `src/ui/prototypes.rs` are static fixtures, not a position fix.
- Saturated fills carry black knockout text and black symbols. That pairing is the look, so a
  bright slab with white text on it is a departure rather than a variation.
- Symbols are built from primitives on integer geometry, not drawn as bitmaps. A new icon is
  rectangles and lines in code, which keeps it scalable and keeps the framebuffer free of assets.
- A screen that suggests a new capability is a proposal about behaviour, not a visual change. Add
  the capability first, or leave the control out.

`src/ui/prototypes.rs` is the applied result, and
[`context/GRAPHICS-PROTOTYPES.md`](context/GRAPHICS-PROTOTYPES.md) records how it is structured.

## Dependencies and conventions

The root manifest owns dependency versions, features, and git patches.

Two patches are load-bearing. `fontdue` and `fontdue-macros` are forked for the
`fontdue_font_from_file!` compile-time font macro, `FontRepr`, and the `raster` module, none of
which exist upstream. `tca9554` is forked to replace the atomic register masks with a mutex-guarded
cache and a `RawMutex` type parameter. Dropping either patch will not compile.

`lc76g`, `sx127x-lora` and `sx127x-common` are local path crates. `crates/sx127x-lora` publishes
the package name `sx127xlora`, so the manifest key and the directory differ. Check
[`Cargo.toml`](Cargo.toml) before relying on a fork-only API or changing a dependency.

`src/lib.rs` deliberately carries `#![expect(unused)]` while the UI is being built. `PERF:` comments
mark measured or suspected hot spots and open questions; they are context, not a task list.

## Writing conventions

Commit messages are short. Subject is one line, `scope: what changed`, imperative, lowercase after
the prefix, no trailing period. Add a body only when the subject does not say enough, and use it to
say what was wrong, never to restate the diff or list the touched symbols. Read the last few commits
that touched the same area before writing one. Keep the bench out of the history: no board or part
names, no instrument names, no measured figures from a run, no "tested on", no wiring. That belongs
in `docs/` or the working notes. No AI attribution or co-author trailers.

A comment earns its place by saying what the code cannot. Do not restate the adjacent line, the
signature, or the log string. Do not explain why a value is cloned or a lifetime is arranged a
certain way when the compiler rejects every alternative; comment the invariants a reader could
break silently instead, like the ordering the types do not enforce or the call a caller owes
afterwards. Keep it short and plain, with no second sentence rephrasing the first.

Reserve undefined-behaviour vocabulary for undefined behaviour. A sensor reading taken before the
reference settled is unreliable, stale or not yet settled, not "undefined", "invalid" or "garbage".
To a Rust reader "undefined" means memory unsafety. UB vocabulary belongs in `# Safety` sections.

Dependency versions are looked up, not recalled. Use `cargo add` so the current version is resolved
for you, and `cargo info <crate>` to see one first. Choosing an older version is allowed and costs a
written reason: an MSRV, a pin, a transitive requirement, a yanked release, a known regression. Say
which. Silence reads as a claim that the version is current.
