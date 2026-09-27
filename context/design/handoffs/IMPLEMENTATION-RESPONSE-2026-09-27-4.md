# Implementation response: scatter spread, compass exit, hour rail

For the design agent. It follows `IMPLEMENTATION-RESPONSE-2026-09-27-3.md` with the owner's
changes since. Captures are in `context/screen-captures/`.

## Owner's changes

- **The settings scatter runs under the rows.** S1 kept the scatter out of a rectangle around
  the grid, so it ended in straight vertical edges at columns 77 and 389. The rows are drawn
  over the scatter without a fill of their own, so when they scrolled across it the cut
  showed. The rectangle is gone, and only the title and the hint stay clear. Both fields also
  grew from radius 160 to 220, so the marks thin out towards the middle rather than stopping.
  Near the text only a few dim marks show. See `panel-rest.png` and `panel-scrolling.png`.
- **The clock's scatter is wider**: the upper field's radius grew from 137 to 220, and the
  lower's from 111 to 178. The margins around the digits, icon, wordmark and token lines stay.
  Because each field thins towards its far side, the change is modest: about a fifth more
  marks, mostly towards the edges. See `clock-gnss.png`.
- **The compass field recedes as its page leaves.** Its blocks dim to 40 % of their brightness
  over the first half of a swipe away, rather than sliding off at full brightness. Dragging
  back restores them. The entry is unchanged.
- **The hour rail opens from its middle.** It used to appear whole with the zone's line. Now it
  opens outward in step with that line's reveal, 240–400 ms into the entry, and the hour marker
  shows once the opening reaches its cell. As the page leaves, it closes back towards the
  middle.

## Cost

These were checked in simulation and on the panel by the owner, and the draw times were not
measured. The compass dims during a swipe, which already redraws in full. The rail's reveal
redraws its own row. The larger scatter fields draw a few more marks per full clock draw, and
each breathing step changes a few more of them.

## Open for the design

- Whether the clock face wants a denser spread than the wider fields give.
- Whether 40 % is the right level for the compass field to recede to.
