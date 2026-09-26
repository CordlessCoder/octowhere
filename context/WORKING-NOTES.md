# Working notes

How work on this repository has been done, and what was learned doing it, for an agent picking
it up on another machine. Open work is in [`BACKLOG.md`](BACKLOG.md); how the screens should
look is in [`design/`](design/README.md).

## How the owner works

- New designs are built one piece at a time, in the order the backlog gives. The owner reviews
  a candidate fastest live in `tools/ui-sim` on their own display: rebuild it and launch it in
  the background (not under Xvfb) for them. Recordings (`--record <scene> out.mp4`) are the way
  to share an animation.
- Compare each state against the hand-off's render before showing it:
  `uv run tools/design-compare.py pair out.png ours.png design.png`, where `ours.png` comes
  from the `render` example or `ui-sim`'s P key. The design's renders use fixture data; do not
  copy pixels or the Python scatter hash (hand-off §2).
- Measure a new screen's draw on the board before committing it. A screen that runs past a
  frame is recorded as an overshoot and optimised once it works, not before.
- A change to an approved screen needs a design document or a recorded owner decision.
  Owner decisions go in the design's `DECISIONS.md` or the spec they amend, at the time.
- The owner's esp-hal fork (`~/git/esp-hal` on the original machine) is theirs to write:
  code, comments, commits and PR text. esp-hal's AI policy forbids generated prose. An agent's
  part there is research, running the board and reviewing when asked. Bench code on this
  repository's `bench/*` branches is still the agent's to write.

## The board

- The development board enumerates as
  `/dev/serial/by-id/usb-Espressif_USB_JTAG_serial_debug_unit_44:1B:F6:86:1A:38-if00`. On the
  original machine two other devices share USB: the second LoRa board and an NXP MCU-LINK
  (`/dev/ttyACM1`, `/dev/ttyACM2` there). Never flash either; address the board by its by-id
  path, not a `ttyACM` number.
- Check `pgrep espflash` before flashing. Another session (the fontdue work) borrowed the board
  on the original machine and handed it back by message; reflash master after it returns.
- Never burn eFuses.
- It has no battery fitted. The PMIC reports none present, so the clock shows no battery reading,
  and the charging crawl has never run on the board. Its draw is unmeasured.
- From a shell without a TTY, `cargo run --release` flashes but its monitor fails ("Failed to
  initialize input reader"). Flash with
  `espflash flash --partition-table partitions.csv --port <by-id> <elf>`, then capture with
  `timeout N espflash monitor --non-interactive -L defmt --elf <elf> --port <by-id> > file`.
  That is a reset read. Filter the file after the capture ends: a pipeline under `timeout`
  loses its output.
- A reset read loses the first two seconds or so of log while USB reconnects. Put start-up
  microbenchmarks behind a 3 s delay.
- Never combine `--no-reset` with `--non-interactive`: it left the board frozen in download
  mode and the owner had to reconnect it. `--no-reset` alone reads without a restart.
- Opening the serial port resets the chip, even with DTR and RTS held low. To inspect a hang,
  halt it with `probe-rs` over the USB JTAG first (`probe-rs list` shows "ESP JTAG").
- The PMIC's I2C init fails about one boot in three right after flashing. Retry before
  suspecting a change.
- `pkill -f` with a pattern that matches its own command line kills the calling shell.
- `tools/flash-until.sh` on `bench/fontdue-pin` flashes and ends the capture on a marker or a
  panic. A normal boot prints `[DISPLAY] OK`.
- `bench/clock-draw` carries `tools/clock-bench.sh`, which flashes that bench and saves its log,
  and `tools/clock-bench-summary.py`, which gives the median time per part of the clock's full
  redraws, ticks and other draws. To measure uncommitted master code there, copy the changed
  module into the bench worktree and re-add its `part_timing::mark` calls.

## Measuring and optimising draws

- Bound a lever before working on it: turn the part off (a no-op build) and re-measure the
  frame. Per-row timers named a "next lever" on the fault screen that a no-op build showed had
  under 2 ms to give, and three faster writes were thrown away.
- Time per part (`part_timing::mark`) or per call, never per row: `Instant::now()` per row cost
  about 4 ms of a 25 ms frame.
- Microbenchmarks mislead here. Three row writes beat the simple loop alone and lost to it in
  the frame, for a reason not found. Confirm every result in the frame.
- Look for overdraw first. Clear to the colour a screen needs and paint each pixel once; the
  owner calls a double full-screen fill an obvious anti-pattern.
- Core 1's flush contends for PSRAM and moves a full fill by about 2 ms between runs. Compare
  medians over hundreds of frames, and rerun the baseline when a result surprises.
- Small `fill_solid` calls cost call overhead, not memory: a 6 × 2 fill takes about 1.7 µs
  even cache-hot. libm `sqrtf` is about 0.56 µs and a division (ROM `__divsf3`) about 0.26 µs;
  `core::intrinsics::sqrtf32` is slower than libm.
- Rates seen: a word fill about 41 ns/px alone and 61 ns/px while core 1 flushes; the
  pixel-at-a-time coverage write 75–100 ns/px.
- Core 0's stack is what DRAM statics leave (AGENTS.md, "Memory"). Keep large per-frame
  structures small, and keep big buffers on the heap rather than passing through the frame
  loop's stack.
- esp-backtrace's defmt path drops formatted panic messages; an exception shows only its
  location. esp-rtos keeps a guard word at offset 60 of the core-1 stack, so skip it when
  watermarking.
- A bench that loops one screen works well: a feature that replays it, per-part totals logged
  per frame, and a `uv` script over the defmt log (median per part, skipped frames found from
  gaps in the frame number).

## Driving ui-sim without a display

- Under Xvfb: `timeout 40 env -u WAYLAND_DISPLAY xvfb-run -a -s "-screen 0 800x600x24" bash -c
  "timeout 30 <ui-sim binary> & sleep 0.5; timeout 25 uv run -q drive.py; wait"`. Wrap every
  part in `timeout`: a run once hung for six minutes.
- With no window manager, the 466 × 466 window sits at (167, 67) and keyboard focus follows
  the pointer. A pointer outside the window silently drops every later key, Escape included.
- Drive it with a PEP 723 script using `python-xlib`'s `xtest.fake_input`. Keys are in
  `tools/ui-sim/src/main.rs`'s header. Recording a scene (`--record`) needs no display at all
  and is usually the better route.
- To measure spacing, render with the `render` example and scan rows of the PNG with Pillow for
  lit pixels. That is how the owner's even-spacing requests were met.

## Scripts

Run throwaway scripts with `uv run`, with a PEP 723 header for dependencies, never bare
`python3`. Keep them in a subfolder of a scratch directory: a stray `copy.py` beside a script
shadows the standard library.

## Settled, not to reopen

- Shortening the full-panel flush is closed, including DMA straight from PSRAM and PSRAM at
  120 MHz. `BACKLOG.md`, "Deferred", has why.
- Pixel shift, when built, moves the whole frame in core 1's flush copy, never on core 0.
  `BACKLOG.md` has the design.
- Compass behaviour: the slab never moves between states, TOP EDGE UP has a 750 ms grace, a
  cover re-arms after 260 ms without a report or on a finger, and a cover goes to the clock
  from anywhere. The compass no longer recalibrates on cover; only the panel's COMPASS cell
  does.

## Outside the repository

On the original machine, and not needed to build or continue the design:

- `context/octowhere-design-project/`, the design agent's full backup (ignored by git). It
  holds explorations, historical packages and the design agent's transcript. Everything the
  build needs is in `context/design/`.
- The two Marathon reference videos in `context/` (excluded in `.git/info/exclude`). The
  design's `references/VIDEO-TIMING-MAP.md` gives their timecodes.
- `~/git/esp-idf`, a blobless ESP-IDF clone kept for reading Espressif's reference code, and
  `~/git/fontdue`, the fontdue fork's working copy with its design and measurement notes.
