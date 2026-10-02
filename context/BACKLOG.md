# Backlog

Open work that is not in progress. Each entry says where its detail lives. Optimisation entries wait
until the feature set is complete, because profiling an incomplete firmware prices the wrong total.

## Next

- Build the identity's version barcode in with the charging gauge's build (owner, 2026-10-01):
  the solid wiping in to the middle, the seed and the slices flying out on `ease::FLIGHT`, as
  `ui::charging` does since `DECISIONS.md` entry 24. Today the barcode in the row under the
  identity's title (`ui/identity.rs`, `BARCODE_WIDTH`) appears whole on its frame, as each of the
  row's elements does. Tell the design in the next update.
- Finish the approved architecture cleanup (owner, 2026-09-28): remove the u8g2-era assets,
  unused dependencies and helpers, and unused sensor snapshot fields; move motion processing into
  `octowhere-motion`. Those changes are in the current branch. Then move the generic I²C peripheral
  drivers in `src/peripherals/` into their own crate. `host-tests` currently imports some of these
  files by path. Move their tests with them and leave board bring-up and shared-bus ownership in
  the firmware.
- Build the 2026-09-26 design, [`design/`](design/README.md), which the owner approved in full
  (`design/DECISIONS.md`). One piece at a time, each compared against the hand-off's renders
  (`tools/design-compare.py`), reviewed by the owner in `ui-sim`, and measured on the board
  before it is committed. Built: the shared pieces (`VIOLET`, `DEEP_BLUE`, several scatter
  fields on one grid), the start-up (S1 self-test, G19 identity, G17 card), the K1 clock,
  the H2b always-on face, S1 settings with D3 inner screens, and C1 compass. Their on-target
  draw times are in `docs/logs/display/settings-draw-2026-09-26.md` and
  `docs/logs/display/compass-c1-draw-2026-09-26.md`. The owner found every screen good and
  legible on the panel (2026-10-01).
- Shorten settings saves further, if the remaining pause shows. Since `bde7924` each write is
  programmed rather than rewriting its sector, and a save holds the display core for about
  23 ms at the median, 85 ms at most, from 272 and 2,011 ms (`bench/settings-save`,
  2026-10-01). Erases are nearly all of it: 1 to 7 a save, about 11 ms each. Holding the core
  only around each erase would cap the pause at one erase, at the cost of a wait for a frame
  per operation; erasing a page ahead of time, or deferring the write until the panel is idle,
  are the other options.
- The scatter (`ui::scatter`) damages only the marks that appear or go, and the identity
  draws about 5 ms a frame once its band settles, 4.3 to 16.4 ms while it types in. What is
  left is overhead that does not shrink with the damage: working out which points show,
  about 1.8 ms a frame in the step; the scatter's draw asking `Clip::visible` for each
  point, about 1.7 ms; the clear, 1.6 ms for about 2,000 pixels; and the hatch's and the
  microtext's fills, which the clip rejects one at a time. A 6 × 2 `fill_solid` on the
  framebuffer costs about 1.7 µs even where the cache holds its lines, so small fills are
  call overhead rather than memory, on every screen. Since the multi-field scatter, the
  step also tracks each mark's kind and tests the glass per point, and its settled median
  rose from 1.84 to 2.28 ms; testing the glass as a column range per row, dropping its two
  small allocations a call, and reusing the static lower field's work are the candidates.
  For the draw, drawing from the step's shown bitset or walking the damage's spans per grid
  row would replace the per-point `Clip::visible`. The clear could skip undamaged rows or
  run from the spans. A lean `fill_solid` for narrow rectangles (integer clamp, one row
  offset, direct stores) would help every screen. `bench/scatter` times each part
  (`scatter-bench`), and at start-up the scatter, its arithmetic and small fills alone.
  - The start-up's fault screen draws a frame in about 25 ms, at most 28. The band and the
    strip are painted once, black with their text knocked out (`chrome::Knockout`). The
    clear of the rows outside them is 7 ms, the band with the giant name 7.2 and the strip
    with the running line 5.5. Writing the band's and strip's pixels is about 4.2 ms of that;
    a fill of the same rows would take about 2.3. Writing uniform runs as words, two pixels a
    word, or eight bytes of coverage at a time each beat the pixel-at-a-time write alone on
    the device and lost to it in the frame, for a reason not found. Without the writes, the
    name and the line still take about 3.7 and 3.2 ms beyond rasterizing: turning the
    raster into coverage rows, gathering and combining them. Damaging only the band and the
    hatch, the only parts that change between frames, is the other lever. `bench/fault-draw`
    times each part (`fault-draw-bench`), and at start-up the row write alone.

- Lay out the 512 KiB of SRAM deliberately. esp-hal's linker script gives `.data`, `.bss` and
  core 0's stack 341,760 bytes (`0x3FC88000` to `0x3FCDB700`); the stack is whatever the other
  two leave. Since 2026-09-26, 72 KiB of the heap sits in `dram2`, the RAM the ROM needs only
  during boot, and 168 KiB in `.bss`, which left core 0 about 90 KiB of stack (AGENTS.md,
  "Memory"). On 2026-10-02 the static part went to 120 KiB, which left 115,724 bytes of stack,
  after the radio task overflowed about 60 KB from under a group screen. Still open: size the
  heap from a peak measured across every screen (about 50 KB over the clock and the compass,
  `bench/clock-draw`; the fault screen's 40 KB glyph raster, the picker and the group screens
  were not in that run), the data cache's reclaimed segment above `0x3FCF0000`,
  what IRAM holds (15 KiB of `.rwtext`), the two 8 KiB display DMA buffers and the 8 KiB
  core-1 stack. Do not measure core 0's stack by painting it from `_stack_end` up to the stack
  pointer: that crash-looped the board, probably because esp-rtos keeps data there.
- Move the CO5300 driver into its own crate under `crates/`, with the QSPI command layer it
  needs, and implement more of the controller reusably (owner, 2026-09-24). Today
  `src/drivers/co5300.rs` covers init, address windows, brightness, TE and pixel streaming.
  The datasheet (`docs/datasheets/CO5300_Datasheet_V0.00.pdf`) also has TE modes and the scan
  line as proper settings, reading the current scan line (45h), partial and scroll areas, idle
  mode, deep standby, and high-brightness and contrast controls.
- Build the protocol in [`LORA-PROTOCOL.md`](LORA-PROTOCOL.md), in its "Build order". Steps 1
  to 3 are done: pairing, the member table, its storage and the screens, paired between the two
  boards by the mesh's commands and through the screens (`docs/logs/lora/pairing-2026-10-02/`,
  `docs/logs/lora/pairing-screens-2026-10-02/`). Step 4 is done too: listening to members and
  neighbours with a sweep every 13 rounds, the cancel rule, and a changed member record sent in
  the next slot (`docs/logs/lora/founding-and-listening-2026-10-02/`). Step 5, CAD, needs the
  slot timing it depends on measured first ("Time sync" there).
- Finish what step 3's screens leave open (`SCREEN-DESIGN-BRIEF.md`, "Group and pairing as
  built"):
  - Every group screen's legibility on the panel, which nobody has judged yet. Typing on the
    name keyboard's 39 × 53 px keys with a finger is accurate and responsive (owner, 2026-10-02).
  - The joining device cannot tell it was returning, so its RESTORED screen never shows. The
    group it receives would need to say so.
  - Reading a screen back with `touch-inject` takes about 11 s and interrupts the board. The
    link is the ESP32-S3's own USB Serial/JTAG controller, full-speed USB, whose JTAG side
    takes one 4-bit command per clock (the owner's probe-rs fork,
    `probe-rs-espressif/src/espusbjtag/protocol.rs`). probe-rs halts the core for every Xtensa
    memory read, so holding a halt across the whole read changed nothing: 10.0 s halted and
    10.2 s running for the 434,312-byte frame, 42 KB/s, on the fork's `c029e0a3`
    (`bench/jtag-read`). Its new tip `e1c448a8d` takes 11.8 to 12.4 s, 35 KB/s, alternating the two
    builds on one board, and a 64 KB read loses the same 20%. The probe-rs session bisected it
    to upstream's `c0b48362f` (#4318), which makes a 64 KB read take 15% more USB transfers;
    its mechanism is only partly identified. Its changelog's extra clock per transfer is about
    2 of the roughly 9.5 clocks a word it added, out of about 63; the rest likely comes from its
    fixed path between TAP states, not yet diffed. The fork's queue refactor (#4315) adds about
    0.5 s of host CPU a frame on top. Separately, and older: the IR is shifted again for every
    word though it always selects NAR, about 9 of the 63 clocks; caching the selected IR is on
    the probe-rs session's list (estimated from the path tables, not measured). Each 32-bit word
    is a NAR and an NDR scan, about 40 captured bits, and the driver blocks on the IN endpoint
    every 544 captured bits, about 13 words a round trip. Levers, estimated rather than
    measured: the driver's own comment quotes the TRM as allowing 128 bytes of capture before
    the device pauses, against the 68 it waits at; keeping IN transfers queued, as it already
    does for OUT, would leave the command stream as the limit, about 2 million clocks a
    second. On the firmware side, the frame run-length coded at four bytes a run came to 32 to
    64 KB for four of the boards' frames. An RTT channel (owner's suggestion) could carry that
    up and every injected input down, in place of the per-feature statics of `rtc-inject`,
    `pair-inject` and `touch-inject`; it uses the same memory reads.

- Make the partial flush cheaper. With the compass redrawing only what changed, a one-degree turn
  flushes about 28,000 pixels in about 42 regions and takes about 10 ms, against 14.8 ms for the
  whole panel. In the frame loop, per frame: the address window took 2.1 ms (three separate
  command transactions a region), starting the stream 0.7 ms, and moving the rows 6 ms, about
  three times the per-pixel rate of a region flushed alone. The display core copies each short
  row out of PSRAM itself while core 0 draws into the other buffer, and the two slow each other:
  the draw runs 2.4 ms faster with flushing held off. DMA straight from the PSRAM framebuffer is
  ruled out: the DMA cannot read PSRAM as fast as the SPI sends (see the flush entry). Opening
  each stream with `RAMWR` instead of a separate command, now in place, took the address window
  from 2.1 to 1.8 ms a frame. What remains is the row copies. The bench that measured this,
  `bench/row-span-damage`, was deleted; its last commit was `e92ff49`. Pixel shift's region
  path, which builds each row from the framebuffer row the shift picks, costs about 0.5 µs a
  row more than the straight copy it replaced, shifted or not: 15 to 22 µs on a small region
  and 0.6 % on a large one. A shifted whole frame costs 30 to 60 µs over the unshifted
  11.5 ms (`bench/flush-shift`, 2026-10-01, with no wait for TE).
- Shorten the clock face's draws further. Measured on 2026-09-25 with `bench/clock-draw`, after
  the face stopped laying out parts outside the damage and the clear stopped painting under the
  band: a tick draws in about 1.5 ms (2.7 before), a full draw in 17.5–22.6 ms (20.5–27 before),
  and an entry's draws total about 160 ms (236 before). What is left:
  - Each change draws twice, since the other buffer catches up on the next step, even with
    nothing new. Skipping the draw and the swap on a step that changed nothing would fold the
    catch-up into the next change, which on a tick covers the same pixels. But core 1 answers
    the settings hold only between frames, so the hold would need another way in first.
  - A sparse change pays per 64-byte cache line of PSRAM, not per pixel. A ring-fade step
    repaints 8,040 px in 8.2–9.5 ms: the clear takes 3.7 ms, the ring 2.3 and the band 1.2,
    each passing over the same rows in turn after the last pass has been evicted. Drawing a
    region row by row, every layer at once, would pay each line once.
  - A full draw: the clear 6.5 ms, the band 2.9, the hours and minutes 2.5 ms each pair at
    136 px, rasterized every time now that the glyph cache is gone, the date 1.5 and the ring
    about 2 once faded in.
  - A tick still spends about 0.55 ms rasterizing the seconds and 0.27 ms on the band's rows.
  - Frames during a swipe were not measured after the change.
- Take the framebuffer clear off the drawing core. It is paid per 64-byte PSRAM cache line:
  clearing only the visible circle saved 0.6 ms, not the 21% its area suggests. Partial redraws
  now clear only the damage, about 2.3 ms of a one-degree turn on the compass, much of it spread
  thin across many short spans. The candidates are a GDMA memory-to-memory clear, or core 1
  clearing a buffer after flushing it. Either changes the buffer hand-off in `util::Swap`, and
  partial redraws rely on a buffer keeping its own pixels, so only damaged spans may be cleared.
- Subset PP Fraktion Mono Bold to the glyphs in use, to recover some of the 54 KB it added.
- Add the LoRa radio to the self-test (owner, 2026-10-02). Its six cells, POWER to GNSS, are
  laid out by the S1 hand-off, so a seventh needs a place in that design first. `bring_up`
  already brings the radio up, and spawns `mesh::offline` when it does not answer, which the
  group screens show as NO RADIO; it reports nothing to `BOOT_REPORTS`.
- Bring the self-test's parts up concurrently (owner, 2026-09-28). `bring_up` joins the GNSS
  settle with one future that probes the clock, touch, IMU and magnetometer in turn, so each
  part's own waits (touch's 100 ms settle, the magnetometer's trim reads) add up. Give each part
  its own future and join them. The I²C transactions still take turns on the shared bus mutex;
  the waits between them overlap. Each part keeps its own deadline.
- Shorten the GNSS time to first fix (owner, 2026-09-28). The command details are in
  `docs/datasheets/LC76G_AGNSS_Application_Note_V1.1.pdf`. The firmware already sends the RTC's
  time after a reset or power-on, resets the module only when it does not answer, and saves its
  navigation data with a fix; the defaults it reports are in `docs/hardware-notes.md`. Left:
  1. Keep the last fix in the settings store and send it as reference position, `$PAIR600`,
     at start-up. It need only be within 30 km. A unit with no fix of its own could take a
     peer's position from the mesh instead (`LORA-PROTOCOL.md`), and a peer's time if its RTC
     has stopped.
  2. Widen EPOC's prediction to GPS with Galileo or BDS (`$PAIR498`). It predicts GPS alone
     now, and only from ephemeris the module has received itself. EASY is unsupported here.
  3. Set a static navigation threshold (`$PAIR070`, off by default), so a device standing still
     does not report a drifting position to the mesh.
  4. Tune the elevation mask (`$PAIR072`, 5°) and minimum SNR (`$PAIR058`, 9 dB-Hz) against
     field logs, trading multipath error for availability.
  5. BDS B1C (`$PAIR158`): the module accepts the command, but Quectel lists it only for the PA
     and PB variants with GPS and BDS alone. Enable it only if a test under the sky shows B1C
     tracked.
  6. Measure the time to first fix under the sky before and after each of these, cold and warm.

  Sharing orbit data between units does not work: the module can output its ephemeris as
  RTCM 3 (`$PAIR436`), but it takes orbit data only as Quectel's EPO (`$PAIR471`), which comes
  from Quectel's server and has no documented conversion from ephemeris.
- Show GNSS as faulted when resetting a stuck module keeps failing (2026-10-01). Moving the IPEX
  connector can leave the LC76G refusing reads at `0x54`, or answering with no NMEA, until it is
  reset. `gnss_task` now resets it after 8 failed reads in a row or 10 s of reads with nothing in
  them, at most once a minute, and sends it the RTC's time after. It publishes no fix while it
  does, but no screen says GNSS has failed: the self-test's fault is the only GNSS fault the
  design has, so a running one needs a design round. Strain relief on the IPEX cable addresses
  the trigger itself.

## Deferred, with detail elsewhere

- Touch-to-frame latency during a drag is about 60 ms at the median, 75 ms at p90, from the
  controller's report to the end of the flush that shows it (`bench/touch-latency`, 2026-09-30,
  a finger on the clock and the compass). Reading and stepping take under 2 ms and the read waits
  about 4–6 ms for the frame loop. The rest is the draw, about 30 ms, core 1's wait for TE,
  about 9 ms, and the flush, about 15 ms. The flush and TE are settled (below), so the draw is
  the lever. It was about 100 ms before `touch_task` read each report once and stopped waiting for
  the frame loop to take the one before. A lift counts `LIFT_GRACE` (30 ms) after its report,
  but with a finger that came to a median 54 ms on the compass and 40 ms on the clock face:
  the step at the deadline waits behind a draw. The draw is the lever there too.

- Shortening a full-panel flush is closed (owner, 2026-09-25): it was explored as far as it
  usefully goes. The findings stay here so nobody retries them. Measured during drags on
  2026-09-24 (`bench/tearing`): a flush took 13.3–15.9 ms, mean 14.5, once each chunk's
  transfer was spun on rather than awaited (it was 15.3). Per 8 KiB chunk, the CPU copy out of
  PSRAM takes about 205 µs while core 0 draws (115 µs with core 0 idle), and the transfer 204 µs
  alone but about 232 µs beside the copy. Transfers alone would take 11 ms a frame. Larger
  chunks from the heap gained 0.3 ms at 16 KiB and nothing more at 32 KiB, because the first
  chunk's copy is not overlapped. Frame rate during drags is set by core 0, not the flush: a
  step and full draw took 23 ms mean, 28 ms at most, for 38 frames a second. The flush matters
  there only through the PSRAM contention it adds to the draw. What was tried or left:
  - DMA straight from the PSRAM framebuffer does not work, and the reason is now known
    (`bench/psram-dma`, 2026-09-24). esp-hal's `DmaTxBuf` writes the cache back itself, and the
    framebuffer is 64-byte aligned. But the DMA cannot read PSRAM as fast as the SPI clock
    sends: at 80 and 40 MHz the panel showed long strips of one repeated pattern, and at
    10 MHz it was clean. None of the DMA's settings changed that at 80 MHz: 32- or 64-byte
    PSRAM blocks (esp-hal writes 64 as a value the S3's register description calls reserved),
    the channel's transmit FIFO (`OUT_SRAM_SIZE_CH`, whose field resets to 14, about 128 bytes
    by the documented formula; 80 bytes was worse, 256 no better), or core 0 not drawing at
    all, which helped only a little. The CPU copy reads PSRAM faster (41 MB/s while core 0 draws, 75 MB/s
    idle), so copying through internal buffers is the right design here, as in ESP-IDF's
    bounce buffers. The direct flush did take the copy's contention off core 0's draw, 23 ms
    down to about 20.
  - PSRAM at 120 MHz: not pursued (owner, 2026-09-25). On `bench/psram-120` the bench starts
    PSRAM at 80 MHz, then moves the memory core clock from 160 to 240 MHz, divides flash by 3
    on SPI0 and SPI1 so it stays at 80 MHz, and writes the PSRAM timing ESP-IDF v6.1's tuning
    chose on this board (`SMEM_TIMING_CALI` 7, `SMEM_DIN_MODE` 0x01249249). A 4 MB pattern check
    passed, and during the synthetic drag core 0's step and draw fell from 23.1 to 19.2 ms and
    the flush from 14.7 to 13.3 ms, 38 to 42 fps. But the board froze within minutes under the
    drag, and the cause was not found. esp-hal's `SpiRamFreq::Freq120m` hangs at boot, esp-hal
    has no MSPI timing tuning, and ESP-IDF calls octal PSRAM at 120 MHz experimental.
    `tools/idf-psram-reference` on that branch reproduces ESP-IDF's register dump. esp-hal also
    leaves PSRAM untuned at 80 MHz (extra dummy 0, sampling mode 0, where ESP-IDF sets 2 and 4),
    which is unexamined.
  - Flush only bands covering the visible circle, about 80% of the square, at the cost of an
    address window and an unoverlapped first chunk per band.
- The partial-flush hardware check, in [`HARDWARE-VERIFICATION.md`](HARDWARE-VERIFICATION.md).
  Partial flushing is the default. The owner has checked the compass by eye; the rest of the
  check has not been run.
- Flash encryption, delta coordinates, temperature-compensated RTC calibration, moving a group to
  its fallback band and messages longer than a packet, in the "Deferred" section of
  [`LORA-PROTOCOL.md`](LORA-PROTOCOL.md).
- `panic = "immediate-abort"`, in the "Binary size" section of [`AGENTS.md`](../AGENTS.md).
- fontdue's opt-in 16-byte `Line` (`compact-lines`), designed in
  `~/git/fontdue/DESIGN-compact-lines.md`. Once it lands, it will ask octowhere to confirm that the
  linked `.data` size is the same with the feature on and off. The spike put 1,760 B of line arrays
  in RAM: esp-hal's default `place-switch-tables-in-ram` copies `.rodata.cst*` into `.data`. The
  planned fix is one line static per font. Measure with `xtensa-esp32s3-elf-size -A` against a
  build of the same revision without the feature. Enabling `ESP_HAL_CONFIG_PLACE_ANON_IN_RAM`
  would move fontdue's other per-glyph arrays to RAM in either layout. Weigh that before
  uncommenting it.
- fontdue's compressed line store, on fontdue master since `2ad75496` and not taken while flash is
  plentiful. It is one macro argument, `store: true` (optionally `grid: 16`), on
  `fontdue_font_from_file!`. For MarathonShapiro at scale 2.1, before every font moved to 24, it
  stores the lines in about a third of their flash, draws 27%, 17% and 11% slower at 12, 32 and
  64 px, and differs from raw lines by at most 1 unit. The earlier spike results are in
  `~/git/fontdue/MEASURE-LINE-STORE{,-V2,-V3}-ON-TARGET-HANDOFF.md`. The bench is the
  `fontdue-line-store` feature on `bench/fontdue`, which does not build against the current pin:
  its `line_store_bench.rs` needs the API port in fontdue's
  `dev-tools/board/octowhere-line-store-d3.patch`.
- opt-level 3 for the compass draw, measured on 2026-09-24 and not taken: the heading frame went
  from 22.6 ms to 21.6 ms with the whole build at 3, 22.0 ms with only octowhere at 3, and 21.5 ms
  with octowhere and fontdue at 3, for 150 to 175 KB more image. Branch `bench/compass-draw-rows`,
  built with `--config 'profile.release.opt-level=3'` and the package variants.
- opt-level 3, measured on 2026-09-23 at fontdue `381f935c` and not taken. The whole profile at 3
  grows the image by 108,496 bytes for under 1.5% on full redraw, flush and rasterize. fontdue
  alone at 3 costs 160 bytes for 0.3–1.6% on rasterize and no change in redraw. Revisit only if a
  profile of the complete firmware puts fontdue on top. Branch `bench/opt-level`.

## Xtensa-specific acceleration

Survey the firmware's hot paths for gains from instructions the compiler does not emit on its own.
Do this after the feature set is complete. Profile first and pick candidates from the profile, not
from this list.

What is established:

- The fused reciprocal works. `recip0.s` plus two `msub.s`/`madd.s` Newton steps costs 17 cycles
  above a multiply. `__divsf3`, the ROM routine every Rust `f32` division calls, costs 49. The fused
  result stayed within 1 ulp. The bench is on branch `bench/f32-division`.
- Rust does not fuse a multiply and an add, and `f32::mul_add` is std-only. So FMA needs inline
  assembly, which needs `#![feature(asm_experimental_arch)]`. `freg` is the float register class.
- At opt-level `s`, a short fixed-count loop is not unrolled. Write a sequence out by hand when
  its counter and branch would land inside the dependent chain being timed.
- By default the target enables `esp32s3ops`, the 128-bit SIMD extension, and `loop`, the
  zero-overhead loops. Also `mac16`, `minmax`, `clamps`, `nsa` and `fp`. Every loop in the bench
  disassembly still compiled to `addi.n`/`bnez`, not a hardware `loop`. Whether LLVM ever emits
  `loop` here is unverified.
- Moving fontdue's code into IRAM changed the warm font benchmark by under 0.25%, because about
  4.5 KB fits in the instruction cache. Placement is not a lever for warm hot loops. Cold draws
  are unmeasured. Branch `bench/fontdue`.
- In fontdue's 16-byte `Line` spike, the fused reciprocal replaced two `__divsf3` calls per line.
  It recovered 83–88% of the gap to the cached 24-byte layout, and the output was bit-identical
  on target. It saved 1,546–1,973 cycles per glyph against about 1,370 for the divisions alone,
  so the calls also cost through the float spills around them. The results are in
  `~/git/fontdue/MEASURE-COMPACT-RECIP-ON-TARGET-HANDOFF.md`.
- At opt-level `s`, a closure passed to fontdue's `BitmapIter::fold` was not inlined: the
  disassembly showed a `callx8` per cell, and a per-pixel blend through it ran slower than
  collecting a row and blending it. Hot per-pixel work belongs in a plain loop over a slice, or
  behind a per-row callback such as `BitmapIter::rows`. Branch `bench/compass-draw-rows`.
- `recip0.s`, `madd.s` and `msub.s` are verified on the core. The rest of the FP option, such as
  `rsqrt0.s`, `sqrt0.s` and `div0.s`, and every `esp32s3ops` instruction, are not.

Candidates to measure:

- Glyph coverage blending. `lerp_u8` in
  [`crates/octowhere-ui/src/chrome.rs`](../crates/octowhere-ui/src/chrome.rs) runs three scalar
  channel lerps per covered pixel. The SIMD extension could blend many at once.
- Framebuffer fills and copies in
  [`crates/octowhere-ui/src/framebuffer.rs`](../crates/octowhere-ui/src/framebuffer.rs), and flush
  staging in [`src/drivers/framebuffer.rs`](../src/drivers/framebuffer.rs). 128-bit loads and stores
  help only where PSRAM bandwidth is not the limit, so measure the bandwidth first.
- fontdue rasterisation. The rotated-label path is the fontdue session's call, using the numbers
  above. Its outline accumulation is the other candidate.
- Screen-rotation maths from the magnetometer: heading, `atan2`, vector normalisation. `rsqrt0.s`
  is the candidate there.
- libm's `f32` trigonometry works in `f64`, which this core emulates in software: `sincosf`,
  `sinf`, `cosf`, `asinf` and `hypotf` reach `__muldf3` and the other double routines through
  their kernels and `rem_pio2f`. On the draw path, `compass_screen::tick_shape` and
  `letter_place` call `sincosf` for every tick and letter of the turning dial, and
  `smooth::disc_row` and `chrome::glyph_reach` call `hypotf`; the motion task calls `asinf`
  (found from the release ELF's literal pools, 2026-10-01). Unmeasured: bound it on the compass
  field bench by replacing the dial's `sincosf` with a table of the 360 whole degrees first.
  `HardIron`'s fit and the NMEA coordinates use `f64` directly and run once per sample or fix.
- AES-SIV for the protocol, which is software AES today; the chip's AES accelerator could take its
  blocks. It runs once per packet, so it only matters if profiling says so. X25519 runs once per
  pairing and does not qualify.

The deliverable is a table of candidates, with cycles measured before and after and which ones
were taken. Each measurement gets its own `bench/<topic>` branch, per "Measurement code" in
[`AGENTS.md`](../AGENTS.md).
