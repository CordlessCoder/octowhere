# Screen design brief

This brief is for a design agent designing a new screen for this device. It covers the
hardware, the screens as built, the gestures, what the renderer draws and what that costs, and
what data and settings exist. Read it with:

- [`marathon-ui-cross-project-handoff.md`](marathon-ui-cross-project-handoff.md), the design
  doctrine. It was written for an earlier product. Where it disagrees with this brief about this
  device, this brief wins.
- [`clock_face_design/CLOCK-FACE-SPEC.md`](clock_face_design/CLOCK-FACE-SPEC.md): the clock
  face, the compass's layout, the zone picker, the icon system and the motion primitives.
- [`compass-animation/COMPASS-ANIMATION-ADDENDUM.md`](compass-animation/COMPASS-ANIMATION-ADDENDUM.md):
  the compass's motion.
- [`clock-wordmark/CLOCK-WORDMARK-ADDENDUM.md`](clock-wordmark/CLOCK-WORDMARK-ADDENDUM.md): the
  OCTOWHERE wordmark on the clock face.
- [`settings-panel/SETTINGS-PANEL-SPEC.md`](settings-panel/SETTINGS-PANEL-SPEC.md): the settings
  panel and its second-level screens.
- [`settings-panel/IMPLEMENTATION-RESPONSE.md`](settings-panel/IMPLEMENTATION-RESPONSE.md) and
  [`settings-panel/SETTINGS-DESIGN-RESPONSE.md`](settings-panel/SETTINGS-DESIGN-RESPONSE.md):
  what the build interpreted in the settings round, and the design's reply. The reply's two
  changes are built.

Everything those documents specify is implemented and has been approved on the panel. Treat it
as fixed. A new design fits beside it, and changes it only as an explicit, separate proposal.
Where a spec and this brief disagree about what is built, the spec's own Decisions section and
the responses are newer.

## Hardware

The device is a round 1.75-inch touch module: a Waveshare ESP32-S3-Touch-AMOLED-1.75.

| Property | Value |
| --- | --- |
| Panel | Round AMOLED, CO5300 controller |
| Resolution | 466 × 466 framebuffer. Only the inscribed circle is visible; the square's corners are lost |
| Active area | 43.76 mm diameter, about 10.65 px/mm (270 ppi). Read from the manufacturer's drawing, not stated there |
| Glass | 48.96 mm outer diameter. The edge of the glass shades the outermost pixel or so |
| Module | 46.00 mm diameter PCB, 10.40 mm deep including the glass |
| Colour | RGB565, 65,536 colours. Smooth gradients band visibly, so use solid fills and stepped fades |
| Black | An unlit AMOLED pixel is off, not dim. Pure black is the field and the strongest contrast step |
| Brightness | Set from the stored setting at boot (10–100 %, default 120 of 255). Changes apply at once. No ambient light sensor, no sleep or always-on mode |
| Touch | CST9217 capacitive, in the same 466 × 466 coordinates. Two contacts, plus a recognised "hand covers the screen" report. No hover, no pressure. A held finger stays held however still it is |
| Buttons | A boot and a power button, on GPIO0 and GPIO10 according to the vendor's pin list, not verified on this board. The firmware reads neither |
| Sensors | 6-axis IMU (QMI8658), magnetometer (BMM350), GNSS receiver (LC76G), real-time clock (PCF85063A), battery and USB power (AXP2101) |
| Radio | LoRa (SX1272). The location mesh that will use it is designed but not built |
| Not driven | Audio codec, SD card slot. No speaker, buzzer or vibration motor is in use |

Geometry a layout must respect:

- The panel's centre is the pixel corner at (233, 233). Pixel (x, y) covers x..x+1 and y..y+1, so
  pixels 232 and 233 straddle the centre on each axis. Anything drawn symmetric about the centre
  should be symmetric about that corner.
- The perimeter ring that reads as the edge is centred on radius 231 with a 2 px stroke.
- A layout must fit the circle. Text and slabs near the top and bottom need the chord width at
  their row, not 466. Every screen keeps its ink inside radius 226, except the ring, bands, field
  rules and deliberately cropped content such as the panel's third column.

The owner judges every design on the physical panel at viewing distance. The smallest text in use
is 14 px, about 1 mm tall in capitals, and it reads. Touch targets are no smaller than about 10 mm.

## Screens as built

| Layer | Screens |
| --- | --- |
| Pager | Clock face, compass. A ring of two that wraps |
| Sheet | Settings panel, over whichever face it was opened from |
| Second level, under the panel | Zone picker (two steps), brightness editor, device page, clear-settings confirm |

- **Clock face:** as the clock face spec and the wordmark addendum describe. A time that a fix
  or zone change replaces types in again by cell reveal: the hours, minutes and seconds over
  180 ms, and the date line over 160 ms from 120 ms. A tick never animates the digits.
- **Compass:** as the clock face spec's compass section and the animation addendum describe, with
  the 66 px outlined icon. It no longer has cover-to-recalibrate, its `COVER SCREEN TO RECAL`
  hint, or the divider above it. The tilt line ends the stack. Calibration restarts from the
  panel's COMPASS cell.
- **Settings panel:** a registration grid of six cells in three columns, two in view, scrolling
  sideways: ZONE, BRIGHTNESS, COMPASS, GNSS, BATTERY, DEVICE. ZONE opens the picker and
  BRIGHTNESS the editor. COMPASS restarts calibration and closes to the compass. GNSS, BATTERY
  and DEVICE open the device page, which ends with the attribution and `CLEAR SETTINGS`.
- **Brightness:** any whole percentage from 10 to 100, set from the finger's x over the track:
  p = clamp(round(100 (x − 66) / 334), 10, 100), and the controller gets round(255 p / 100). Each
  of the ten track cells is a tenth of full. The cell the level falls in fills from its left as
  far as the level reaches.
- **Zone picker:** without a trusted date (the clock reads 1 Jan 2000 after a power loss until
  GNSS sets it), a zone is listed under every offset it keeps in a year. Rows then show `--:--`.

Captures drawn by the firmware's own code come from `crates/octowhere-ui/examples/render.rs`.
It draws the faces' stills and `panel-rest`, `panel-end`, `panel-scrolling`, `panel-pulling`,
`panel-device`, `panel-device-end`, `settings-brightness`, `settings-clear`, `picker-offset` and
`picker-zone`. `tools/ui-sim` records scenes as GIF or MP4 at 20 ms per frame, with the finger
marked: `--record settings` and `--record tour`. The captures' fixture is 13:07:42 on Thu 24 Sep
2026 in Europe/Dublin, and heading 047°, pitch +05, roll −12, calibration 54 %. None of it is a
reading.

## Gestures as built

A contact becomes a drag after 16 px of movement, and its axis is decided then. One that lifts
before that is a tap, reported where it landed. The firmware tracks one finger's position and
velocity. It sees a second contact but no gesture uses one.

| Where | Gesture | Effect |
| --- | --- | --- |
| A face | A drag at least as sideways as vertical | Turns the page |
| A face | A downward drag, with the downward movement at least twice the sideways movement | Opens the panel over the face |
| A face | Any other drag (mostly downward but under two to one, or mostly upward); any tap | Nothing |
| Panel | A tap on a cell in view | Opens it |
| Panel | A tap on a cell in the cropped column | Scrolls that column into view |
| Panel | A drag at least as sideways as vertical | Scrolls the grid, then snaps |
| Panel | An upward drag | Closes the panel |
| Panel | A downward drag | Nothing |
| Picker, device page | A vertical drag | Steps the list (picker) or scrolls it (device page) |
| Brightness | A tap in the top cap, or anywhere else | Cancels, or keeps the level |
| Brightness, clear confirm | A horizontal drag | Sets the level, or moves the handle (only if the drag starts on the handle) |
| Any screen | Cover | Goes to the clock face, discarding any edit in progress. From the panel it closes with the released-drag motion, and the face under it becomes the clock. It is the only cover gesture |

Physics:

- **Page and sheet:** follow 1:1. A release commits past a quarter of the width or height, or
  faster than 600 px/s. The remainder, or the way back, eases out cubically over 160 ms
  whatever the distance, and the page's entry starts on the frame it lands.
- **Grid snap:** a cubic ease-out over 160 ms, to either end, or one step in the flick's
  direction past 600 px/s.
- **Picker list:** one row per 40 px of travel. A release faster than 500 px/s keeps stepping,
  its speed decaying with a 200 ms time constant until it is under 60 px/s or at the list's end.

There is no long press, double tap or edge swipe. Each is small firmware work on the gesture
tracker, but it is a proposal the design must name.

## Data a screen can show

A screen shows only data the system has: no invented readings, and no status word without a
condition behind it. A design that needs data the screens do not receive yet must say so, since
plumbing it is firmware work. There are three grades.

1. **Reaches the screens today:**
   - The time, as UTC from the real-time clock, read about every 250 ms. With it: whether GNSS
     has set the clock since boot, and whether the clock stopped since it was last set.
   - The time zone in force: its IANA name, UTC offset, daylight saving flag and abbreviation,
     and whether it was chosen automatically or by hand. The screens convert to local time
     themselves. The offset the zone last had while the clock was trusted is kept for the
     clock's fault states.
   - Built into the screens' code: the zone table (all 444 zones' names, rules and reference
     points), and the tzdata and boundary-data releases.
   - The compass: `live`, calibration percentage, heading in tenths of a degree (magnetic),
     pitch and roll in whole degrees, and a `disturbed` flag.
   - Battery: presence, percentage and voltage. Power: charging, and whether USB is present.
     Until the power controller first answers, the battery reads as unknown (`--`).
   - GNSS: fix, satellites in use and in view, and the last fix's position.
   - The display brightness, and the firmware version (`0.1.0`, the crate's version string).
   - Touch contacts and the cover report.
2. **Known to the firmware, not passed to the screens:** GNSS time to the millisecond, fix
   quality, and when the clock was last set from GNSS. Also the compass calibration's internals.
3. **Does not exist:** sleep, screen timeout, always-on or raise to wake; a 12-hour clock (ruled
   out by the clock spec); units, languages, sounds or vibration; pairing, the location mesh,
   Wi-Fi or Bluetooth; alarms, timers, step counting and notifications.

## Settings

| Setting | Stored | On the panel |
| --- | --- | --- |
| Time zone mode, automatic or manual | yes | ZONE, then the picker |
| Manual zone | yes | the picker |
| Last zone GNSS found (automatic mode's memory) | yes | shown through ZONE |
| Brightness | yes | BRIGHTNESS |
| Compass calibration | no. It is learned at run time and restarted from the COMPASS cell | COMPASS |

- Settings live in an ekv database in flash. Clearing erases all four stored keys. The panel
  spec has the device return to its defaults: automatic zone and brightness 120.
- Adding a setting is a new key.
- Every write erases a 4 KiB flash page, and the display core waits through it. The first saved
  brightness held the screen for 331 ms. The same freeze follows tapping to keep a brightness,
  storing a zone, or clearing. The panel with the new value reaches the screen before the
  write starts, so the pause follows the confirmation. Removing it is in the firmware
  backlog.

## What the renderer draws

The firmware is `no_std` Rust drawing into an RGB565 framebuffer. Every draw goes through a
target that takes antialiased coverage a row at a time and blends it with what is already there.

- **Fonts:** three faces, each at any pixel size and antialiased.
  - Marathon Shapiro Wide 65, the display face. Printable ASCII.
  - PP Fraktion Mono Regular, the full font, including `©`.
  - PP Fraktion Mono Bold. Printable ASCII and `°`.
  - Any other glyph in these fonts' full files can be added, at a cost in flash, not draw time.
    `assets/` also holds PP Fraktion Sans (Light, Bold and italics), the Mono italics, and a
    trial of KH Interference. Adding a face costs flash and needs its licence checked.
- **Upright text:** aligned in a box, or pen on a baseline. Ink bounds can be measured before
  drawing, so a design can centre by ink and tests can check that text fits.
- **Quarter-turned text:** a transpose and a flip of the upright raster. It costs about what
  upright text costs. The wordmark uses it.
- **Rotated text at any angle:** a string centred on a point, as the compass's `N E S W`. It is
  the costliest text.
- **Solid rectangles:** exact, and the cheapest thing to draw.
- **The 5 × 5 icon system:** outlined, at any module size. The frame is a quarter of the module
  (at least 2 px), black inside, and the modules are in the state colour. In use: 96 px (clock
  and second-level screens), 66 px (compass and panel cells). The glyphs are in the specs'
  symbol tables.
- **A band clipped to the page's circle:** full-width rows whose ends meet radius 232 with an
  antialiased pixel, as the clock's band and the field rules.
- **Antialiased ring:** a full circle between two radii. There is no arc or partial ring yet.
- **Antialiased polygons:** any polygon, and curves once flattened. A fill needs about 5 bytes of
  scratch per pixel of its bounding box, from a 252 KiB heap, so very large shapes do not fit.
- **Motion, all in use:** a colour fade toward black in steps; the icon row build; the cell
  reveal of text; the rule draw-out; the dial sweep. There are also three kinds of sliding
  content: the page swipe (horizontal), the sheet (vertical, a face and the panel), and the
  panel grid's scroll with its snap. The device page's list scrolls under a fixed edge. There is
  no group alpha and no transparency.

Constraints:

- **Draw order is the layering.** A later draw covers an earlier one.
- **An antialiased edge must know its background to look right.** Every screen draws on a known
  flat colour: text on `BLACK`, knockout text on its band or slab colour. Anything over an image,
  a pattern or another shape's edge is possible but slower, and a deliberate choice.
- **Symbols are built in code** from rectangles and polygons on integer geometry, never bitmaps.
- **No images, photographs or textures.**

## What drawing costs

The frame loop runs on one core and the transfer to the panel on the other, with two framebuffers
between them. A frame draws only when something changed. On a settled screen, only the parts
that changed redraw. Measured on the device, per frame:

| Frame | Draw |
| --- | ---: |
| Either face at rest | nothing drawn |
| Clock, a second ticks | about 1.5 ms |
| Clock, a minute changes (one 136 px digit) | 3.5–3.7 ms |
| Clock, a step of its entry while the ring fades | 8.2–9.5 ms |
| Clock, a step of its entry, after the ring fade | 0.9–4.4 ms |
| Compass, tilt changes by a degree | 1.7 ms |
| Compass, a step of the dial sweep | 7–12 ms |
| Compass, heading changes by a degree | about 13 ms |
| Clock drawn in full | 17.5–22.6 ms |
| Compass drawn in full | 13–23 ms |

The wordmark is about 0.3 ms of a full clock draw. The screens of the settings round are not
measured.

- A full redraw happens during a page swipe or sheet travel. A fade of the perimeter ring
  redraws the ring, and recolouring the clock's band redraws the band's rows.
- A sparse change costs more per pixel than a compact one. The framebuffers are in external
  memory, reached through a cache in 64-byte lines, so each row a change touches costs at least
  a line. The ring fade touches every row twice, which is why it costs more than its 8,000
  pixels suggest.
- The settled panel redraws only a cell whose reading changed. While scrolling it redraws the
  grid rows and markers. Opening, closing and entry steps redraw in full. Every second-level
  screen redraws in full on any change.
- Cost grows with the area a change touches and with how many separate places it touches. Each
  separate region sent to the panel costs about as much as 1,000 more pixels.
- A new screen redraws in full on every change until its own change tracking is written. The
  design should say which elements change and how often, as the specs' change tables do.
- The flash image is 1,026,624 bytes, 6.55 % of the app partition. Flash is not a constraint.

## Owner decisions that bind later screens

- Colour tokens only. `RED` is only for a fault.
- `BLUE` marks a live, valid reading on a status icon. `GRAY` as an icon colour marks a value that
  is valid but unconfirmed. `LIME` and `PURPLE` are unused.
- Every icon is outlined. The bands and slabs carry the colour where a state must be loud.
- Filled means reading, outlined means editing. A mode in force is filled `GRAY`.
- Elements that come and go keep their space, so nothing else moves between states.
- Wording is shortened before it is shrunk.
- Fades may look stepped. Durations are not stretched to add frames.
- Digits never animate on a tick. A value that a new fix or zone replaces may type in again.
  Accents build after the page settles and leave with the swipe, driven by its offset. A fault
  shows at once.
- Nothing on a face can be changed by an accidental touch. Faces take no taps.
- A destructive action takes two stages, the second a drag. It is marked `ORANGE`, never `RED`.
- Cover means "go to the clock face" everywhere, discarding any edit in progress. It is the only
  cover gesture.
- Settings live on the panel. A new setting is a new cell or lives under an existing one.

## Colour

Colours come only from these tokens. Each is a swatch from the reference board except `BLACK`.

| Token | Hex | Role |
| --- | --- | --- |
| `LIME` | `#C0FE04` | Unused |
| `RED` | `#F24723` | Faults only |
| `ORANGE` | `#F1710D` | Attention: calibrating, interference, a stopped clock, the clear confirm; the compass's `N` |
| `PURPLE` | `#5500E4` | Unused |
| `BLUE` | `#409DE4` | A live, valid reading's status icon |
| `GRAY` | `#888E98` | Frames, rules, minor marks, captions, secondary text, a mode in force, an unconfirmed value |
| `WHITE` | `#D2D3D6` | Primary text, major marks, neutral bands and slabs, field rules |
| `BLACK` | `#000000` | The field; knockout text and symbols on saturated fills |

A new colour is allowed if it comes from the reference board. Its unused swatches:

| Hex | Appearance |
| --- | --- |
| `#FFFAC3` | pale yellow |
| `#ECDB0B` | yellow |
| `#01E67C` | green |
| `#81EBB1` | pale green |
| `#E8337C` | deep pink |
| `#B32BE5` | violet |
| `#31333B` | dark neutral |
| `#1E1F24` | near-black neutral |

`BLACK` stays pure rather than taking the board's near-black, because an unlit pixel is a
contrast step no near-black reaches. The two neutrals above would read as lit grey on this panel.

## Rules that bind the code

From the doctrine, as the firmware applies it:

- No invented data. Everything shown comes from the data above or is a fixed label. Synthetic
  values are for exploration only and must look synthetic.
- `RED` means a fault, never a data series, an idle state or a prompt.
- Saturated fills carry black knockout text and black symbols.
- Symbols are primitives on integer geometry, in code.
- A control implies a capability. Use only settings with an implementation path, or name the
  capability as a proposal.

## Starting a new round

The owner sets the round's question. The layers above are where a new screen can live: a page in
the pager, a sheet like the panel, or a second-level screen under the panel. Say which one, and
what it adds to the gestures table.

The earlier rounds worked well with:

1. A concept image of every state at 466 × 466, on a pure black field, with the circle shown.
   That includes each value state, each editing state, and the screen's entry and exit.
2. A specification an implementer can build from without guessing. It covers each element's
   position in panel pixels, face, size and colour token; the states, their conditions and
   precedence; what changes when, and how it animates; the gestures and the regions they act on;
   and the flows between screens.
3. A list of what the design needs that the firmware does not have yet: data, gestures, glyphs,
   primitives, settings. Each item is firmware work, and the owner decides on it.
4. A decisions section for what the owner settles during the round.

The implementer comes back with questions where the specification leaves a choice open. The owner
reviews the result in the desktop simulator, which runs the firmware's own drawing code, then on
the panel.
