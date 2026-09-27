# Implementation response: charging bar, fault screen, and scatter

For the design agent. It answers `context/octowhere-implementation-update-2026-09-27/`
(the charging bar and start-up fault update), and records the owner's changes to the design
since. Captures are in `context/screen-captures/`, drawn by the firmware's own code through the
`render` example and `tools/ui-sim`. Readings in them are fixtures.

## Built as specified

- **Clock gauge.** Solid fill when not charging; while charging, uneven full-width bands in the
  six K1 states, with the state's gauge colour and orange at or below 15 %. Geometry as the
  update's table. `ui/charging.rs` reproduces `render_fill.py`'s band heights and gaps exactly
  at every height 0–99 and phase 0–71 (checked against the script; it needs f64 to match its
  largest-remainder ties). Unknown level: gray frame and the two dashes, no fill. 0 %: empty.
- **Wipe.** Solid layer under the bands, smoothstep edge, down on charge entry, up on stop,
  450 ms at 35 % and above, 180 ms below. A reversal mid-wipe continues from the current
  edge, its time scaled to the distance left; a level change mid-wipe keeps the exposed
  fraction. Entry holds the assembled layout, and the loop starts from it. No wipe on a first
  reading, an unknown or zero level, or page entry.
- **Fault ticker.** One line in the strip, 4 px a frame by absolute frame, alternating every 12
  frames between the failed parts' names in `#001DFF` and `FAULT_FAULT_` in `#ECDB0B`, both
  moving while hidden. Shapiro 34 px, ink top at row 242, the scripts' two-space gap between
  repeats. The inks are tokens `FAULT_BLUE` and `FAULT_YELLOW`, used only on the fault screen.
- **Several failures.** Count and the first two failures above the band at ink tops 162, 175,
  188, `+N` at column 298; the giant name and the reason stay the first failure's. The hatch
  shows only with a single failure. See `startup-fault-two.png`.
- **Exit.** 18 frames after the 120-frame hold, then the clock's entry: drop to row 408, then
  338 with the cap above 171 gone but the three fragments, the four edge bites, and the
  `340 × u²` lift. A touch still skips straight to the clock. See `startup-fault-exit.png`
  and `startup-failed.gif`.

## Where the build differs

- **NO DATA holds its bands still**, at the assembled phase, as the update's renders do
  (`render_fill.py` sets phase 27 for it). The update's text does not say so.
- **USB with no battery** shows the unknown gauge, since it has no level.
- **The band behind the giant name** runs to the edge of the square rather than the circle.
  During the exit, the corners of those rows that lift into the glass show black where the
  update's surface would be red. Everywhere else the surface is red past the circle, as asked.
- **Fault red** stays the firmware's `#F24723`, not the study's sampled `#EF4521`.
- **Unchecked on the panel:** RGB565 colour against the renders, and power at normal
  brightness.

## Measured on the device

Each figure is a draw only, not its transfer, with a synthetic 87 % battery and a
demonstrated magnetometer failure on bench `bench/charge-fault-draw`.

| What | Median | Max |
| --- | --- | --- |
| Fault screen frame, with the new ticker | 24.1 ms | 26.6 ms |
| Fault exit frame | 28.2 ms | 30.1 ms |
| Charging gauge, a redraw of the fill alone | 2.5 ms | 5.8 ms |
| Breathing scatter step (below) | 0.93 ms | 14 ms |

The exit's first build drew 41–52 ms a frame and skipped the frame where most of the red
drops out. Painting each pixel once fixed it: every exit frame now draws within the 33 ms
frame, and all 18 showed in each run.

## Owner's changes since the update

- **The charging loop rests.** After each 2.4 s pass the bands hold 4 s on the dispersed
  layout the pass starts and ends on, so the gauge is less distracting.
- **The resting scatter breathes.** This departs from the hand-off's "at rest, texture is
  static". On the clock face and the settings overview, the scatter's density eases down to
  75 % and back over 10 s, only while the screen is awake; it holds while dimmed. A step
  redraws only the marks that change, about five a second. The screens settings opens keep a
  still scatter at full density.
- **The fault ticker turns every 36 frames** (1.2 s) instead of 12, and the fault screen's
  hatch has 6 px stripes and gaps and no longer crawls.
- **Scatter entry and exit.** The settings overview now blooms its scatter in over 120 ms once
  the panel settles, as the clock face does. Both screens thin their scatter over the first
  half of a swipe or close, the bloom reversed, where before it moved away at full density.

## Open for the design

- Whether the breath's depth and period suit the panel at normal brightness; they were set in
  simulation, not on the glass.
- Whether the screens settings opens should breathe too, or bloom on entry.
- The breath and the charging loop keep the frame loop stepping while the screen is awake.
  Their power cost is unmeasured.
