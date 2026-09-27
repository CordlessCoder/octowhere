# Implementation response: no ring, zone list order, recordings

For the design agent. It follows `IMPLEMENTATION-RESPONSE-2026-09-27-2.md`. Captures are in
`context/screen-captures/`.

## Owner's changes

- **No perimeter ring on any screen.** The gray ring at radius 231 is gone from the clock face,
  the compass, the settings overview, the screens it opens, and the self-test, with its entry
  fade and swipe fade. During a swipe the moving page's ring was cut off by the glass's
  circle, and fading it early was tried and not chosen. NO DATA's red ring on the clock and
  the compass goes with it; both still show NO DATA in red on their band or slab.
- **The zone list's order moved below the slab.** D3 put `01  NEAREST FIRST` above the slab,
  where only the first entry was drawn. Past the first entry the zone before the selected one
  sits on those rows and the two overlapped. The order now shares the count's line, right
  aligned: `NEAREST FIRST  03 / 23`. See `picker-zone.png` and `picker-zone-scrolled.png`.

## Recordings only

The recordings now hold the device as a hand would: pitch and roll drift by a few degrees and
the heading by about a degree, so the tilt line visibly changes. A turn of the heading moves
quickly, runs a few degrees past its mark and settles back, rather than easing evenly. This
changes the synthetic readings in the recordings, not the firmware.

## Open for the design

- Whether the screens want anything at the edge in place of the ring. The glass's own edge
  is the boundary now.
- Whether the zone list's order and count read well on one line, or want a layout of their
  own.
