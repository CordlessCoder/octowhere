# The member face on a board, 2026-10-04

The 2026-10-04 hand-off's member face on board 1A:38 (id 0, Dredge), with 1C:1C (id 1, Roger
Roger) beside it in their group of two. Neither board has a GNSS fix indoors, so both ran the
working tree built with `fix-inject`, `touch-inject`, `pair-inject` and `timing-log`, and
`tools/fix-inject.py` stood positions in for fixes: 1A:38 in Dublin for its screens only, and
1C:1C 420 m away at 025° for its screens and its mesh, which took GPS time from its RTC. The
screens were driven and read back over the USB JTAG with `tools/touch-inject.py`. The logs are
the two boards' serial captures, with the `timing-log` and QSPI lines taken out.

| Shot | What it shows |
| --- | --- |
| `no-positions.png` | The face before any member's position had arrived: NO POSITIONS, north up |
| `ring-north-up.png` | 1C:1C's position on 1A:38's ring: 025° true, 420 m, its age and its direct age |

What the run showed:

- 1C:1C took GPS time at 137 s, once its first sweep ended, and its first packet reached 1A:38
  at 161 s with its position (`1a38.log`). 1A:38 adopted its timebase and placed it at once.
  The face showed the distance and bearing the injection set.
- Neither compass was calibrated, so both faces stayed north up, as the hand-off asks when the
  heading cannot be trusted. Turning, and the cost of the full redraw each degree of it brings,
  was not tried.
- With the ring showing and still, the frame loop stepped on each 20 ms motion sample, and
  steps came 22.3 ms apart at the median over 183 frames. Stepping and drawing took about 2 ms
  of that, since the timer also counts the wait for the sample.
- The GNSS module's resets and I2C failures in `1a38.log` follow the debugger's 11 s halts for
  each shot, as in the Events run. The shots also show pixels past the glass's edge, which the
  panel never shows: what the page drew there on its way across.
