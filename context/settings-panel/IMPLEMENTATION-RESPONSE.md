# Settings panel: implementation response

For the design agent. The firmware implements `SETTINGS-PANEL-SPEC.md` as of master `c2d6af4`
(2026-09-24), and the owner has used it on the board. This records what was built, what the
owner decided during implementation, where the build had to interpret the spec, how closely it
matches the concept images, and the state of the UI a next round starts from. The spec itself
has been edited to carry the owner's decisions; this document says why and what else changed.

## Where to see it

- `crates/octowhere-ui/examples/render.rs` draws stills through the firmware's own code:
  `panel-rest`, `panel-end`, `panel-scrolling`, `panel-pulling`, `panel-device`,
  `panel-device-end`, `settings-brightness`, `settings-clear`, `picker-offset`, `picker-zone`,
  beside the faces' captures. Command in its header.
- `tools/ui-sim` plays and records scenes: `--record settings out.gif|out.mp4` is the panel
  alone (open, scroll, brightness, picker, close), `--record tour …` walks both faces and ends in
  the panel. Recordings sample every 20 ms and mark the finger: a disc while down, a ring
  fading for 300 ms after a lift.
- Code: `crates/octowhere-ui/src/ui/` — `panel.rs` (grid, layout, contents, accents), `sheet.rs`
  (vertical travel over the faces), `second.rs` (brightness, device page, clear), `picker.rs`,
  `text.rs` (placing text by ink), `stage.rs` (routing, timing, effects).

## Owner decisions during implementation

All are in the spec's Decisions (items 5–9) or its body.

1. **Six cells.** Battery and GNSS data were plumbed to the screens, so the grid has three
   columns and scrolls, as drawn.
2. **COMPASS cell** restarts calibration and closes to the compass, with no confirm.
3. **The opening drag may start anywhere** on a face, not only the top cap.
4. **Clear warning** reads `ERASES ZONE, LAST FIX ZONE` / `AND BRIGHTNESS`. The settings store
   holds the zone mode, the manual zone, the last zone GNSS found, and the brightness; compass
   calibration is not stored.
5. **A cover on any screen goes to the clock face.** On an editor it first discards the edit:
   brightness returns to its opening level, nothing is stored or cleared. From the panel it
   closes with the released-drag motion, and the face under it becomes the clock.
6. **The compass loses cover-to-recalibrate**, its `COVER SCREEN TO RECAL` hint and the divider
   above it, with their entry and exit steps. The compass addendum carries a note; its tables
   still list them as a record. The tilt line now ends the compass's stack.
7. **Brightness takes any whole percentage from 10 to 100.** The owner found the default 47 %
   unreachable at ten steps. p = clamp(round(100 (x − 66) / 334), 10, 100); the controller gets
   round(255 p / 100). Each track cell is a tenth of full, and the cell the level falls in fills
   from its left as far as the level reaches (47 %: four full, the fifth 70 %).
8. **Without a trusted date the picker lists a zone under every offset it keeps in a year.**
   After a power loss the clock reads 1 Jan 2000 until GNSS sets it, and January offsets put
   Dublin under +00:00 only; now it is under +00:00 and +01:00. The step 2 abbreviation is the
   one belonging to the chosen offset. Rows still show `--:--`.

Earlier the same day, and already in `CLOCK-FACE-SPEC.md`: a time a fix or zone change replaces
types in again by cell reveal (hours, minutes, seconds over 180 ms, the date line over 160 ms
from 120 ms); a tick never animates the digits.

## Interpretations where the spec left room

Tell the owner if any of these should change.

- **Drag routing on a face.** At 16 px a drag with downward travel at least twice its sideways
  travel opens the panel. Otherwise it goes to the pager, which turns pages only for drags at
  least as sideways as vertical. So a drag that is mostly downward but under two to one, or
  mostly upward, does nothing.
- **Drag routing on the panel.** At least as sideways as vertical scrolls the grid; otherwise
  upward closes and downward does nothing.
- **Open and close use the pager's physics:** follow 1:1, release commits past a quarter of the
  height or faster than 600 px/s, and the remainder eases exponentially (35 ms time constant).
- **The panel's entry starts when the sheet reaches rest**, which the easing reaches only within
  1 px. The last few pixels of creep delay the entry by about 200 ms after the panel looks
  settled; the pager's pages behave the same. The concept starts the entry on arrival. Not
  changed yet.
- **Grid snap** eases out cubically over 160 ms to 0 or the end, or one step in the flick's
  direction past 600 px/s.
- **Second-level entry:** the 96 px icon builds a row every 30 ms, the hint reveals over 160 ms.
- **Picker stepping:** one row per 40 px of travel; a release faster than 500 px/s keeps stepping
  with its speed decaying (200 ms time constant) until under 60 px/s or at the list's end.
- **Picker order.** Nearest first ranks by distance from the last GNSS fix to each zone's
  reference point from IANA `zone.tab` and `zone1970.tab`; every zone but the 26 `Etc` zones has
  one, and those come last. With no fix yet, A to Z; the hint says which.
- **Clear confirm:** a release counts when the handle's middle is over the target, i.e. its left
  edge at 308 or more of 340.
- **Values with no reading:** BATTERY shows `--` until the power controller answers, then `NONE`
  without a battery. ZONE shows `--` when a zone is set but its offset is unknown (stopped or
  unreadable clock and no remembered offset), `NO ZONE` when none is set. The device page's
  POWER row reads `USB  CHARGING`, `USB`, `CHARGING` or `BATTERY`.
- **Firmware version** is the crate's version string, `0.1.0` today.
- **Opening a cell** is a hard cut, as specified; so is returning. Returning keeps the grid's
  scroll.

## How closely it matches the concepts

Compared side by side with the concept images: panel at rest, at the end, mid-scroll and pulling
over the clock; brightness; device page at top and end; clear confirm; picker steps 1 and 2. They
match in position, size and colour to within antialiasing, apart from fixtures (the concept's
zone lists are illustrative, so step 2's neighbours and counts differ). The entry, from a
recording at 20 ms, follows `panel-anim-entry-frames.png` in order and timing once it starts;
see the start delay above.

Not compared: the close sequence and the face's re-entry frame by frame, and anything on the
panel itself beyond the owner's use of it.

## Costs known so far

- **Draw times on the board are not measured** for any of the new screens.
- **Redraw regions:** the settled grid redraws only a cell whose reading changed, and the grid
  rows and markers while scrolling. Opening, closing and entry steps redraw in full. Every
  second-level screen redraws in full on any change; a finer split is not done.
- **Saving a setting holds the display about a third of a second.** Settings moved to an ekv
  database, whose every write erases a 4 KiB flash page, and the display core waits through the
  write. The first saved brightness took 331 ms. The screen freezes that long after tapping to
  keep a brightness, storing a zone, or clearing. In the backlog.
- **Flash image** grew by 96,512 bytes to 1,026,624, 6.55 % of the app partition. The full
  regular Fraktion font (for `©`) is part of that. Flash is not a constraint.

## State of the UI for a next round

- **Screens:** the clock face and the compass in the pager, and the settings panel over either,
  with the brightness editor, device page, clear confirm and zone picker under it.
- **Gestures:** sideways drag on a face turns the page; downward drag opens the panel; a cover
  goes to the clock face from anywhere and is the only cover gesture left. Taps do nothing on the
  faces.
- **Data the screens receive now** (adds to `SCREEN-DESIGN-BRIEF.md`'s tables): battery presence,
  percentage, voltage, charging, USB power; GNSS fix, satellites in use and in view, and the last
  fix's position; the display brightness; the firmware version; each zone's reference point.
- **Stored settings:** zone mode, manual zone, last automatic zone, brightness. All are on the
  panel.
- **Touch:** a held finger now stays held however still it is; before, a still finger could
  read as lifted after about 50 ms and end a drag early. Nothing in the designs needs to change
  for it.
- **The brief** (`SCREEN-DESIGN-BRIEF.md`) was written for this round and does not yet describe
  the panel as built. Read this document with it until it is refreshed.
