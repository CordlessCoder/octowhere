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

- `src/drivers/` owns the display path: QSPI, the CO5300 panel, and flushing the framebuffer to
  it.
- `crates/octowhere-peripherals/` owns the I2C devices' drivers: touch, power, RTC,
  magnetometer, and their shared register helper. They are generic over `embedded-hal-async`'s
  I2C and build and test on the host; the firmware reaches them as `octowhere::peripherals`,
  and keeps their bring-up and the shared bus. The IMU comes from `ph-qmi8658` rather than a
  local module.
- `crates/octowhere-motion/` owns compass calibration, sensor fusion, and IMU unit conversion.
  It has no board or renderer dependency and builds for the host.
- `crates/octowhere-mesh/` owns the location mesh in `context/LORA-PROTOCOL.md`: the slot
  schedule, the packet's header and records, sealing them with AES-SIV, the table of positions,
  the timebase, the group and its members (`members`), pairing (`pair`), the exchange
  without a radio, messages and the store every node holds them in (`messages`), removing
  a member by moving the group to a new key (`rekey`), and a device's Ed25519 identity, with
  its X25519 key derived from it, which signs its records (`identity`). It has no radio or
  board dependency and builds for the host. `src/mesh.rs` runs it on the radio.
- `crates/octowhere-node/` owns what a node of the mesh shows the screens and takes from
  them (`view`), and the changes to its state it stores (`GroupWrite`). It builds for the host,
  and is where `src/mesh.rs`'s node is moving, behind the seams it now runs on, for the
  simulator of several boards (`context/BACKLOG.md`).
- `crates/octowhere-ui/` owns screen state, drawing and touch handling. It has no board dependency,
  so it also builds for the host. `src/ui/` there owns dirty tracking, geometry,
  gestures and paging, the clock and compass screens with
  the icon and cell reveal they share, the settings panel (`panel`, the grid; `sheet`, its
  travel over the faces; `second`, the screens it opens; `picker`, the zone picker; `text`,
  placing text by its ink), `startup`, the self-test, identity, logo card and fault screen that
  open the firmware, `scatter`, the identity's halftone scatter, kept apart for other screens
  to use, `rest`, the screen timeout, the dim and the fades between levels, `shift`, the pixel
  shift's positions against burn-in and the column split the flush shares, `always_on`, the
  face the screen rests on, `power_off`, the power key's confirmation, `group`, the group,
  name and pairing screens, drawn from a list of what each shows (`layout`), with the mesh's
  published state and the requests they make of it in `view`, from `octowhere-node`, and a
  simulated mesh for the host in `sim`, `stage`, which holds the screen state and turns touch
  and readings into redraws and settings to store, and `script`, which steps a stage on a
  simulated clock for tests and scenes. `src/chrome.rs` is the font and draw-target layer, and
  `src/framebuffer.rs` holds the pixels. The firmware re-exports its `chrome`, `framebuffer`,
  `motion` and `ui` modules. The UI keeps re-exporting the motion modules under `ui::` for
  existing screen and test paths.
- `src/main.rs` holds both cores, the bring-up of the parts behind the self-test, the sensor
  and motion tasks, and the frame loop, which feeds the stage and flushes what it draws.
- `src/board.rs` holds display geometry, the TCA9554 line indices, and the I2C addresses that
  external crates take. GPIO numbers are not there: they live at the binding sites in `main.rs`,
  and `docs/hardware-notes.md` has the pin map.
- `crates/tz/` finds the time zone under a position and converts UTC to local time, from zone
  data built into the binary. `tools/tz-data.py` rebuilds `crates/tz/data/zones.bin` and the
  crate's test vectors from timezone-boundary-builder's boundaries and the IANA rules; its header
  has the command. The boundaries are ODbL, and `crates/tz/data/NOTICE.md` carries the
  attribution the licence asks for. Its default `boundaries` feature, which `octowhere-ui`
  forwards, holds the boundaries and the lookups by position; without it only the tables before
  them are built in, which `tools/ui-web` uses.
- `src/gnss_time.rs` estimates when each UTC second begins on the local timer, from when the
  GNSS module's bursts arrive. It has no board dependency, and `host-tests` tests it.
- `src/settings.rs` keeps settings in flash across restarts, in an ekv database: the time zone
  mode, the manually chosen zone, the zone GNSS last placed the device in, the display's
  brightness, the screen timeout, and whether the screen rests on the always-on face. The
  mesh's state sits beside them: this device's Ed25519 seed, which its keys come from, and name, the end of its block of
  message sequence numbers, and its group's key and its generation, id, members and gone
  members, and removals. Clearing the settings leaves the mesh's state (owner). `partitions.csv` is the flash layout, and the cargo runner flashes it.
- `tools/compass-texture.py` records the design's compass fields into
  `crates/octowhere-ui/src/ui/compass_texture.rs`, from the design's own generator, and checks
  the recording repaints it exactly.
- `tools/tz-references.py` writes each zone's reference point into `crates/tz/src/references.rs`,
  which the picker ranks zones by.
- `crates/` also holds the local `lc76g`, `sx127x-lora` and `sx127x-common` crates.
- `host-tests/` is the std test harness for the board-side modules.
- `tools/ui-sim/` runs the stage in a desktop window. `tools/ui-web/` builds it to
  WebAssembly with a page that runs it in a browser, controls in place of the desktop's keys;
  its `build.sh` writes the static site to `dist/`, `deploy.sh` copies it to a server over SSH,
  and `.github/workflows/ui-web.yml` publishes it to GitHub Pages. `tools/design-compare.py`
  puts screens beside the design's renders. `tools/` also holds the bench scripts.
- `docs/` holds hardware reference: the topology notes, the datasheet pack, and captured GNSS,
  LoRa and compass traces under `docs/logs/`.
- `context/` holds the agent-facing documents below. This file stays at the root.

`context/` carries what is not in the code:

- [`context/BACKLOG.md`](context/BACKLOG.md) lists open work that is not in progress, and says
  where each entry's detail lives. Read it when choosing what to do next.
- [`context/SCREEN-DESIGN-BRIEF.md`](context/SCREEN-DESIGN-BRIEF.md) briefs a design agent on
  a new screen: the hardware, what the renderer draws and what it costs, what data the screens
  receive, and the approved screens. It describes the screens as built, including the
  settings panel. Captures of their states are in `context/screen-captures/`, drawn by the
  `render` example. Keep it current when the renderer, the costs or the screens' data change. The compass's earlier specification and the design
  round's questions and answers were removed once the firmware implemented them; git history
  has them.
- [`context/design/`](context/design/README.md) is the approved design of every screen, from
  the design agent's hand-off of 2026-09-26: the S1 self-test, the G19 identity and G17 card,
  the K1 clock, the C1 compass, the S1 settings overview with the D3 screens it opens, and the
  H2b always-on face, and the later hand-offs that amend it, the 2026-10-02 pairing and group
  screens last. The owner approved all of it; its `DECISIONS.md` records that and settles
  what the hand-offs left open. Only `DECISIONS.md` and `docs/` are in the repository: the
  hand-offs, renders, references and specs, and the update and power-off packages in
  `context/`, are kept locally and ignored by git (owner, 2026-10-02), and the README lists
  them. Start at `handoffs/IMPLEMENTATION-HANDOFF-CURRENT.md`, which names the render each
  state is checked against. The README says how to run the concept renderers from the fonts
  under `assets/`. `specs/` keeps the earlier functional specs with the owner's decisions
  since (clock face, wordmark, compass animation, settings panel, round 3 display and motion,
  round 4 clock); the hand-off overrides them where they conflict, and
  `SCREEN-DESIGN-BRIEF.md` has where the build interpreted them. Only the start-up's
  identity, logo card and fault screen run at 30 fps; everything else keeps timings in ms. The
  design agent's full backup is `context/octowhere-design-project/`, also kept locally.
- [`context/WORKING-NOTES.md`](context/WORKING-NOTES.md) is how the work is done here: how
  the owner reviews, the board's rules and pitfalls, measuring draws, driving `ui-sim`
  without a display, and what is settled. Read it before using the board.
- [`context/HARDWARE-VERIFICATION.md`](context/HARDWARE-VERIFICATION.md) lists open hardware
  questions from static review. They are questions, not confirmed defects.
- [`context/IMPLEMENTATION.md`](context/IMPLEMENTATION.md) is a finished multi-agent brief kept as
  a record. Its partition is historical.
- [`context/LORA-PROTOCOL.md`](context/LORA-PROTOCOL.md) is the agreed design for the location
  mesh of up to 32 nodes: band and radio settings, gossip digest, GPS-anchored TDMA, packet
  layout, messages, crypto and pairing. Steps 1 to 4 of its build order are implemented, and the mesh's side of step 6.
- [`context/palette-reference.md`](context/palette-reference.md) records the colour values from the
  reference board and the role each one plays in `chrome.rs`.

`IMPLEMENTATION.md` tells the reader to keep full-frame flushing until a hardware check passes. It
is a record of what that round was told, not current instruction, and the firmware now flushes
partially. `HARDWARE-VERIFICATION.md` has what has been checked of that since.

## Build and test

The `esp` toolchain from [`rust-toolchain.toml`](rust-toolchain.toml) and the target from
[`.cargo/config.toml`](.cargo/config.toml) are selected automatically.

The host lines below use `+stable`, but nightly is fine on the host (owner, 2026-10-03): a host
crate may require it for a feature stable lacks, such as the allocator API before Rust 1.100.
Move that crate's lines, and those of the crates that build it, to `+nightly` in the change
that needs it, not before.

```text
cargo +stable fmt --all --check
cargo +stable fmt --all --manifest-path host-tests/Cargo.toml --check
cargo +stable fmt --all --manifest-path tools/ui-sim/Cargo.toml --check
cargo +stable fmt --all --manifest-path tools/ui-web/Cargo.toml --check
cargo build --release --offline
cargo clippy --release --offline -- -D warnings
cargo +stable test --manifest-path host-tests/Cargo.toml \
  --target x86_64-unknown-linux-gnu --locked
cargo +stable clippy --manifest-path host-tests/Cargo.toml \
  --target x86_64-unknown-linux-gnu --locked --all-targets -- -D warnings
cargo +stable test --manifest-path crates/octowhere-ui/Cargo.toml \
  --target x86_64-unknown-linux-gnu --locked
cargo +stable clippy --manifest-path crates/octowhere-ui/Cargo.toml \
  --target x86_64-unknown-linux-gnu --locked --all-targets -- -D warnings
cargo +stable test --manifest-path crates/octowhere-motion/Cargo.toml \
  --target x86_64-unknown-linux-gnu --locked
cargo +stable clippy --manifest-path crates/octowhere-motion/Cargo.toml \
  --target x86_64-unknown-linux-gnu --locked --all-targets -- -D warnings
cargo +stable test --manifest-path crates/octowhere-node/Cargo.toml \
  --target x86_64-unknown-linux-gnu --locked
cargo +stable clippy --manifest-path crates/octowhere-node/Cargo.toml \
  --target x86_64-unknown-linux-gnu --locked --all-targets -- -D warnings
cargo +stable test --manifest-path crates/octowhere-mesh/Cargo.toml \
  --target x86_64-unknown-linux-gnu --locked
cargo +stable clippy --manifest-path crates/octowhere-mesh/Cargo.toml \
  --target x86_64-unknown-linux-gnu --locked --all-targets -- -D warnings
cargo +stable test --manifest-path crates/octowhere-peripherals/Cargo.toml \
  --target x86_64-unknown-linux-gnu --locked
cargo +stable clippy --manifest-path crates/octowhere-peripherals/Cargo.toml \
  --target x86_64-unknown-linux-gnu --locked --all-targets -- -D warnings
cargo +stable test --manifest-path crates/tz/Cargo.toml --target x86_64-unknown-linux-gnu
cargo +stable clippy --manifest-path crates/tz/Cargo.toml \
  --target x86_64-unknown-linux-gnu --all-targets -- -D warnings
cargo +stable test --manifest-path crates/tz/Cargo.toml --target x86_64-unknown-linux-gnu \
  --no-default-features
cargo +stable clippy --manifest-path crates/tz/Cargo.toml \
  --target x86_64-unknown-linux-gnu --all-targets --no-default-features -- -D warnings
cargo +stable clippy --release --manifest-path tools/ui-sim/Cargo.toml \
  --target x86_64-unknown-linux-gnu --locked -- -D warnings
cargo +stable clippy --release --manifest-path tools/ui-web/Cargo.toml \
  --target wasm32-unknown-unknown --locked -- -D warnings
```

`--all` takes `cargo fmt` into the local path crates, so the root's line covers every crate
under `crates/`; `host-tests`, `tools/ui-sim` and `tools/ui-web` are outside the firmware's
graph and need their own. Drop `--check` to apply it.

The firmware's clippy run does not reach `crates/octowhere-ui`, because a path dependency is not
a workspace member. Its own clippy line above is what lints it. The stable clippy there is newer
than the `esp` one and flags more.

`cargo run --release`, from the repository root, uses the configured `espflash` runner to flash
the board with `partitions.csv` and decode its log. The firmware logs only through defmt, so the image holds an index per message rather than
its text, and the serial stream needs the ELF to read. `espflash monitor --non-interactive
--log-format defmt --elf <elf>` reads it, and restarts the board as it opens the port. Do not add
`--no-reset`: it has left the board frozen in download mode. `DEFMT_LOG` in
[`.cargo/config.toml`](.cargo/config.toml) sets the level at compile time. It is `info`, which
leaves out the periodic sensor samples and the GNSS start-up trace; build with `DEFMT_LOG=debug`
for them.

Every release build emits one `linker_messages` warning about a LOAD segment with RWX permissions.
It is expected for this target and is not a regression.

The UI runs on the host through `crates/octowhere-ui`. Its tests drive a `Stage` through
`ui::script::Driver` with taps, swipes and readings, and check that a redraw clipped to tiles
matches a full one. `tools/ui-sim` plays scenes written on the same driver, from its
`scenes.rs`, and records them to GIF or MP4 the same every run, which is the way to share an
animation. Its MP4s are 4:4:4 H.264, which keeps the panel's thin coloured lines sharp. 4:2:0
smears them, and doubling the size to keep a colour sample per pixel breaks players' scaling
(owner). Players built on ffmpeg or VLC take 4:4:4, as do Chromium and Firefox on Linux; Safari
and Windows' decoders were not tried. The `render` example writes every screen to PNG, and lays a frame of each state out in one `screen-atlas.png`
headed by the revision that drew it; the tiles' order is fixed in `examples/render/atlas.rs`,
so two revisions' atlases compare tile for tile until a tile is added. `tools/ui-sim` is also
the interactive window. Both render through the firmware's own drawing code, so they show what the
panel will show, but they say nothing about draw time on the target. Commands are in the headers
of `examples/render.rs` and `tools/ui-sim/src/main.rs`. Keep the crate free of board
dependencies: that is what lets the host build it, and its manifest enforces it.

`host-tests` pulls the board-side modules it can test (`util` and `gnss_time`) in by `#[path]`
rather than copying them, so those must keep compiling for std on x86. Board-only
drivers stay out of it. See [`host-tests/README.md`](host-tests/README.md).

The data-cache settings in [`.cargo/config.toml`](.cargo/config.toml) affect drawing and SPI flush
timings. Change them only with a measurement.

`cargo outdated --root-deps-only` runs here and is the drift check. It rebuilds a temporary manifest
with a stable cargo, so any `cargo-features` line in [`Cargo.toml`](Cargo.toml) stops it working.
Weigh that cost before adding one.

## Binary size

Measure the flash image with `espflash save-image`, not the section totals. `xtensa-esp-elf-size`
counts bytes that alignment padding absorbs, and the two disagree by a wide margin on this target.
The image is currently 1,635,968 bytes, 10.44% of the 15,663,104-byte app partition that
`partitions.csv` gives it. Measure with `espflash save-image --chip esp32s3 --flash-size 16mb
--partition-table partitions.csv <elf> <out>`; without those two options it assumes 4 MB of flash
and the default table. The time zone
data is about 390 KB of that, and its boundary tolerance in `tools/tz-data.py` is the lever: the
bench branch `bench/tz-boundary-size` tabulates size against accuracy. PP Fraktion Mono Bold with
all of printable ASCII is about 54 KB; subsetting it to the glyphs the compass uses is the other
lever.

`panic = "immediate-abort"` is the size lever, and it is not taken. It needs
`cargo-features = ["panic-immediate-abort"]` restored to unlock it, which costs the drift check
above. The older route through `build-std-features` no longer exists; the toolchain rejects it and
names the profile setting instead.

It was measured at 43,632 bytes of code and data but only 9,568 bytes of image, 0.23% of the
partition. Most of the saving is in `.rodata`, which the linker pads to a 196,608-byte window
either way. The saving is headroom rather than occupancy: `.rodata` has 31,992 bytes left in that
window and would have 59,936 with the setting, and crossing the window costs a further 65,536 bytes
in one step. Those figures predate the time zone data, which added about 390 KB to `.rodata`, so
remeasure before relying on them. It costs every panic message, location
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
`tools/flash-until.sh`; `bench/gyro-cod`, which measures the gyro offset around the IMU's
on-demand calibration (`gyro-cod-bench`); `bench/compass-draw`, the compass screen's draw timed
per part before the coverage-row rewrite, with the antialiased ring variants and fontdue's share
of the text (`compass-draw-bench`); and `bench/compass-draw-rows`, the same bench on the
rewritten draw path, with pixel checks of the precomputed ring and the turned ticks, the clear
variants and the text split (`compass-draw-bench`); and `bench/compass-states`, the implemented
compass design's full draw in each state, entering, and early and halfway through a swipe
(`compass-state-bench`); and `bench/compass-c1`, which measures C1's full states and entry
steps, a swipe and one-degree redraw with core 1 flushing (`compass-c1-bench`); and
`bench/font-scale`, the same draw with a warm and a cold glyph
cache, and `tools/font-scale-sweep.sh`, which reruns it at each fontdue `scale`;
`bench/glyph-cache`, which counted the removed glyph cache's contents on the host
(`examples/glyph_cache.rs`) and the heap on the board (`glyph-cache-bench`); and
`bench/no-glyph-cache`, both benches on the uncached draw path; and `bench/touch-cover`, which
logs every touch report and polls during a cover (`touch-report-log`); and `bench/zone-lookup`,
which replaces the GNSS position with a tour of synthetic fixes and logs each zone lookup's
steps and time, each settings write, and how long it held core 1 (`zone-lookup-bench`); and
`bench/tz-boundary-size`, a host script that simplifies timezone-boundary-builder's zone
polygons at several tolerances, sizes their encoding and checks every GeoNames city of over
15,000 people against the full set (`tools/tz-boundary-size.py`, usage in its header); and `bench/face-draw`, which starts on the clock
face with a fixed zone, reruns the clock's and the compass's entries in turn with a synthetic
heading, and logs every draw's time and area (`face-draw-bench`); and `bench/tearing`, which drags a
synthetic finger between the faces and logs TE's period and pulse and each flush's wait and
length (`tearing-bench`), with TE moved to a scan line by `tearing-scanline` (`TE_LINE` at build
time), and splits each flush into copying and transfer time, times one transfer alone and logs
core 0's step and draw; `flush-spin` spins on each chunk and `flush-chunk` takes larger chunk
buffers from the heap (`FLUSH_DESCRIPTORS` at build time); and `bench/psram-dma`, built on it,
which sends full flushes by DMA straight from the PSRAM framebuffer (`flush-psram-dma`, chunk
size `DIRECT_CHUNK`) at a build-time SPI clock (`SPI_MHZ`), PSRAM block size (`EXT_BURST`) and
transmit FIFO depth (`OUT_FIFO`), optionally with core 0 no longer drawing (`idle-core0`); and
`bench/clock-draw`, which alternates the clock face (20 s, the RTC's time in a fixed zone) and
the compass (3 s), and logs every draw's time and area with the clock face's time per part,
from an optional timer in `octowhere_ui::part_timing`, and the internal heap's use and peak every 5 s (`clock-draw-bench`); and
`bench/fault-draw`, which plays the start-up's fault demonstration for each part in turn and
logs each fault frame's draw per part, and at start-up times the knocked-out row write and
outlined text alone (`fault-draw-bench`); and `bench/scatter`, which replays the start-up
and logs each identity and card frame's draw, the identity's per part, its damage and the step,
and at start-up times the scatter, its arithmetic's primitives and small fills alone
(`scatter-bench`); and `bench/charge-fault-draw`, which replays a demonstrated failure every
30 s and switches a synthetic 87 % battery's charging every 4 s on the clock face, and logs every
draw with the start-up frame it drew (`charge-fault-bench`, summarised by
`tools/charge-fault-summary.py`); and `bench/fill-rect`, which alternates the clock face and the
compass and logs every draw with the count and time of its solid fills (`fill-rect-bench`,
`fill-rect-scene` for the draw times alone, `fill-rect-off` with the framebuffer's fill skipped,
summarised by `tools/fill-rect-summary.py`; `round-scene` instead replays the start-up and
drags the settings panel between its pages, with `round-restore` putting back the removed circular
clip, summarised by `tools/round-summary.py`), and on the host groups every fill that reaches the
framebuffer by the drawing that asked for it (`tests/fill_sources.rs` in `crates/octowhere-ui`);
and `bench/startup-v11`, which replays the V11 start-up for ever and logs each identity and card
frame's draw by part and the internal heap in use (`startup-bench`, summarised by
`tools/startup-summary.py`); and `bench/touch-latency`, which times each touch read from the
controller's INT edge, caught by a pulse counter, through the read, the step, the draw, core 1's
wait for TE and the flush that show it (`touch-latency-bench`, summarised by
`tools/touch-latency-summary.py`), can replace the controller's reports with a scripted finger
(`touch-latency-synthetic`) and put back the old blocking handoff (`touch-latency-fifo`),
runs the finger on both handoffs with `tools/touch-latency-sweep.sh`, logs every read as it is
made, with the report's bytes (`touch-read-log`), times each lift from its report to the step that counts it, and steps the controller through its modes on each short BOOT press (`touch-mode-probe`); and `bench/compass-field`, which holds the compass on
its settled fields with a turning synthetic heading and logs every draw's time
(`compass-field-bench`, summarised by `tools/compass-field-summary.py`); and
`bench/flush-shift`, which times flushes on core 1 with no wait for TE, whole and in regions,
at each pixel shift position, beside the region flush from before pixel shift
(`flush-shift-bench`, summarised by `tools/flush-shift-summary.py`); and `bench/settings-save`,
which saves a key nothing reads every 3 s and logs each save's flash operations and how long
it held the display core (`settings-save-bench`, summarised by `tools/settings-save-summary.py`); and
`bench/startup-handover`, which replays the start-up 6 s after each handover, every other time
with a part failing, with a synthetic battery whose charging flips every two rounds, and logs
each start-up and fault frame's and the clock entry's step and draw (`startup-handover-bench`,
summarised by `tools/startup-handover-summary.py`; results in
`docs/logs/display/startup-handover-2026-10-01.md` and, before and after the heap work with the
before build on `bench/startup-heap-before`, `docs/logs/display/startup-heap-2026-10-03/`). And
`bench/gnss-stuck` holds the GNSS module in reset twice and switches its NMEA output off, for
`gnss_task`'s recovery to clear (`gnss-stuck-bench`). And `bench/mesh-pa` sends every round,
cycling the transmitter through both PA pins at two powers each, and logs what each node hears
(`mesh-pa-bench`; results in `docs/logs/lora/pa-2026-10-01/`). And `bench/mesh-crc`
sends in every other slot and logs every packet's bytes, sent and received, with what each
board was doing (`mesh-crc-bench`); `mesh-monitor` adds an RSSI watch of the channel and
`mesh-low-power` sends at +2 dBm. `tools/mesh-crc-compare.py` and `tools/mesh-burst-compare.py`
read two nodes' logs; results in `docs/logs/lora/crc-2026-10-02/`. And `bench/jtag-read` adds
`tools/jtag-read`, a host tool on the owner's probe-rs fork that times reading a framebuffer
over the USB JTAG with the core running and halted; the result is in `context/BACKLOG.md`. And
`bench/stack-watermark` paints core 0's stack at boot and logs the deepest it has been used
every 15 s (`stack-watermark-bench`, with the inject features to drive the boards); results
under "Memory". And `bench/ui-allocations` counts the stage's heap requests on the host, the
largest, the sizes asked most and the most held at once, through every start-up, on the faces
and through the settings panel and the screens it opens, counting only the stage's own steps
and draws (`crates/octowhere-ui/tests/heap_requests.rs`); results in `context/BACKLOG.md`.

## Concurrency

Core 0 runs two executors. The thread-mode one runs `async_main`, the frame loop and the tasks
that never use the I2C bus. `BUS_EXECUTOR` runs every task that does, from the `FROM_CPU_INTR2`
software interrupt at level 1, so they preempt thread mode instead of waiting for it to yield.
Core 1 owns the display SPI/DMA path.

- `async_main`, in thread mode, loads the settings, starts core 1, then runs `bring_up` and
  `frame_loop` together. `bring_up` brings each part up against its deadline, reports when
  each check starts and how it ends to the start-up's self-test through `BOOT_REPORTS`, the
  radio's last, then spawns the tasks below with the parts that answered, the bus tasks through
  `start_bus_tasks`. A part that fails is left
  out, and its owner runs without it; there is no motion task without the IMU. `frame_loop`
  owns drawing. It takes touch reads from `TOUCH_READS`, asking for one through `TOUCH_POLL`
  while a contact is held, and the latest sensor values from `SENSOR_STATE` and
  `MOTION_STATE`, draws into its current framebuffer, records `dirty` and the display level to
  set, and hands the state to core 1.
- `sensor_task`, on `BUS_EXECUTOR`, owns the PMIC and RTC, and takes the GNSS state from
  `GNSS_STATE` and the zone from `ZONE_STATE`. It publishes a whole `SensorSnapshot` through the
  `SENSOR_STATE` signal every 250 ms, and passes each fix to `zone_task` through `ZONE_FIX`. It
  passes the PMIC's power key presses to the frame loop through `KEY_PRESSES`. Once
  `GNSS_PARKED` is set, it waits for `SETTINGS_DONE` to reach `SETTINGS_QUEUED` and has the PMIC
  power the board off. Every settings write goes through `queue_write`, which keeps that count.
- `gnss_task`, on `BUS_EXECUTOR`, owns the GNSS module. The module sends a burst of NMEA each
  second, and the task reads it only from 100 ms before the burst is due until it has drained,
  leaving the bus alone between. It publishes the parsed state through `GNSS_STATE` and times
  UTC on the local timer from each fix's burst (`gnss_time`, read with `gps_utc`). When the
  frame loop sets `POWER_OFF`, once the panel is off, it saves the module's navigation data and
  sets `GNSS_PARKED`. After 8 failed reads in a row, or 10 s of reads with nothing in them, it
  resets the module through the I/O expander, at most once a minute, configures it again and
  sends it the RTC's time.
- `motion_task`, on `BUS_EXECUTOR`, owns the IMU and magnetometer, the compass calibration and
  the sensor fusion. It samples every 250 ms, or every 20 ms while the frame loop sets
  `COMPASS_ACTIVE`, and publishes a `MotionSnapshot` through `MOTION_STATE`.
- `touch_task`, on `BUS_EXECUTOR`, owns the touch controller. It reads it on each falling edge
  of the controller's INT, every 10 ms while a finger is down, or when the frame loop asks, and
  queues each read in `TOUCH_READS` without waiting for it to be taken. A newer contact replaces
  one the frame loop has not taken, a stale read never displaces a report, and a lift or cover
  is kept ahead of the next report. While the stage rests on the always-on face or dark, the
  frame loop asks through `TOUCH_WAKE_GESTURES` for the controller's gesture mode, in which it
  reports only the gestures it recognises; a double tap wakes the screen, and a reset takes the
  controller out of it again.
- `radio_task`, on `BUS_EXECUTOR`, owns the LoRa radio, its `DIO0` line and the RF switch. It
  runs the link test when a `lora-link-*` feature is on, and otherwise the mesh (`src/mesh.rs`),
  which takes the latest fix from `mesh::FIX`, set by `gnss_task`, the RTC's time from
  `mesh::RTC_TIME`, set by `sensor_task`, and GPS time from `GPS_TIME`. It is spawned only when
  the radio answered at boot; otherwise `mesh::offline`, in thread mode, keeps the name and can
  leave the group, and tells the screens there is no radio. Either publishes what the screens
  show of the mesh, which the frame loop takes on `mesh::VIEW_CHANGED`, and the frame loop
  passes the stage's requests to `mesh::COMMANDS`.
- `zone_task`, in thread mode, looks the zone up again whenever a fix moves about a kilometre
  in automatic mode, a zone at a time with a yield between, takes the settings panel's choice
  from `ZONE_CHOICE`, publishes the zone through `ZONE_STATE`, and queues a new zone for
  `settings_task`. It stays out of `BUS_EXECUTOR` because a yield there polls the task again at
  once, which would hold off the frame loop for the whole lookup.
- `settings_task`, in thread mode, owns the flash and saves what `SETTINGS_WRITES` and the
  mesh's `GROUP_WRITES` queue.
- `boot_key_task`, in thread mode, owns GPIO0 and passes the BOOT key's short and long presses
  to the frame loop through `BOOT_KEY_PRESSES`. The stage takes them as `Input::boot_key` and
  does nothing with them yet.
- `second_core` on core 1 waits for display TE with a timeout, flushes the handed-off regions
  through `Co5300Display`, sets the display level a frame carries before flushing it, and
  returns the other framebuffer. It moves the picture by the frame's pixel shift as it copies
  each row into its DMA buffers, repeating the framebuffer's edge past it, and flushes in full
  when a frame's shift differs from the last one sent; an unshifted full flush keeps the
  straight copy. A frame can also switch the panel out of sleep before it goes
  out, or into sleep after. The panel comes up dark. TE pulses when the panel's scan
  reaches `TE_LINE` in `src/drivers/co5300.rs`, so a flush runs behind the scan. A full flush
  takes about as long as the scan, so moving the line, or waiting for TE's level instead of its
  edge, brings back tearing.

A flash write stops the cache both cores run from, so core 1 must not touch flash while one
runs. Reads through `esp-storage` may not need it, but the settings store holds core 1 for them
too. `settings::with_display_core_held` asks core 1 to wait, and core 1's loop calls
`settings::hold_display_core_if_asked` before each frame, which spins in IRAM with its
interrupts masked until core 0 is done. The request is answered only at core 1's next frame, so
it must never come from the frame loop, which would then stop handing frames over. Anything else
that touches flash at run time goes through the same pair. Loading at boot happens before core 1
starts.

The I2C bus is an `embassy_sync` `Mutex<CriticalSectionRawMutex, _>`, shared through
`I2cDevice` clones. After `bring_up` spawns the bus tasks, nothing in thread mode may use it. The
mutex keeps one waiter's waker, and registering a second wakes the first, so two waiters on
`BUS_EXECUTOR` wake each other in turn at interrupt level. A thread-mode holder then never runs
to release the bus, and core 0 stops; it did, within seconds, with only the GNSS and radio tasks
there. The bus went async on core 0, which binds its interrupt there. A transaction holds the
bus for its length, and a 512-byte GNSS read takes about 12 ms at 400 kHz. The lock has no
priority; the owner chose a plain mutex over a bus-owning task, and a priority-aware mutex is the
route if the radio needs one. The bus has esp-hal's software timeout, per byte. A device can end
a read early, and without a deadline esp-hal then yields for ever waiting for commands that never
run; a yield on `BUS_EXECUTOR` runs again at once, so core 0 stops. The touch controller did, in
its normal and low-power scan modes.

Every lock esp-hal and esp-rtos take raises the interrupt level to 5, so `BUS_EXECUTOR` never runs
inside one. esp-hal saves the FPU registers across interrupts (`float-save-restore`), so the bus
tasks may use floats. esp-hal's async drivers are not `Send`, since each binds its interrupt to
the core that made it, so `start_bus_tasks` takes them in `OnCore0`, which asserts they stay on
core 0.

The two cores exchange two PSRAM framebuffers through `util::Swap`, which is lock-free and carries
`unsafe impl Send/Sync`. A started `SwapThreadFuture` must be allowed to complete; dropping it
poisons the thread and a later `get()` panics.

## Rendering

- Damage is `chrome::Dirty`, spans of columns per pair of rows at the panel's 2 × 2 write grain
  (`ui/dirty.rs`). `Stage::changed` holds what a step changed: on a settled face, the old and
  new places of each part that changed; on the settled panel, the cells that changed or the
  scrolling grid; on a group screen, the items of its list that differ from the last step's;
  elsewhere, the whole panel.
- Each framebuffer repaints the previous step's damage and its own, since it last held the frame
  before that, drawing through `chrome::Clip`. The flush sends only the step's own damage, since
  the panel already shows the step before. A buffer not yet drawn is drawn in full. The spans are
  a couple of kilobytes each, so they live on the heap rather than pass through the frame loop's
  stack.
- `Clip` makes every draw land only on damaged pixels. `CoverageTarget::visible` lets a drawing
  skip work outside them, which is what makes a partial redraw cheap: glyphs, ticks and whole
  text lines outside the damage are never rasterized. It is a hint, so a drawing must still clip.
  Laying text out is not free either, since the font and zone tables sit in flash behind the
  data cache the framebuffers keep evicting. So the clock face checks each part against a fixed
  region (the `*_INK` constants in `clock_screen.rs`) before laying it out, and
  `each_line_stays_in_its_region` checks every character each line can show against its region.
  `ClockView` works out the local time once per reading for the same reason.
- Every draw target the screens use is a `chrome::CoverageTarget`, which takes antialiased
  coverage a row at a time and blends it with what is underneath. `chrome::Window` shifts and
  clips once per row, and stands in for embedded-graphics' `translated` and `clipped`, which go
  per pixel. `chrome::OnBackground` names a known background so edges skip the read-back. Nothing
  checks that promise.
- `chrome::Knockout` paints text over a solid background across a run of rows, writing each
  pixel once instead of filling and then blending; the fault screen's band and strip use it.
  It paints each row as it arrives and blends only where a glyph reaches back over pixels
  already painted, so it holds no coverage. `chrome::Recording` keeps coverage drawn into it as
  runs, to blend later in any colour; the identity's title is one, a part a glyph.
  `FontdueRenderer::draw_outline_on_baseline` draws text's outline from its coverage grown by
  a radius; no screen uses it yet.
- `screens::render` clears the round panel's visible circle and the 3 px past it that the
  pixel shift can bring on, in runs of rows that may reach `CLEAR_SLACK` columns further so
  that each run is one fill; below `Clip`, one fill a row
  made most of a partial redraw's calls. The rest of the square's corners are never cleared
  or seen. The clear also leaves the settled compass's slab and the clock's band
  interior, wherever its page is, because those screens paint them solid.
  `a_frame_replaces_everything_under_it` draws over an old frame to catch a hole left unpainted.

The display path is split across:

- [`src/drivers/qspi_bus.rs`](src/drivers/qspi_bus.rs), which owns QSPI command transfers.
- [`src/drivers/co5300.rs`](src/drivers/co5300.rs), which initializes the panel, handles TE,
  address windows, brightness, and double-buffered DMA pixel streaming.
- [`crates/octowhere-ui/src/framebuffer.rs`](crates/octowhere-ui/src/framebuffer.rs), which
  stores draw-target pixels. The firmware allocates it in PSRAM.
- [`src/drivers/framebuffer.rs`](src/drivers/framebuffer.rs), whose `Flush` trait streams the
  framebuffer to the panel and aligns partial flushes to the controller's pixel granularity.

`chrome::Color` selects the framebuffer and panel colour format. Keep the format consistent through
the QSPI, display, framebuffer, and UI layers.

## Radio

The SX1272 sits on its own SPI bus with `NSS` on GPIO18 and `DIO0` on GPIO44. It is configured in
`async_main` as `Sx127xLoraConfig::for_variant::<Sx1272>()` with `auto_optimize` and `use_crc` on,
which leaves the driver defaults in place: 868 MHz, SF7, 125 kHz, CR 4/5, explicit header, 8-symbol
preamble, sync word `0x12`. The mesh then tunes it to band O's 869.4625 MHz and +17 dBm on
`PA_BOOST`; the link test keeps 868 MHz and the reset power, +14 dBm on `RFO`.
The driver is built with its `half_duplex` feature, which gives a packet the radio's whole
256-byte FIFO. Without it the FIFO is split between transmit and receive and the driver refuses
any packet over 128 bytes, in either direction; a pairing's group transfer met it first.

The antenna path is the part that catches people. A transmit and a receive select different RF
switch positions, and both are TCA9554 outputs rather than radio pins, so `LoraPath::transmit` and
`LoraPath::receive` each perform an I2C write before the radio call. Skipping one does not fail
loudly; it transmits or listens through the wrong path. That also couples the radio to the shared
I2C bus, so any timing the protocol depends on includes an I2C transaction and waiting for the bus.

`radio_task` runs the mesh, `src/mesh.rs` on `crates/octowhere-mesh`: slots, in an order the
group's key shuffles each round, carrying neighbours, a digest of the member table, the member
and gone records asked for or changed, a digest of the messages held, a summary of them when
a neighbour's differs, messages, and positions, with a timebase taken from other nodes without
a fix, under the group key pairing gave the node. Member, gone and key records carry their
device's Ed25519 signature, which a node checks before taking them (the protocol's
"Signatures"); a check takes about 32 ms on the board, and a signature about 35. A node in no group sends nothing and keeps the
radio asleep. Without a fix a node has no position of its own. Commands reach the mesh through
`mesh::COMMANDS`: start a pairing to add or join, choose a device found, answer the code, cancel,
leave the group, rename, refresh, send text, remove a member, keep one another member removes.
A refresh listens throughout for three rounds and keeps sending; a pairing stops it. A device
that leaves sends its gone record in its next two slots before it forgets the key. Every node
holds every message for 24 hours in a store in PSRAM, lost at a restart, and the summaries
bring back what a neighbour lacks. A removal sends the new key to each remaining member, and
the group switches to it at the round the key names; a node keeps the old key for any member
not yet heard on the new one, and in a sweep round sends that member its key message under the
old key. There are no screens for messages or removals yet: they wait for a design round. A pairing takes the radio to band O's upper channel at +2 dBm until it
ends; the protocol's "The exchange as built" has the frames and their order, and
`docs/logs/lora/pairing-2026-10-02/` the first pairings between the two boards. The group
screens send the commands, and `pair-inject` lets `tools/pair-inject.py` send them over the USB
JTAG too; `docs/logs/lora/pairing-screens-2026-10-02/` has a pairing through the screens. The
mesh asks `settings_task` to store the group through `GROUP_WRITES` and waits for
`GROUP_SAVED` where a pairing's commit depends on it. Keys and nonces come from the hardware's
true random source, which `async_main` enables with the ADC's noise and leaves on. A board whose `DIO0` rises with no flag raised has its
flags polled instead, 1 ms apart; one board did until a joint was reworked
(`docs/hardware-notes.md`).

The `lora-link-tx` and `lora-link-rx` features build the older link test in `radio_task` instead,
sending eight bytes of `OWLK` plus a big-endian sequence number every 250 ms. Transmit waits on `DIO0`'s edge. Receive stays in continuous receive and waits
on `DIO0`'s level, which holds until `RxDone` is cleared. A two-board round trip is recorded in
`docs/logs/lora/round-trip-2026-09-22.log`, with RSSI around -100 dBm: it sent on `RFO`, which
the antenna is not on. The receiver logged every
other sequence number there. Both ends then ran inside the 250 ms sensor loop, and the receiver
listened only in part of it. Since the move the transmit end has run alone, and the two-board
test has not been rerun.

The protocol is designed and written down in
[`context/LORA-PROTOCOL.md`](context/LORA-PROTOCOL.md). Read that before touching the radio. It
settles medium access, packet layout, freshness, crypto and pairing. Its "Firmware structure"
section says what the firmware changed for it.

The driver has more than the link test uses: channel activity detection, RSSI and SNR per packet,
frequency error, and a hardware random source. The protocol needs channel activity detection to
keep receive power down at 32 nodes, once slot timing is good to milliseconds.

Radio hardware findings, including the RF switch requirement, the pin correction and the SX1272
errata workaround, are in [`docs/hardware-notes.md`](docs/hardware-notes.md).

## Memory

- The internal heap is 192 KiB, from two `esp_alloc::heap_allocator!` calls in `main`: 72 KiB
  in the RAM the second-stage bootloader frees after boot (`#[esp_hal::ram(reclaimed)]`), and
  120 KiB as a static in `.bss`. Core 0's stack is whatever DRAM `.data` and `.bss` leave,
  119,556 bytes at `da41826`; `_stack_end_cpu0` and `_stack_start_cpu0` in the ELF give it
  exactly. The tasks' futures are statics, so the stack shrinks as they grow. It was about
  19 KiB with the whole heap in `.bss`, and the clock face overflowed it. The mesh's work took
  it to about 60 KB, and the radio task overflowed it from under a group screen. Tasks on
  `BUS_EXECUTOR` run on the stack thread mode left, so their depth adds to the frame loop's.
  With step 6 of the mesh (2026-10-03) the radio task's poll takes an 18,768-byte frame,
  `Mesh::run` 9,488 and `Mesh::pair` 11,008, about 45 KB on its deepest path, against about
  30 KB before. Painted at boot, the stack's deepest use was 45,336 bytes of 111,212 after the
  start-up, 66,544 on the device adding in a pairing with its group screens drawn and
  messages sent, and 72,076 on the device joining, 65% (`bench/stack-watermark`). Signed
  records then doubled a group's size, to about 5 KB, and every frame that held or moved one
  grew with it, until a pairing overflowed the stack into the tasks' state. A group's records
  and a pairing's welcome now live on the heap: the radio task's poll takes 11,808 bytes,
  `Mesh::pair` 6,448 and `Mesh::run` 2,608, and the largest frames left are leaves,
  `Group::restore` 6,000, Ed25519's `verify` 5,392 and `Group::clone` 5,136. The watermark
  figures predate signing; remeasure before relying on them.
  Each function's frame is the `entry a1, N` that opens it in `xtensa-esp-elf-objdump -d`, in
  hex once it is large. The dump names code with no symbol of its own after the symbol before
  it, so a large frame can carry an unlikely name.
  The mesh's view and the group screens' list are each kilobytes, so they are filled where they
  live on the heap rather than built on the stack. An overflow is caught by esp-hal's stack
  guard as a panic, or corrupts memory silently. The heap peaked at about 50 KB over the clock
  and the compass (`bench/clock-draw`); 18 KB is in use after boot. `context/BACKLOG.md` has
  the plan for laying SRAM out deliberately.
- PSRAM is registered in the separate `PSRAM_HEAP` static. Framebuffers must be allocated with
  `FB::alloc(&PSRAM_HEAP)` rather than the global allocator. The mesh's message store, about
  49 KB, and the key messages it keeps for catch-up, about 25 KB, are made in place there by
  `zeroed_in_psram` in `src/mesh.rs`, from types whose all-zero value is valid (`Zeroable`).
  Never add PSRAM to the global allocator, even as a fallback after the internal regions: a value
  holding an atomic could land there, and atomics in PSRAM break (owner).
- esp-alloc serves the internal heap's two regions first fit, the 72 KiB one first, and grows a
  block by allocating the new one and copying, so a large request needs that much contiguous
  room in one region. The start-up's identity held two 45,828-byte buffers for its title, and
  with them a raster that still had to grow left no room on 2026-10-03, which panicked both
  boards at boot. fontdue's glyph raster is now made at its largest right after the heap comes up
  (`chrome::raster_buffer`, `Stage::use_raster`) and never grows, and the title is a
  `chrome::Recording` of 36,660 bytes, reserved whole (`TITLE_BYTES` in `ui/identity.rs`).
  The compass dial's ticks borrow the same raster (`FontdueRenderer::with_raster`). Measured on
  the host by `bench/ui-allocations`, the start-up holds at most 61,194 bytes above the raster
  and a fault screen under 1 KB, and the faces and the settings panel's pages make no requests
  as they step and draw; the group screens hold two 8,848-byte lists while open.
- The current RGB565 configuration uses 466 × 466 × 2 = 434,312 bytes per framebuffer, with two
  framebuffers.
- Core 1 uses the 8 KiB `CORE1_STACK` static.
- Settings are an ekv database in the `storage` partition, 1 MiB at `0xF00000`, 256 pages of
  4 KiB. The firmware formats it when it finds no database. `espflash erase-region 0xF00000
  0x100000` clears it. The 24 KiB `nvs` partition at `0x9000` held settings before and is unused.

Check the allocator and framebuffer definitions in [`src/main.rs`](src/main.rs) and
[`crates/octowhere-ui/src/chrome.rs`](crates/octowhere-ui/src/chrome.rs) when changing memory
placement.

## Cargo features

All default off. None belongs in normal firmware behavior.

- `damage-debug`, `timing-log` visualize or log dirty regions and frame timings.
- `gnss-raw-log` dumps raw NMEA; `gnss-full-power` skips the low-power GNSS configuration.
- `rtc-inject` lets `tools/rtc-inject.py` set the RTC over the USB JTAG with probe-rs, either as
  though GNSS set it or as the RTC's own unconfirmed time, without a reset. After one, GNSS no
  longer sets the clock until the firmware restarts.
- `pair-inject` lets `tools/pair-inject.py` give the mesh its commands over the USB JTAG,
  beside the screens: add, join, choose, accept, decline, mismatch, cancel, leave, rename,
  refresh, send, remove and keep. Its `deaf` makes a pairing or the mesh drop what it hears for
  a while, to lose a frame or a switch on purpose, and its `phantom` enrols a member no device
  stands behind, so that two boards can try a removal with a member left to tell.
- `touch-inject` lets `tools/touch-inject.py` tap, swipe and cover the screen and press the
  power key over the USB JTAG, and read the framebuffer drawn last back to a PNG, for driving
  the screens on a board nobody holds. A read takes about 11 s and interrupts the board.
- `lora-link-tx` and `lora-link-rx` build the two ends of a link test. They are mutually exclusive
  and `main.rs` refuses both with a `compile_error!`.
- `fontdue-target-bench` diverts `async_main` into the on-target font benchmark, which never
  returns. `fontdue-target-bench-gcc` implies it and only tags the results with a different
  baseline label. Keep benchmark-only paths out of the normal frame loop.

## Fonts and layout

The active UI uses the compile-time fontdue renderer in
[`crates/octowhere-ui/src/chrome.rs`](crates/octowhere-ui/src/chrome.rs), with the Marathon Shapiro
and PPFraktion font data under `assets/`. KH Interference Bold sets the large readings, the clock's
label, the compass caption and the settings' row names and selected values; its asset is a trial,
and a release needs a licensed one. KH Interference Regular sets the identity's subtitle. Fraktion
Sans Light sets the clock's band lines, the offset picker's lower neighbour, the always-on face's
battery value and the group screens' explanations. Fraktion Mono Regular sets names, addresses
and the name keyboard, so that a name keeps its case. Maratype sets the identity's title and nothing else: the owner rejected it on every
other screen. `embedded-layout` supplies the current text alignment helpers.

Every font is built from its full font file, and the macro's `chars:` list picks the glyphs it
keeps (owner). Do not add a pre-subset file under `assets/`; widen `chars:` for a new
character instead. A character outside the list draws as the font's missing glyph.

## Design language

The firmware has two faces, the clock and the compass, and the settings panel over them, and all
follow the approved design in `context/design/`, listed above. Change how any of them looks or
moves only against it or a new design round. A new screen starts
from a design round rather than from a sketch in code.

[`context/design/docs/marathon-ui-design-language.md`](context/design/docs/marathon-ui-design-language.md)
is the doctrine the screens were designed from, and
[`context/design/docs/OCTOWHERE-COLOR-ROLES.md`](context/design/docs/OCTOWHERE-COLOR-ROLES.md)
says what each colour means here. The doctrine is project-agnostic and was carried in from an
earlier product, so it describes the visual language and the working method, not this board's
screens. Read it before changing how anything looks. These rules constrain the code directly:

- Colour tokens are a single source of truth. They live in
  [`crates/octowhere-ui/src/chrome.rs`](crates/octowhere-ui/src/chrome.rs) as
  `LIME`, `RED`, `ORANGE`, `PURPLE`, `BLUE`, `VIOLET`, `GRAY`, `WHITE`, `BLACK`. Every one is a value from the
  reference board except `BLACK`, which stays pure for panel contrast. Define a new colour there,
  not at the call site, and take its value from
  [`context/palette-reference.md`](context/palette-reference.md) rather than inventing one.
- `RED` means a fault. Do not spend it on a data series, an idle state or a prompt.
- A screen must not imply data, capability or state the system does not have. No invented sensor
  readings, no status word without a condition behind it, no control with no implementation path.
  Synthetic values are for visual exploration and must be recognisable as synthetic.
- Saturated fills carry black knockout text and black symbols. That pairing is the look, so a
  bright slab with white text on it is a departure rather than a variation.
- Symbols are built from primitives on integer geometry, not drawn as bitmaps. A new icon is
  rectangles and lines in code, which keeps it scalable and keeps the framebuffer free of assets.
- A screen that suggests a new capability is a proposal about behaviour, not a visual change. Add
  the capability first, or leave the control out.

`crates/octowhere-ui/src/ui/screens.rs` holds the ring of screens and draws a frame of them.
Each screen's drawing and damage live in its own module, `clock_screen.rs` and
`compass_screen.rs`.

## Dependencies and conventions

The root manifest owns the firmware's dependency versions, features, and git patches. Each local
crate owns its own, and the host crates keep their own lockfiles.

Two forks are load-bearing. `fontdue` and `fontdue-macros` are forked for the
`fontdue_font_from_file!` compile-time font macro, `FontRepr`, and the `raster` module, none of
which exist upstream. They are git dependencies of `crates/octowhere-ui`, whose manifest holds the
pin, and the firmware reaches them as `octowhere::fontdue`. `tca9554` is forked to replace the
atomic register masks with a mutex-guarded cache and a `RawMutex` type parameter, and is a patch
in the root manifest. Dropping either will not compile.

`octowhere-ui`, `octowhere-tz`, `octowhere-motion`, `octowhere-mesh`, `octowhere-node`,
`octowhere-peripherals`,
`lc76g`, `sx127x-lora` and `sx127x-common` are local path crates. `octowhere-tz` lives in
`crates/tz`, and the firmware reaches it as `octowhere::tz`.
`crates/sx127x-lora` publishes the package name `sx127xlora`, so the manifest key and the directory
differ. Check [`Cargo.toml`](Cargo.toml) before relying on a fork-only API or changing a dependency.

The ESP32-S3's FPU is single precision, so `f64` arithmetic is emulated in software. Code
that runs on the board uses `f32` and libm's `f` functions, even where it ports a design
script written in doubles; a test against the script's output checks the result instead
(owner). The compass calibration fit in `octowhere-motion` keeps `f64` on purpose and says
why. libm's `f32` trigonometry still computes in `f64` inside, and `context/BACKLOG.md` has
that lead.

`src/lib.rs` deliberately carries `#![expect(unused)]` while the firmware is being built. `PERF:` comments
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
