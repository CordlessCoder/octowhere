# Compass design changes after the answers document

This is a change notice for the implementation agent. It updates the earlier `compass-design-answers.md` where the two conflict; other decisions in that document still apply. The lower-slab comparison was an exploration, not a set of approved pixel coordinates.

1. **Remove the separate heading abbreviation.** Do not show a centre `N`, `NNE`, `NE`, etc. beneath the heading readout. Keep the three-digit degree heading and the four rotating `N E S W` letters on the dial. Do not fill the vacated line with another label.

2. **Shorten the interference warning.** Display `INTERFERENCE` instead of `MAG INTERFERENCE`. Keep its orange warning treatment; only the wording changes.

3. **Move the shared readout slab lower.** The lower placement from the comparison is the preferred direction: put the numeric heading nearer the position previously occupied by the secondary heading text, above the tilt information. Apply the same slab position and dimensions to heading, interference, calibration, top-edge-up and NO DATA so state changes do not jump. Keep the heading and percentage digits and their suffixes aligned within that slab.

4. **Put situational text above the lower slab, with tighter spacing.** `INTERFERENCE`, `TURN ALL WAYS` and `TOP EDGE UP` belong above the slab in their respective states. The exploratory lower-slab rendering left too much space between the `MAGNETIC` caption, the `INTERFERENCE` line and the slab. Bring those elements into a compact visual group. No replacement coordinates or exact gap sizes have been approved yet; determine them by rendering and reviewing the actual layout. The ordinary heading state has no situational text in that group.

The interim arrangement with situational messages below `COVER SCREEN TO RECAL` is superseded by items 3–4. Preserve the cover gesture, state semantics, dial behavior, common slab geometry and previously specified animation rules while refining this vertical placement.
