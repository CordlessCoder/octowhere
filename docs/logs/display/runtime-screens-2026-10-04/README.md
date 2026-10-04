# The runtime screens' cost on a board, 2026-10-04

What the 2026-10-04 hand-off's screens cost board 1A:38 to step and draw, and the internal heap
they hold, measured with `bench/runtime-screens` (`runtime-screens-bench`, which implies
`timing-log` and the UI crate's `item-timing`). 1C:1C, in its group, sent the messages and kept
the GPS timebase. `tools/runtime-screens-bench.sh` on that branch drives the board through the
phases and reads the screen back after each, in a phase of its own; `tools/runtime-screens-summary.py`
reads the log beside them. Rerun:

```text
git worktree add /tmp/bench-screens bench/runtime-screens
cd /tmp/bench-screens
cargo build --release --offline --features runtime-screens-bench,touch-inject,pair-inject,fix-inject
# flash, capture the log, wait for the timebase, then:
tools/runtime-screens-bench.sh 1A:38 <log> <elf> 1C:1C <other elf> <phases> <shots dir>
uv run tools/runtime-screens-summary.py <log> <phases>
```

A step is the stage's step, from the frame loop's wake. A draw is the drawing into the
framebuffer, for the steps that changed pixels, and a flush the transfer that followed. All times
are in milliseconds; the heap peak is the internal heap's, sampled at each draw.

| File | What it is |
| --- | --- |
| `before.txt`, `before-phases.txt` | A whole run at `a54b8eb`, before the changes below. Every phase showed the screen it is named for. |
| `after.txt`, `after-phases.txt` | A run at `981a6c6`. From `inbox` on, the pager had moved to the compass, so those phases are not the screens named; the rest are. |
| `member-face-items-before.txt`, `-after.txt` | The member face's full draws by kind of item, before `80ac3cd` and after `0cc1cd0` |

What the runs showed:

- Opening the drawer, switching its roots and scrolling a list draw full frames of 40 to 70 ms
  (`before.txt`, p95), against a flush of about 16 ms. At rest the drawer redraws only where
  its halftone changes, 2 to 5 ms a frame. A conversation, the keyboard and a removal's
  countdown draw in 2 to 4 ms a frame at the median.
- The member face drew a full frame in 315 ms without a fix of its own and in about 123 ms
  with one, so a page swipe into it ran at about three frames a second. The coordinates
  card's outline, drawn as a path, took 205 ms of the 315: a path scanned the whole
  rectangle inside it. Four fills draw the same pixels (`80ac3cd`). Each arc of the ring took
  27 to 29 ms, with an angle found at every pixel though only the arc's ends need one
  (`80ac3cd`), and the grid's lines 52 to 54 ms, with a square root and a division at every
  pixel (`0cc1cd0`). The face now draws
  a full frame in 82 to 88 ms without a fix and 89 to 99 ms with one: the grid 35 to 40 ms,
  the arcs 16 to 30 ms, the text 12 to 17 ms. Renders before and after match but for 4 pixels
  in seven member-face renders, by at most 8 of 255.
- The frame loop stepped without waiting while the charging gauge or the drawer's halftone
  moved, though neither changes faster than the panel shows frames: about 400 passes a second
  on the clock face while charging, which took about half of core 0, and 137 a second with
  the drawer at rest, at 4.5 ms each. Stepped once a panel frame (`981a6c6`), the clock face
  makes 51 passes a second and the drawer 44. They drew 93 and 219 frames over the phase,
  against 94 and 257 before.
- The internal heap's peak was 119,936 bytes of 196,608, with the drawer open while messages
  arrived; 91,304 bytes on the faces.
- Every screenshot halts the board for about 11 s, so each one sits in a phase of its own,
  left out of the summaries.
