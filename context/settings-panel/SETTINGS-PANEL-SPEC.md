# Settings panel specification

This is the settings panel, reached from either face by a downward drag. It's a registration
grid of six cells in three columns that scrolls sideways, two columns in view at a time. The
zone picker (clock face spec, "Zone picker") opens from it, as do the brightness editor, the
device page and the two-stage clear.

Read it with `SCREEN-DESIGN-BRIEF.md` and `clock_face_design/CLOCK-FACE-SPEC.md`. The motion
primitives, cell reveal, icon row build and rule draw-out, are defined in the clock face spec
and used here unchanged.

The images are in `images/`, all at 466 × 466, drawn by a concept renderer (Pillow, 4×
oversampled), not the firmware:

| File | Shows |
| --- | --- |
| `panel-rest.png`, `panel-end.png`, `panel-scrolling.png` | Columns 1–2 in view, columns 2–3 in view, part way through a scroll |
| `panel-manual.png`, `panel-no-zone.png`, `panel-calibrated.png`, `panel-compass-no-data.png`, `panel-no-fix.png` | Value states |
| `panel-device.png`, `panel-device-end.png` | The device page, at the top and scrolled to the end |
| `settings-brightness.png` | The brightness editor, approved |
| `settings-clear.png`, `settings-clear-drag.png` | The clear confirm, stage 2, approved |
| `panel-pulling.png`, `panel-closing.png` | Part way through opening over the clock, and part way through closing |
| `panel-anim.gif`, `panel-anim-slow4x.gif` | Open, entry, scroll to the end and back, close, the clock's re-entry |
| `panel-anim-entry-frames.png` | The panel's entry, a frame every 20 ms |
| `panel-overview.png` | The panel's stills on one sheet |

Fixture: zone automatic, IST; brightness 120 of 255 (47 %); compass calibration 54 %; firmware
`0.1.0`. The battery (87 %, 4.02 V, charging, USB) and GNSS (fix, 9 of 14 satellites) values are
synthetic: that data does not reach the screens yet.

## Assumptions

- BATTERY and GNSS need their data plumbed to the screens. The owner chose to do that, so the
  grid has six cells and scrolls.
- Tapping COMPASS restarts calibration and switches to the compass.
- Clearing settings erases the one settings partition. It holds the zone mode, the zone chosen
  by hand, the last zone GNSS found, and the brightness. Compass calibration is not stored.
- Frames every 20 ms, as for the faces. Nothing here has been measured.

## Structure and flows

```
clock ─┐                       ┌─ ZONE ──────── zone picker (steps 1, 2)
       ├─ drag down ─ PANEL ───┼─ BRIGHTNESS ── brightness editor
compass┘  drag up (back to   ├─ COMPASS ───── restarts calibration, closes to the compass
          the face it came     ├─ GNSS ─┐
          from); cover: clock  ├─ BATTERY ┼──── device page ── CLEAR SETTINGS ── drag to confirm
                               └─ DEVICE ─┘
```

- The panel is one layer, over whichever face it was opened from. Closing it returns to that
  face, which then runs its own entry sequence.
- Every second-level screen returns to the panel, at the panel's same scroll position, on its
  top-left button.
- A cover on any screen goes to the clock face. On an editor it first discards the edit, as
  `CANCEL` does: nothing is stored or cleared.
- Storing a zone, keeping a brightness or completing a clear also returns to the panel.

## Panel layout

All positions are in panel pixels. c is the column (0, 1, 2), s is the scroll (0 at rest with
columns 0–1 in view, 1 at the end with columns 1–2 in view), and the cell's left edge is
x_L = 87 + 146 (c − s).

| Element | Position | Treatment |
| --- | --- | --- |
| Ring | radius 230–232 | `GRAY` |
| Title | centred by ink, ink top 38 | `SETTINGS`, Marathon Shapiro Wide 65, 16 px, `WHITE` |
| Horizontal rules | rows 72, 218 and 364; from the grid's left edge to its right edge (x_L of column 0 to x_L + 146 of column 2) | 1 px `GRAY` |
| Vertical rules | x = 87 + 146 (k − s) for k = 0 to 3, rows 72–364 | 1 px `GRAY` |
| Cells | rows 73–217 (top row) and 219–363 (bottom row), each 145 wide between rules | Cell i (0–5) is column i div 2, row i mod 2 |
| Index | pen x_L + 10, ink top at the cell's top + 9 (rows 82, 228) | `01`–`06`, Fraktion Mono Regular 14 px, `GRAY` |
| Icon, 66 × 66 | x_L + 70, rows 82–147 and 228–293 | Outlined, 10 px modules, 8 px padding, 3 px frame |
| Name | pen x_L + 10, ink top at the cell's top + 100 (rows 173, 319) | Mono Bold 14 px, `GRAY` |
| Value line | ink left x_L + 10, centred on the cell's top + 128 (rows 201, 347) | Mono Regular 16 px, colour per the value table |
| Tag | in the value line, before the value: 18 px tall, as wide as its text's advances plus 10 px | Filled `GRAY`, Mono Bold 14 px `BLACK`, centred. The value starts 6 px after it |
| Column markers | three 8 × 8 squares at x 215, 229 and 243, rows 386–393 | Columns in view filled `WHITE`; the other outlined, 2 px `GRAY` |
| Hint | centred by ink, ink top 406 | `DRAG UP TO CLOSE`, Mono Regular 14 px, `GRAY` |

- Everything is clipped to the page's circle, radius 232. At rest the third column is cropped by
  the edge, which shows that the grid scrolls.
- All ink in the columns in view stays inside radius 226, at both rest positions. A script
  checked this, and it correctly flags the cropped column as outside.
- The widest value line, a `MANUAL` tag and an offset abbreviation such as `+0530`, is 112 px.
  The cell has 126 px between its inset and its right rule.

## Cells and their values

| i | Name | Glyph | Icon colour | Value | Value colour | Tag |
| --- | --- | --- | --- | --- | --- | --- |
| 01 | ZONE | offset bracket `11011 10001 00100 10001 11011` | `WHITE` | the abbreviation (`IST`), or `NO ZONE` | `WHITE`; `GRAY` for `NO ZONE` | `AUTO` or `MANUAL` |
| 02 | BRIGHTNESS | ramp `00001 00011 00111 01111 11111` | `WHITE` | the level, `47%` | `WHITE` | none |
| 03 | COMPASS | calibrating `01110 10001 10000 10001 01110` | `ORANGE` under 100 %, `WHITE` at 100 % | `CAL 54%` | as the icon | none |
| 03 | COMPASS, not live | no data `11100 11000 00100 00011 00111` | `RED` | `NO DATA` | `RED` | none |
| 04 | GNSS | GNSS `00100 01010 10101 01010 00100` | `BLUE` with a fix, `WHITE` without | `FIX  9` (satellites in use), or `NO FIX` | `WHITE`; `GRAY` for `NO FIX` | none |
| 05 | BATTERY | battery `01110 11111 10001 11111 11111` | `WHITE` | the percentage, or `NONE` with no battery | `WHITE`; `GRAY` for `NONE` | `USB` while USB power is present |
| 06 | DEVICE | `i` `00100 00000 01100 00100 01110` | `WHITE` | the firmware version | `WHITE` | none |

- The glyphs for zone, calibration, no data and GNSS are the canonical ones from the faces. The
  ramp, battery and `i` are new.
- `RED` appears only for the compass not being live, which is the compass's own fault state.
- A tag is a mode in force, filled `GRAY`, as on the clock's plate.
- The brightness value is the level as a percentage of 255, rounded, so the firmware's 120 shows
  as `47%`.

## Brightness editor

As approved in `settings-brightness.png`. Layout, as for the zone picker:

| Element | Position | Treatment |
| --- | --- | --- |
| Hint | centred by ink, ink top 48 | `DRAG TO SET BRIGHTNESS`, Regular 14 px `GRAY` |
| `CANCEL` | box x 72–181, rows 104–142 | As the picker's |
| Icon, 96 × 96 at x 262 | rows 90–185 | Ramp glyph, `WHITE`, outlined |
| Field | rules at rows 198–201 and 314–317, locators at x 48 and 400 | As the picker's |
| Level | ink left 62, ink bottom 262 | `50%`, Mono Bold 56 px, `WHITE` |
| Track | ten cells, x 66–400, rows 276–299, 6 px apart | Cells up to the level filled `WHITE`; the rest outlined, 2 px `GRAY` |
| Scale | `10` ink left 66 and `100` ink right 400, ink top 326 | Regular 14 px `GRAY` |
| Hint 2 | centred by ink, ink top 372 | `TAP TO KEEP`, Regular 14 px `GRAY` |

- Any whole percentage from 10 % to 100 % can be set, as the owner decided after trying ten
  steps (2026-09-24): the default 47 % was not one of them. A percentage p sets the controller to
  round(255 p / 100). The floor of 10 % keeps the screen readable enough to undo.
- Each of the ten cells is a tenth of full. The cell the level falls in fills from its left as
  far as the level reaches, so 47 % shows four cells full and the fifth 70 % filled.
- It opens showing the current level. The firmware's 120 shows as `47%`.
- A horizontal drag anywhere in the field sets the level from the finger's x over the track:
  p = clamp(round(100 (x − 66) / 334), 10, 100). Each change applies to the controller at once,
  so the panel shows the level live.
- A tap anywhere below the top cap stores the level and returns to the panel, as the owner
  decided (2026-09-24): the hint under the field said to tap, and a tap on it did nothing.
- `CANCEL` (top cap, rows 0–150) restores the level the editor opened with, and returns to the
  panel. A cover restores it too, then goes to the clock face.
- The stored level applies at boot. Today the firmware sets 120 at boot.

## Device page

The detail page for DEVICE, BATTERY and GNSS.

| Element | Position | Treatment |
| --- | --- | --- |
| Hint | centred by ink, ink top 48 | `DEVICE`, Regular 14 px `GRAY` |
| `BACK` | box x 72–181, rows 104–142 | As the picker's |
| Icon, 96 × 96 at x 262 | rows 90–185 | `i` glyph, `WHITE`, outlined |
| Scroll edge | row 195, clipped to the circle | 1 px `GRAY`. The list scrolls under it |
| Rows | from row 206, every 30 px: key Mono Bold 14 px `GRAY` at x 70, value Mono Regular 16 px `WHITE` ink right 396, a 1 px rule in `GRAY` at half level under each | `VERSION`, `BATTERY` (percentage and voltage), `POWER` (`USB`, `CHARGING`), `GNSS` (`FIX` or `NO FIX`), `SATELLITES` (in use `OF` in view) |
| Attribution | centred by ink, lines 20 px apart, after the rows | Regular 14 px `GRAY`, five lines: `TIME ZONE BOUNDARIES:`, `TIMEZONE-BOUNDARY-BUILDER 2026D`, `© OPENSTREETMAP CONTRIBUTORS`, `ODBL 1.0`, `ZONE RULES: IANA TZDATA 2026D` |
| `CLEAR SETTINGS` | box x 143–322, 44 px tall, 18 px after the attribution | Outlined, as `BACK` |

- The list is clipped to rows 196–440. A vertical drag scrolls it 1:1, from 0 to the end (108 px
  with the fixture's rows), and stops there.
- The release names in the attribution come from the zone data, so the text follows a rebuild.
  It keeps the copyright holder and the licence.
- `BACK` (top cap, rows 0–150) returns to the panel. A tap on `CLEAR SETTINGS` opens the
  confirm.

## Clear settings

Stage 1 is the `CLEAR SETTINGS` box on the device page. Stage 2 is the drag confirm, as approved
in `settings-clear.png`:

| Element | Position | Treatment |
| --- | --- | --- |
| Hint | centred by ink, ink top 48 | `DRAG ACROSS TO CLEAR`, Regular 14 px `GRAY` |
| `CANCEL` | box x 72–181, rows 104–142 | As the picker's |
| Icon, 96 × 96 at x 262 | rows 90–185 | Cross `10001 01010 00100 01010 10001`, `ORANGE`, outlined |
| Field | as the picker's | |
| Rail | x 62–403, rows 257–258 | 2 px `GRAY` |
| Target | x 340–403, rows 226–289 | Outlined, 2 px `GRAY` |
| Handle | 64 × 64, rows 226–289, left edge from x 62 to 340 | Filled `ORANGE`, with a `BLACK` arrow built from rectangles |
| Swept part | x 62 to the handle's right edge, rows 226–289 | Filled `ORANGE` |
| Warning | centred by ink, ink tops 338 and 356 | What the erase removes, Regular 14 px `GRAY`: `ERASES ZONE, LAST FIX ZONE` / `AND BRIGHTNESS` |

- A drag counts only if it starts on the handle. The handle follows the finger's x.
- Releasing with the handle in the target erases the settings. It then applies the defaults
  (automatic zone, brightness 120) and returns to the panel, which shows them.
- Releasing short of the target puts the handle back at the start, in one frame.
- `CANCEL` returns to the device page. A cover goes to the clock face, erasing nothing.
- `ORANGE` marks it as needing attention. `RED` is reserved for faults.

## Gestures

A contact becomes a drag after 16 px, and its axis is decided then.

| Where | Gesture | Effect |
| --- | --- | --- |
| Clock or compass | A drag that is downward, with the downward movement at least twice the sideways movement | Opens the panel, following the finger. Past a quarter of the height, or on a flick faster than 0.6 px/ms, it opens. Otherwise it settles back |
| Clock or compass | Any other drag, mostly sideways | Changes page, as today |
| Panel | A tap on a cell whose column is fully in view | Opens that cell (flows above). The cell is its hit region, 145 × 145 px, about 13.6 mm |
| Panel | A tap on a cell in the cropped column | Scrolls that column into view. It does not open it |
| Panel | A horizontal drag | Scrolls the grid 1:1. On release it snaps to s = 0 or 1, whichever is nearer, or one step in the flick's direction on a flick faster than 0.6 px/ms. It stops at both ends |
| Panel | A drag upward | Closes the panel, following the finger, with the same thresholds as opening |
| Panel | Cover | Closes the panel to the clock face, with the same motion as a released drag |
| Panel | A drag downward | Nothing |
| Zone picker | A horizontal drag | Nothing. The picker is a second level and does not leave by swiping. This settles the clock spec's "whatever it does on the settings panel" |
| Second level | Cover | Discards any edit and goes to the clock face |
| Compass | Cover | Goes to the clock face. It no longer restarts calibration |

The opening gesture changes both approved faces. The face keeps its page drag. The compass's
cover now goes to the clock face instead of restarting calibration.

## Motion

### Opening and closing

The panel and the face move as one vertical page swipe. The panel comes down from above while
the face moves down below it, and each is clipped at the moving edge. While opening, the panel
shows only its centre content: the icons' frames, the tags and the values. The face's accents
leave as in a sideways swipe out, driven by the vertical offset (clock face spec, "Exit").

Closing reverses it. The face returns from below with its accents hidden and runs its own entry
once it settles.

After a release, the panel, like a page, eases out cubically over 160 ms to open or closed,
whatever the distance, and the entry starts on the frame it lands (design response,
2026-09-24). A stored setting's write starts only once the panel showing the new value has
reached the screen, so the pause while it saves follows the confirmation.

### Panel entry

Times in ms from settling. Cells are staggered by index, 30 ms apart, down each column and then
across, including the cropped column.

| Accent | Start | Duration | How |
| --- | --- | --- | --- |
| Ring | 0 | 110 | Fade from `BLACK` |
| Title | 0 | 120 | Cell reveal |
| Rules | 40 | 160 | Draw-out. The horizontal rules grow from x 233 to the grid's ends. The vertical rules grow from row 218 up and down. All are clipped to the circle |
| Icon modules, cell i | 80 + 30 i | 150 | Row build |
| Index, cell i | 100 + 30 i | 60 | Cell reveal |
| Name, cell i | 120 + 30 i | 90 | Cell reveal |
| Column markers | 200 | at once | |
| Hint | 260 | 160 | Cell reveal |

It is complete at 420 ms. Values and tags are never animated.

### Panel exit

Driven by the upward offset. p runs from 0 at rest to 1 at 35 % of the height.

| Accent | Over p |
| --- | --- |
| Hint | 0–0.2, reveal run backwards |
| Column markers | gone above 0.1 |
| Names | 0.1–0.4 |
| Index | 0.15–0.4 |
| Icon modules | 0.3–0.7, rows removed bottom first |
| Rules | 0.4–0.8, draw-out run backwards |
| Title | 0.5–0.8 |
| Ring | 0.6–1.0, fade |

As on the faces, a close that starts during the entry takes the lesser of entry and exit per
accent.

### Scrolling

The grid slides with the finger, and the rules, cells and markers move together. Cells that
come into view arrive complete, with nothing rebuilt. The snap after release eases over 160 ms.

### Second level

Opening a second-level screen, and returning, is a hard cut in one frame. On opening, the
screen's 96 px icon builds by rows (150 ms) and its hint reveals, as in the picker.

## What changes, and how often

| Element | Changes | Region |
| --- | --- | --- |
| COMPASS value and icon colour | as calibration moves, and at 100 % | one value line, one icon |
| GNSS value and icon colour | as the fix or satellite count changes | one value line, one icon |
| BATTERY value and tag | as the percentage or USB changes | one value line |
| ZONE, BRIGHTNESS values | only on return from their editor | full panel redraw on return |
| Grid while scrolling | every frame of a drag or snap | the grid's rows 72–364 |
| Everything | open, close, entry and exit steps | full frame, or the cells a step touches |

## Firmware work this design needs

1. A vertical page swipe between a face and the panel: follow the finger, settle, flick. The
   face's accents follow the vertical offset.
2. The axis decision at 16 px, and the two-to-one downward rule on both faces. This is a change
   to the approved faces.
3. Sideways scrolling of the panel's grid, clipped to the page's circle, with the snap.
4. Tap routing on the panel: open a cell in view, or scroll to a cropped one.
5. Brightness: a stored level, applied at boot, and the editor's live steps.
6. The zone setting's write path from the picker, already listed in the clock spec.
7. Clear settings: the drag confirm, the erase of the settings partition, and the defaults
   after it.
8. Grade 2 data to the screens: the battery (percentage, voltage, charging, USB, presence) and
   GNSS (fix, satellites in use and in view). Also the firmware version string.
9. From the COMPASS cell: restart calibration, close the panel, and switch to the compass.
10. The device page's list clip and vertical scroll.
11. The © glyph in Fraktion Mono Regular, for the attribution.
12. Change tracking for the panel, from the table above.

## Decisions

The owner settled these on 24 Sep 2026.

1. The panel is a registration grid that scrolls sideways, two columns in view. The
   alternatives were a plain scrolling grid and a carousel of 152 px icons
   (`settings-explore.png`).
2. Brightness runs from 10 % to 100 %. The editor is approved as rendered; the owner later made
   the drag set any whole percentage instead of ten steps.
3. Clearing settings takes two stages, the second a drag to confirm. The confirm is approved as
   rendered.
4. The panel opens with a downward drag from either face and closes with an upward drag or a
   cover. Horizontal drags scroll the grid. I proposed these in discussion, and the owner did
   not object.

5. BATTERY and GNSS are in, with their data plumbed to the screens.
6. A tap on COMPASS restarts calibration and switches to the compass, with no confirm.
7. The opening drag may start anywhere on a face, not only in the top cap.
8. The clear warning names the last zone GNSS found as well: `ERASES ZONE, LAST FIX ZONE` /
   `AND BRIGHTNESS`. Calibration is not in the partition.
9. A cover on any screen goes to the clock face, discarding any edit. The compass loses its
   cover-to-recalibrate gesture, its `COVER SCREEN TO RECAL` hint and the divider above it.
   Recalibration is from the COMPASS cell only.
10. On the brightness editor, a tap anywhere below the top cap keeps the level, not only a tap
    in the field.

## Verification record

What I checked:

- Fit: in all seven panel stills, the ink of the columns in view stays inside radius 226, at
  both rest positions. The check flags the cropped column as outside, so it can fail.
- The entry frames every 20 ms, and key frames of opening and closing.
- The panel's worst-case value line (`MANUAL` with a four-digit offset abbreviation) against the
  cell's width.

What I did not check:

- Drawing by the firmware, or viewing on the panel.
- Any draw cost, the 20 ms frame rate, or the feel of the gesture thresholds.
- The real contents of the settings partition.
