# The start-up's draw times after the heap work, 2026-10-03

Measured with the two commits of `bench/startup-handover` (`startup-handover-bench`). They replay
the start-up 6 s after each handover to the clock, every other time with a part failing, each part
in turn, so the fault screen runs as often as the identity. The screen never times out, and a
synthetic 87 % battery flips between charging and not every two replays. Each frame's step and
draw are logged.

The same two commits were built on three bases:

- **Before**, `fc2847b` (kept as `bench/startup-heap-before`): the identity's title in two
  buffers, and the fault screen's knockout gathering each glyph.
- **After**, `00db111`: the knockout painting rows as they arrive (`4708868`), the title replayed
  a glyph at a time from recorded runs (`3a774d1`), and the scatter's damage worked out in one pass
  (`c933578`).
- **Rows**, `94d6bcf`: the title's glyphs replayed together a row at a time, since reverted
  (`c1fda52`).

Each build ran for 240 s on each board, one board at a time, about 17 replays. The figures pool
charging and not; `tools/startup-handover-summary.py` on the bench branch splits them. "Over" is
the share of frames whose step and draw together took longer than a frame, 33.3 ms. Each cell
gives `1a38`, then `1c1c`.

| Frames | Before | After | Rows |
| --- | --- | --- | --- |
| Identity 0-65, draw median (ms) | 22.7 / 22.7 | 24.8 / 24.7 | 25.5 / 25.6 |
| Identity 0-65, over | 0.4 % / 0 % | 1.0 % / 1.3 % | 2.3 % / 2.1 % |
| Identity 66-110, step median (ms) | 2.4 / 2.4 | 2.6 / 2.6 | 2.6 / 2.6 |
| Card, draw median (ms) | 21.7 / 20.6 | 20.6 / 20.8 | 21.1 / 21.8 |
| Clock after the card, step median (ms) | 3.0 / 3.0 | 3.0 / 3.1 | 3.0 / 3.1 |
| Fault 0-119, draw median (ms) | 26.1 / 26.0 | 25.0 / 24.9 | 25.0 / 24.9 |
| Fault 0-119, over | 1.1 % / 1.3 % | 0.7 % / 0.7 % | 0.7 % / 0.7 % |
| Fault exit, draw median (ms) | 28.9 / 29.2 | 28.3 / 28.0 | 27.9 / 27.8 |

About 830 identity frames and 1,350 fault frames went into each run's figures, but only about
190 exit frames, whose share over a frame moved between 3 % and 12 % from run to run with no change
that touches them.

- **The fault screen** draws about 1.1 ms faster a frame, and its exit about 0.8 ms.
- **The identity's opening**, which redraws the whole panel every frame, draws about 2 ms slower,
  and 1 % of its frames run over where at most 0.4 % did. The title's runs are decoded every
  frame. Replaying the glyphs together, a row at a time, was slower still, so what costs time is
  the decoding, not the order in which the framebuffer's rows are walked. The owner chose to keep
  the recorded title for the 55 KB of heap it saves during the start-up (2026-10-03).
- **The identity's settled step** takes about 0.2 ms longer. It works out its scatter's marks
  through the code `c933578` restructured, the only change on its path.
- **The clock's step**, where the scatter's damage moved to one pass, and **the card** are
  unchanged.

The logs are `before-*.log`, `after-*.log` and `rows-*.log`, xz-compressed and named by each
board's hardware address. On `1a38` the GNSS module did not answer at two of these boots and was
reset, which delays only that boot's own start-up.
