# Backlog

Open work that is not in progress. Each entry says where its detail lives. Optimisation entries wait
until the feature set is complete, because profiling an incomplete firmware prices the wrong total.

## Next

- Build the protocol in [`LORA-PROTOCOL.md`](LORA-PROTOCOL.md). Its "Firmware structure" section
  comes first: the radio moves into its own task, and I2C gets a single owning task.

## Deferred, with detail elsewhere

- The partial-flush hardware check, in [`HARDWARE-VERIFICATION.md`](HARDWARE-VERIFICATION.md).
  Partial flushing is the default and the check has never been run.
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
- fontdue's compressed line store, a spike on fontdue branch `spike/line-store-compact`. It would
  replace raw baked lines at about a third of their flash. Version 3 adds 9.5–35% to a
  MarathonShapiro draw and 3.1 ms of startup for both fonts. The results are in
  `~/git/fontdue/MEASURE-LINE-STORE{,-V2,-V3}-ON-TARGET-HANDOFF.md`. The bench is the
  `fontdue-line-store` feature on `bench/fontdue`. Its `line-store` dependency is a `file://` git
  dependency on the local fontdue repository. The 16-byte builds also need path patches, and each
  handoff has the steps.

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
- `recip0.s`, `madd.s` and `msub.s` are verified on the core. The rest of the FP option, such as
  `rsqrt0.s`, `sqrt0.s` and `div0.s`, and every `esp32s3ops` instruction, are not.

Candidates to measure:

- Glyph coverage blending. `lerp_u8` in [`src/chrome.rs`](../src/chrome.rs) runs three scalar
  channel lerps per covered pixel. The SIMD extension could blend many at once.
- Framebuffer fills, copies and flush staging in
  [`src/drivers/framebuffer.rs`](../src/drivers/framebuffer.rs). 128-bit loads and stores help only
  where PSRAM bandwidth is not the limit, so measure the bandwidth first.
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
