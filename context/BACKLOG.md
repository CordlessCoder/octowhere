# Backlog

Open work that is not in progress. Each entry says where its detail lives. Optimisation entries wait
until the feature set is complete, because profiling an incomplete firmware prices the wrong total.

## Next

- Shorten settings saves, ahead of other work that touches settings (design response,
  2026-09-24). Settings are in ekv now, and each write transaction starts a new file that
  erases a whole 4 KiB page first, so every save erases. The first saved brightness took
  331 ms, all of it with core 1 held, so the display stopped updating for that long. The frame
  loop now queues the write only once the frame confirming it has been flushed, so the freeze
  follows the confirmation rather than hiding it. The sequential-storage map it replaced took
  about 1.3 ms for a write into a sector with room (`bench/zone-lookup`). Options: hold core 1
  only around each flash operation rather than the whole transaction, erase a spare page ahead
  of time, defer the write until the panel is idle, or batch saves.
- Smoother transitions between the compass's states, such as interference coming and going
  (owner, 2026-09-24). Today the slab recolours, the state line appears and the icon swaps in one
  frame, as the compass animation addendum specifies ("the state line is never animated").
  Changing that is a design change to an approved screen, so it starts with a design round
  against `context/compass-animation/COMPASS-ANIMATION-ADDENDUM.md`.

- Lay out the 512 KiB of SRAM deliberately. esp-hal's linker script gives `.data`, `.bss` and
  core 0's stack 341,760 bytes (`0x3FC88000` to `0x3FCDB700`); the stack is whatever the other
  two leave. The internal heap is a static in `.bss`: 252 KiB from the first commit, cut to
  240 KiB when moving settings to ekv grew `.bss` and left a 13.9 KiB stack that overflowed
  silently in the partition table read. At boot the heap held 4,548 bytes; its peak while the
  screens run has not been measured. Unused so far: `dram2` (`0x3FCDB700` to `0x3FCED710`, about 72 KiB, which the ROM
  needs only during boot and `esp_alloc` can take with `#[esp_hal::ram(reclaimed)]`, as the
  commented line by `heap_allocator!` in `src/main.rs` shows), and the data cache's reclaimed
  segment above `0x3FCF0000`. Also look at what IRAM holds (15 KiB of `.rwtext`), the two 8 KiB
  display DMA buffers, the 8 KiB core-1 stack, and a guard or a measured watermark for core 0's
  stack so an overflow faults instead of hanging.
- Move the CO5300 driver into its own crate under `crates/`, with the QSPI command layer it
  needs, and implement more of the controller reusably (owner, 2026-09-24). Today
  `src/drivers/co5300.rs` covers init, address windows, brightness, TE and pixel streaming.
  The datasheet (`docs/datasheets/CO5300_Datasheet_V0.00.pdf`) also has TE modes and the scan
  line as proper settings, reading the current scan line (45h), partial and scroll areas, idle
  mode, deep standby, and high-brightness and contrast controls.
- Build the protocol in [`LORA-PROTOCOL.md`](LORA-PROTOCOL.md). Its "Firmware structure" section
  comes first: the radio moves into its own task, and I2C gets a single owning task.

- Make the partial flush cheaper. With the compass redrawing only what changed, a one-degree turn
  flushes about 28,000 pixels in about 42 regions and takes about 10 ms, against 14.8 ms for the
  whole panel. In the frame loop, per frame: the address window took 2.1 ms (three separate
  command transactions a region), starting the stream 0.7 ms, and moving the rows 6 ms, about
  three times the per-pixel rate of a region flushed alone. The display core copies each short
  row out of PSRAM itself while core 0 draws into the other buffer, and the two slow each other:
  the draw runs 2.4 ms faster with flushing held off. DMA straight from the PSRAM framebuffer is
  ruled out: the owner says it does not work on this path. Opening each stream with `RAMWR`
  instead of a separate command, now in place, took the address window from 2.1 to 1.8 ms a
  frame. What remains is the row copies. The bench that measured this,
  `bench/row-span-damage`, was deleted; its last commit was `e92ff49`.
- Shorten a full-panel flush, or take it off PSRAM contention. Measured during drags on
  2026-09-24 (`bench/tearing`): a flush took 13.3–15.9 ms, mean 14.5, once each chunk's
  transfer was spun on rather than awaited (it was 15.3). Per 8 KiB chunk, the CPU copy out of
  PSRAM takes about 205 µs while core 0 draws (115 µs with core 0 idle), and the transfer 204 µs
  alone but about 232 µs beside the copy. Transfers alone would take 11 ms a frame. Larger
  chunks from the heap gained 0.3 ms at 16 KiB and nothing more at 32 KiB, because the first
  chunk's copy is not overlapped. Frame rate during drags is set by core 0, not the flush: a
  step and full draw took 23 ms mean, 28 ms at most, for 38 frames a second. The flush matters
  there only through the PSRAM contention it adds to the draw. Levers:
  - DMA straight from the PSRAM framebuffer, removing the copy and its contention. The owner
    ruled it out before on this path. The missing piece is a cache writeback before each
    transfer, and esp-hal 1.2.2 has it: `esp_hal::soc::cache_writeback_addr` wraps the ROM's
    `Cache_WriteBack_Addr`, and its DMA buffers call it on PSRAM ranges. Still to check is
    whether the PSRAM can feed 40 MB/s while core 0 draws.
  - Flush only bands covering the visible circle, about 80% of the square, at the cost of an
    address window and an unoverlapped first chunk per band.
- Take the framebuffer clear off the drawing core. It is paid per 64-byte PSRAM cache line:
  clearing only the visible circle saved 0.6 ms, not the 21% its area suggests. Partial redraws
  now clear only the damage, about 2.3 ms of a one-degree turn on the compass, much of it spread
  thin across many short spans. The candidates are a GDMA memory-to-memory clear, or core 1
  clearing a buffer after flushing it. Either changes the buffer hand-off in `util::Swap`, and
  partial redraws rely on a buffer keeping its own pixels, so only damaged spans may be cleared.
- Build Shapiro from its full font file with fontdue's `chars:` option, as both PP Fraktion
  Mono weights already are, instead of the hand-made ASCII subset under `assets/`. Regular moved
  for the device page's `©`. Subsetting Bold to the glyphs in use would also recover some of the 54 KB it
  added.

## Deferred, with detail elsewhere

- The partial-flush hardware check, in [`HARDWARE-VERIFICATION.md`](HARDWARE-VERIFICATION.md).
  Partial flushing is the default. The owner has checked the compass by eye; the rest of the
  check has not been run.
- CAD, flash encryption, delta coordinates and temperature-compensated RTC calibration, in the
  "Deferred" section of [`LORA-PROTOCOL.md`](LORA-PROTOCOL.md).
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
- ChaCha20 for the protocol, which is software today, and the SIMD extension could vectorise it. It
  runs once per packet, so it only matters if profiling says so. X25519 runs once per pairing and
  does not qualify.

The deliverable is a table of candidates, with cycles measured before and after and which ones
were taken. Each measurement gets its own `bench/<topic>` branch, per "Measurement code" in
[`AGENTS.md`](../AGENTS.md).
