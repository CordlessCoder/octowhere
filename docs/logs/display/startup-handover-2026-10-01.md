# Start-up and the clock's entry after it, on the target, 2026-10-01

Measured on `bench/startup-handover` at `bd19af6`, based on master `776a3ad`, with
`startup-handover-bench`. Summarised by `uv run tools/startup-handover-summary.py` on that branch
from the raw capture, [startup-handover-2026-10-01.log](startup-handover-2026-10-01.log).

The boot's start-up runs, then the bench replays it 6 s after each handover, six times. The
battery is a synthetic 87 %, charging on odd rounds, and the screen never times out. Each line is
one frame loop pass that drew: `step` is `Stage::step`, `draw` is `Stage::draw`, and `period` is
the time since the last pass's step began. A start-up frame can appear twice, because the next
pass repaints the other buffer. Clock frames are counted from the handover.

| Supply | Stretch | Passes | Step median / max (ms) | Draw median / max (ms) | Step + draw max (ms) |
| --- | --- | ---: | ---: | ---: | ---: |
| Battery | Identity 0-65 | 307 | 0.5 / 4.6 | 23.2 / 33.9 | 34.3 |
| Battery | Identity turn 66-110 | 167 | 2.4 / 11.5 | 2.6 / 28.2 | 31.1 |
| Battery | Identity 111-119 | 31 | 2.3 / 2.8 | 1.7 / 2.5 | 4.9 |
| Battery | Card 120-138 | 66 | 0.3 / 1.5 | 21.9 / 33.9 | 34.2 |
| Battery | Clock 0-159 ms | 20 | 9.0 / 11.7 | 11.9 / 24.0 | 30.5 |
| Battery | Clock 160-2000 ms | 132 | 3.0 / 10.5 | 2.2 / 8.6 | 17.0 |
| Charging | Identity 0-65 | 229 | 0.5 / 2.9 | 22.3 / 37.7 | 38.6 |
| Charging | Identity turn 66-110 | 165 | 2.4 / 5.7 | 2.5 / 25.2 | 28.1 |
| Charging | Identity 111-119 | 35 | 2.3 / 2.9 | 1.7 / 3.4 | 4.8 |
| Charging | Card 120-138 | 68 | 0.3 / 2.7 | 20.1 / 33.2 | 34.0 |
| Charging | Clock 0-159 ms | 19 | 9.2 / 13.1 | 12.3 / 20.4 | 30.1 |
| Charging | Clock 160-2000 ms | 198 | 2.8 / 10.8 | 2.1 / 17.1 | 27.0 |

- The upper scatter's turn on every frame from 66 to 110 keeps 30 fps: no frame of it was
  skipped in any round, and its damage was about 100 px a frame at the median.
- The gauge's build after the handover, from the rise's 160 ms, stays under 27 ms a pass with
  step and draw together.
- The identity's opening and the card redraw the whole panel every frame, and some passes take
  over 33.3 ms. One round of seven skipped two frames, 45 and 135. Neither stretch changed in
  this revision. The card's draw median is about 21 ms. A different bench noted about 12 ms on
  2026-09-26; the two benches' methods were not compared, and the difference was not looked
  into.
