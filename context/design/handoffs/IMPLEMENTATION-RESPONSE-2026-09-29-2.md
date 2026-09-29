# Implementation response: the 29 September update is built

For the design agent. It answers `context/octowhere-implementation-update-2026-09-29/` and
follows `IMPLEMENTATION-RESPONSE-2026-09-29.md`, the power key. The owner approved every change
in the update, and all of it is built: the V11 identity and impact, the HOLD LEVEL horizon, and
the wordmark-free clock with the wide battery well. `DECISIONS.md` entry 13 records it.
Captures are in `context/screen-captures/`.

## How it was checked

Every study was rerun from its own script beside the design backup, and the firmware's
renders were compared with the script's frames pixel by pixel. "Matches" below means that no
pixel in the named region differs by more than 48 of 255 in any channel. The remaining small
differences come from RGB565 and the edge rounding.

## Built as specified

- **Identity title.** Maratype at 112 px, natural proportions, centred on (233, 233). The ink
  measures `(33, 177)..(433, 289)`, as specified. The type-on, the flicker and the slices keep
  their timing. See `startup-identity.png` (frame 97), `startup.gif` and `startup.mp4`.
- **Subtitle row.** It matches V11: the barcode widened to 113.625 px, the square with the
  octagonal hole, the 4 × 5 px digits, the GNSS symbol, and both lines of copy. The dot is dark
  on frame 22 and every 30 frames after it. The row appears element by element from frame 21,
  as before.
- **Pluses.** 7 px arms at (23, 167) and (442, 298). They keep the ticks' arrival and flicker,
  and the partly lit frames show only the horizontal arm.
- **Pin.** It matches V11's round head, point and centred octagonal hole, at 1.17 px a unit. It
  appears from frame 56, and its partly lit frames show the three stems.
- **Impact.** The pin replaces the mark in all 19 frames, with the same cadence: striped, dark,
  lit, grown 1.9×, then dark on lime. The tile frame and hatch are gone. See `startup-card.png`.
- **Scatter.** Its gap now covers rows 163–340, to clear the taller title and row. The marks are
  otherwise unchanged.
- **HOLD LEVEL.** The icon is the centred horizon, `00000 00100 11111 00100 00000`. Nothing else
  in that state changed. See `compass-hold-level.png`.
- **Clock.**
  - The wordmark is gone, and the seconds are at x 388, baseline 244.
  - The black well is at x 262–439, y 281–310, and the fill at x 265–436, y 284–307.
  - The fill runs `round(172 × level / 100)` px from the left.
  - The solid fill, the low orange, and the white on NO ZONE and NO DATA all match the V2
    stills.
  - The `BAT --` hatch matches V4. It shows in every state, NO DATA included.
  - A known 0 % leaves the well empty.
  - The entry grows the fill from the left, where it used to rise.
  - See the `clock*.png` captures.
- **Charging slices.** They match the V2 script's `title_pattern` at every length and phase
  tested, a unit test checks them, and the endpoint never moves:
  - at 87 %, 20 bars and 19 gaps, 62 px lit;
  - at 12 %, 3 bars;
  - at 100 %, 23 bars.
- **Unchanged.** Maratype appears nowhere but the identity's title. The self-test, settings,
  zone values, both NO DATA states and the always-on face keep their type. The always-on face
  keeps its compact battery.

## Where the build interpreted the update

1. **The pin's position.** `IMPLEMENTATION-DELTA.md` gives the pin's origin as (437, 212). But
   the V11 script places it just off the title's top-right corner, at (437, 175), and every
   V11 render shows it there. The build follows the renders. If 212 was meant, say so.
2. **The title sits about a quarter pixel right.** The firmware places text on whole pixels, so
   each vertical edge's antialiased column differs slightly from the render. The ink box is the
   same.
3. **The horizontal wipe**, which the update left to the build:
   - Plugging in drains the solid layer's top edge down the well's 24 rows, revealing the
     slices. Unplugging raises it back over them.
   - Every wipe takes 450 ms. The old 180 ms wipe for a short fill is gone, because every wipe
     now crosses the same 24 rows.
   - A reversal turns back from wherever the edge is.
   - A face that appears while charging shows the slices at once.
   - `clock-charging.mp4` shows a plug-in, the loop, an unplug, and a plug-in cut short
     halfway. **Please review it.** It is the one motion here with no render behind it.
4. **The slices' widths come from version 0.1.0's barcode.** The identity's barcode encodes the
   running version, so the two will differ once the version changes. Say if the battery should
   follow the version instead.
5. **The owner's 4 s rest between charging passes stays**, as in the earlier update.
6. **USB with no battery** reads `USB` and also shows the hatch, since it has no level.

## On the device

Every part answered at start-up, and the clock face runs. Not yet checked: legibility on the
physical panel, or the owner's review of the start-up and the charging motion there.

**Some start-up frames take too long to draw.** The frames that type and flicker the title
take about 48 ms, against a 33 ms frame. The card's frames with the pin at 1.9× take about 97
ms. So the panel drops frames in those stretches, and the recordings, which don't, look
smoother than the device. The settled clock face draws within 15 ms. The owner's rule is to
finish the features before optimising, so this is recorded in `context/BACKLOG.md` rather than
fixed.

## For the design

1. Review the horizontal charge wipe in `clock-charging.mp4`.
2. Confirm the pin's position: (437, 175), from the renders, or (437, 212), from the text.
3. Say whether the charging slices should follow the running version's barcode.
