# The settings panel's frames with its halftone under the rows, 2026-10-04

`4ef2956` runs the settings panel's halftone under its rows, whose text then blends over the
marks instead of assuming a black background. These runs compare it with the commit before,
`8225080`, each built with `--features touch-inject,timing-log`. `drive.sh` flashed each build
to `1a38` and, from 12 s after the boot reports, opened the panel, dragged between its pages six
times, closed it and opened it again, with the same touches each time. `timing-log` logs each
frame's step and draw together. `phases.py` splits the drive by time into the panel opening, the
page drags and the panel settled, where only the halftone's breathing redraws; it rereads these
logs with `uv run docs/logs/display/settings-halftone-2026-10-04/phases.py
docs/logs/display/settings-halftone-2026-10-04 before after`.

| Frames | Before | After |
| --- | --- | --- |
| Opening, median (ms) | 31.3 | 31.5 |
| Opening, over a frame | 30 % | 31 % |
| Page drags, median (ms) | 36.2 | 39.8 |
| Page drags, 90th percentile (ms) | 44.4 | 47.3 |
| Page drags, over a frame | 69 % | 85 % |
| Settled, median (ms) | 7.4 | 8.8 |
| Settled, 90th percentile (ms) | 8.8 | 11.4 |

"Over a frame" is the share of frames that took longer than 33.3 ms. Each figure comes from one
run of each build, about 60 opening frames, 120 drag frames and 110 settled frames.

- **A page drag** redraws the whole panel every frame, and already took longer than a frame for
  most of them before. The blended text adds about 3.6 ms a frame.
- **The settled panel** redraws only the marks that change as the halftone breathes, and the
  rows over any of them. Those now include marks under the text, about 1.4 ms more a frame.
- **The opening** is unchanged.

The logs are `before-1a38.log.xz` and `after-1a38.log.xz`, and each `.marks` file holds its
log's count of timing lines when the drive started and ended.
