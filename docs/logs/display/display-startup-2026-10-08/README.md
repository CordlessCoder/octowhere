# The display's start-up, 2026-10-08

How long the panel takes to show its first frame after a reset, and what four changes took off
it. `bench/startup-timing` (`startup-timing-bench`) marks when boot reaches each step, in µs from
the timer's start, and logs the marks 9 s in. For this round it also marks when `main` starts,
when the settings and the mesh's state have been read, when core 1 starts the panel's controller
and flushes its first frames, and when the panel first takes a level above zero. Each capture is
a reset read by `tools/startup-timing.sh`, so every boot is a warm start, with the panel still
running from the boot before. Rerun:

```text
git worktree add /tmp/startup bench/startup-timing
cd /tmp/startup/firmware
cargo build --release --offline --features startup-timing-bench
cd .. && tools/startup-timing.sh <out-dir> 5 <by-id port>...
uv run tools/startup-timing-summary.py <out-dir>
```

| Directory | What it is |
| --- | --- |
| `before/` | `89ba403`, master's `88b4f8f` with the marks, flashed at espflash's default 40 MHz; five boots on each board |
| `steps/flash-80mhz/` | The same code flashed at 80 MHz; three boots on 1A38 |
| `steps/one-pass/` | With the members and the kept rows read by range |
| `steps/early-reset/` | With the panel's reset pulsed by core 0 before the reads, and the identity made after core 1 starts |
| `after/` | `d6fe523`, master's `57f427d` with the marks, flashed at 80 MHz; five boots on each board |
| `reset-check/` | Whether the panel's reset pulse resets the controller (`startup-te-check`), and a one-off build without the pulse |

`summary.txt` is the summary script over all of them. The step runs came before `89ba403`, which
logs `main`'s start in µs; in theirs it is in the timer's ticks, 16 to the µs.

Medians in ms:

| | 1A38 before | 1A38 after | 1C1C before | 1C1C after |
| --- | ---: | ---: | ---: | ---: |
| `main` starts | 593 | 462 | 593 | 461 |
| Settings read | 647 | 515 | 641 | 509 |
| Mesh's state read | 922 | 594 | 856 | 585 |
| Core 1 starts | 964 | 612 | 898 | 603 |
| Controller started | 1,227 | 745 | 1,161 | 736 |
| First frame flushed | 1,296 | 780 | 1,230 | 771 |
| Panel lit | 1,308 | 790 | 1,242 | 781 |
| Start-up over | 6,433 | 6,255 | 6,364 | 6,263 |

"Start-up over" is the hand-over to the clock face. One boot of the ten after took 7.36 s,
because its GNSS check took 1.2 s, as one of the step runs' did. The display had no part in it.

On 1A38, step by step:

| | `esp_hal::init` done | Mesh's state read | Controller started | Panel lit | Start-up over |
| --- | ---: | ---: | ---: | ---: | ---: |
| Before | 598 | 922 | 1,227 | 1,308 | 6,433 |
| Flashed at 80 MHz | 464 | 788 | 1,093 | 1,174 | 6,301 |
| Members and kept rows read by range | 466 | 594 | 899 | 940 | 6,112 |
| Panel's reset from core 0 | 466 | 594 | 744 | 786 | 6,097 |
| First step waits for the panel | 466 | 594 | 745 | 790 | 6,255 |

What each change did:

- Flashing at 80 MHz. espflash writes the flash clock into the image's header, 40 MHz unless
  told otherwise. The ROM and the bootloader read the image at that clock. The firmware sets its
  own, 80 MHz, when it starts the PSRAM. At 80 MHz `main` started at 460 ms instead of 593, and
  every mark after it moved by about the same 134 ms. The bootloader logs that it has loaded the
  app at 446 ms on its own clock. Its earlier lines are lost while USB reconnects, so the rest of
  the 460 ms was not broken down. QIO did not boot: the ROM failed to load the bootloader
  (`ets_loader.c 78`), and the watchdog reset it until the board was flashed again in DIO.
- Reading the members and the kept rows by range. Reading the mesh's state took 274 ms on 1A38,
  260 of them in 64 lookups: one for each of the 32 member ids and one for each kept row. Each
  took 3.5 to 4.7 ms, and most found nothing, since the group has two members and neither board
  keeps a row. One cursor over each range read them in 54 and 9 ms. Both boards read back the
  same state as before: generation 20, members 0 and 1, and a seed, name, sequence and removal
  state. With no kept rows stored, their range ran empty.
- Pulsing the panel's reset from core 0. Core 1 used to pulse the controller's reset line as it
  started, wait 120 ms, send sleep out, and wait 120 ms more. Core 0 now pulses it right after
  `esp_hal::init`, so the first 120 ms runs out during the reads, and core 1 sends sleep out as
  soon as it starts. Making the identity's keys, 23 ms, moved to after core 1's start. The
  controller started at 744 ms instead of 899.
- Waiting for the panel. Core 1 used to flush the buffer it starts with, never drawn, before the
  frame loop's first frame. And the stage's first step, from which the start-up's sequence runs,
  came about 260 ms before the panel could take a frame. The 200 ms fade-in had finished unseen
  by then, so the panel showed the first frame dark and the next at full level. Now core 1 swaps
  the undrawn buffer away without flushing it, and the frame loop's first step waits for core 1
  to start the controller. The panel lights on the second frame, near the start of the ramp,
  and the fade-in shows. Since the sequence now starts later, the start-up ends 158 ms later than
  in the step before, and 178 ms earlier than before the changes.

Where the 790 ms to the panel's first light now goes, on 1A38:

| | ms |
| --- | ---: |
| ROM and bootloader | 462 |
| `esp_hal::init`, the PSRAM, esp-rtos and the settings | 53 |
| The mesh's state | 79 |
| The framebuffers and core 1's start | 18 |
| The controller's start: sleep out's 120 ms, 10 ms after `MADCTL`, the commands | 133 |
| The first frame, drawn and flushed | 35 |
| The second frame, which lights the panel | 10 |

The reset check. A hardware reset turns the controller's TE line off, so with
`startup-te-check` core 1 waits up to 60 ms for a TE pulse before it starts the controller. A
pulse means the controller kept running through the reset. With the reset line held high and
no pulse at all, TE pulsed 9 and 18 ms into the wait, on 1A38's two boots: the controller keeps
running through an ESP32 reset. With the single 10 µs pulse, TE stayed silent on both boards'
two boots each. The driver before sent a 10 ms pulse and then a second one of 10 µs, from the
vendor's driver; the firmware's start-up now sends the one.

The self-test's checks now run beside its frames. Before, the frame loop waited for core 1
until about 1.28 s, after every check had ended, so the checks ran alone. Now they start 330 ms
earlier, at 640 ms, while the frame loop draws the self-test in the same thread-mode executor.
Each takes longer in wall time, and the last ends at about the same time as before (1,231 ms
against 1,238 on 1A38):

| Check, 1A38 | Deadline | Before | Early reset | After |
| --- | ---: | ---: | ---: | ---: |
| Clock | 200 | 0.9 | 1.5 | 61 |
| Touch | 600 | 111 | 125 | 128 |
| Motion | 500 | 152 | 153 | 218 |
| Magnetometer | 500 | 144 | 226 | 346 |
| GNSS | 4,500 | 11 | 17 | 25 |
| Radio | 500 | 2.7 | 2.1 | 115 |

Every check passed in every boot. The magnetometer's took 343 to 346 ms in all ten, 69% of its
deadline, against 29% before.

No warnings or errors in any capture.
