# Clock face addendum: wordmark

This adds the OCTOWHERE wordmark to the clock face. It changes nothing else in the clock face
specification, except to extend the entry and exit sequences by one step (below).

Files, in `images/`: `wordmark-gnss.png`, `wordmark-rtc.png`, `wordmark-stopped.png`,
`wordmark-nozone.png`, `wordmark-nodata.png` (every state), and `anim-B.gif`,
`anim-B-slow4x.gif` and `anim-B-entry-frames.png` (the entry and exit with the mark).

## Geometry

| Property | Value |
| --- | --- |
| Text | `OCTOWHERE` |
| Face | Marathon Shapiro Wide 65, 26 px |
| Turn | A quarter turn clockwise: letter tops face right, and it reads top to bottom |
| Column | Ink left x 366, 8 px right of the icon (x 262–357). About 19 px wide |
| Vertical | The gap between the O and the W sits on row 198, the band's top edge |
| Ink rows | About 105–316 |

- The size is chosen so that, with the O|W gap on row 198, the E ends at the band's foot
  (row 317). 26.2 px lands it exactly. At 26 px it ends one row short, on 316. Use 26 px unless
  the renderer takes fractional sizes.
- To place it: take the midpoint between the ink bottom of the fourth letter (O) and the ink
  top of the fifth (W), and put that midpoint on row 198. Then OCTO sits whole in the field
  and WHERE sits whole in the band.
- It clears everything else on the face. The seconds end at x 305, the band label at 307 and
  the icon at 357. At the bottom, the longest zone name ends at x 358 on rows 388–401, below
  the mark's last row.

## Inversion

The mark has no ground of its own. Each ink pixel takes the inverse of what is under it:

- over the field, `WHITE`
- over the band, `BLACK`, whatever the band's colour (`WHITE`, `ORANGE` in STOPPED, `RED` in
  NO DATA)

So OCTO is white on black and WHERE is black on the band. The mark adds no colour in any
state, and `RED` still appears only in the fault state.

To draw it, draw the string twice: in `BLACK` clipped to the band's rows 198–317, and in
`WHITE` clipped to the rows outside them. Both backgrounds are flat and known, so the
antialiased edges can use the fast path.

A quarter turn is a transpose and a flip of the upright glyph raster, not a general rotation,
so it costs about what upright text costs.

The mark is drawn in every state, NO DATA included. It is not drawn in the zone picker.

## Motion

The mark is an accent. It is absent while the page is dragged in, and it types in as the last
step of the entry.

- Entry: a cell reveal from 300 to 460 ms after settling, top to bottom. The whole entry now
  ends at 460 ms instead of 400.
- Each cell is the letter's advance along the column, less 1 px at each end, across the
  column's cap width. In the reveal, a cell shows as a solid block one step before its letter.
  The block takes the same inversion as the ink: white over the field, black over the band. No
  cell crosses the band edge, because the edge sits in the O|W gap.
- Exit: the reveal runs backwards, bottom first, over offset fraction p = 0.05–0.25, where p
  runs from 0 at rest to 1 at 35 % of the width. It goes early, just after the zone name starts
  to go.
- If a swipe starts before the entry has finished, the mark shows the lesser of its entry and
  exit progress, like every other accent.

## Change tracking

The mark never changes while the page shows. It redraws only on full frames: page swipes,
state changes that recolour the band, and its own reveal steps (one cell per step).

## Verification record

What I checked:

- The mark in all five states, and the entry frames every 20 ms.
- The mark's corners are inside radius 200, well inside the ring.
- The frame held after the entry is identical to the fully drawn face, so the sequence ends
  complete.

What I did not check:

- Drawing by the firmware's own code, or viewing on the panel. The renders come from a concept
  renderer (Pillow, 4× oversampled).
- Any draw cost, or the assumed 20 ms frame rate.
