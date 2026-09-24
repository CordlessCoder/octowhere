# Screen design brief

For a design agent designing a new screen for this device. It describes the hardware, the two
screens the firmware has, what the renderer draws and what that costs, and what data and settings
exist. Read it with:

- [`marathon-ui-cross-project-handoff.md`](marathon-ui-cross-project-handoff.md), the design
  doctrine. It was written for an earlier product, so where it disagrees with this brief about
  this device, this brief wins.
- [`clock_face_design/CLOCK-FACE-SPEC.md`](clock_face_design/CLOCK-FACE-SPEC.md), the approved
  design of the clock face, the compass's current layout and the zone picker, with its images in
  `clock_face_design/images/`.
- [`compass-animation/COMPASS-ANIMATION-ADDENDUM.md`](compass-animation/COMPASS-ANIMATION-ADDENDUM.md),
  the approved motion of the compass.
- [`clock-wordmark/CLOCK-WORDMARK-ADDENDUM.md`](clock-wordmark/CLOCK-WORDMARK-ADDENDUM.md), the
  OCTOWHERE wordmark on the clock face and its step in the entry.

Both screens are implemented as those documents specify and approved on the panel. Treat them as
fixed: a new design fits beside them, and changes them only as an explicit, separate proposal.

This round designs the **settings panel**. The section "Designing the settings panel" collects
what bears on it.

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
| Brightness | The firmware sets 120 of 255 at boot and never changes it. The controller takes any level from 0 to 255 at run time. No ambient light sensor, no sleep or always-on mode |
| Touch | CST9217 capacitive, in the same 466 × 466 coordinates. Two contacts, plus a recognised "hand covers the screen" report. No hover, no pressure |
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
  their row, not 466. Both approved screens keep all ink but the ring and the band inside
  radius 226.

The owner judges every design on the physical panel at viewing distance. The smallest text in use
is 14 px, about 1 mm tall in capitals, and it reads.

## Screens and navigation

The firmware has two screens in a ring, which wraps: the clock, then the compass. Everything else
was removed as preliminary.

- A horizontal drag moves between them. The page follows the finger and, on release, settles to
  the next page or back. Past a quarter of the width, or on a flick faster than 0.6 px/ms, it moves
  on. During a drag both pages draw, each shifted and clipped at the other's edge, so a layout must
  read when a vertical edge cuts through it anywhere.
- A contact becomes a drag after 16 px of movement; one that lifts before that is a tap, reported
  where it landed. The firmware tracks one finger's position and velocity for gestures. It sees a
  second contact but no gesture uses one.
- The clock face takes no touch at all besides the page drag. The owner moved zone choice off the
  face because a tap there was too easy to trigger by accident.
- The compass takes one gesture besides the page drag: a hand covering the settled page restarts
  calibration, once per hand.
- There is no long press, double tap or edge swipe yet. Each is small firmware work on the
  existing gesture tracker, but it is a proposal the design must name.

## Designing the settings panel

The owner wants a settings panel, and the zone picker in the clock face spec opens from it. How
the panel is reached and what else it holds are open. That is this round's question.

### What the panel must connect to

The zone picker is designed (spec, "Zone picker"). It assumes a settings panel:

- `CANCEL` on its first step, and covering the screen on either step, return to the panel with
  nothing stored.
- Storing a manual zone or automatic returns to the panel.
- "A horizontal drag does whatever it does on the settings panel. If it leaves, the picker is
  discarded without storing anything."
- It follows the rule the owner set for every future setting: filled means reading, outlined means
  editing. A mode in force is filled `GRAY`.

The panel's design owns those connections: what the panel shows for the zone setting, and what a
horizontal drag means there.

### Settings that have an implementation path

Each has a way to work today or with small firmware work. The firmware keeps settings in flash
across restarts; adding one is a new key. Saving pauses the display for a couple of milliseconds,
which a design need not plan around.

| Setting | State in firmware |
| --- | --- |
| Time zone mode: automatic or manual | Stored and used. Nothing on screen can change it yet |
| Manual zone | Stored and used. Chosen through the zone picker |
| Display brightness | The controller accepts 0 to 255 at run time. Fixed at 120 today; storing and applying a chosen level is small work |
| Restart the compass calibration | Exists, as the cover gesture on the compass. A control elsewhere would call the same thing |
| Clear stored settings | The settings live in one flash partition that can be erased. Small work; the design decides whether it deserves a control |

The owner has decided that the panel has a screen brightness slider. Its range, steps, and how
dragging it works alongside the page swipe are for the design. The controller applies a new level
at once, so the slider can show each level live while it is dragged.

### Information the panel could carry

Not settings, but a settings panel is where such things usually live. Grades as in "Data a screen
can show" below.

- **Required: the time zone data's attribution.** The zone boundaries are derived from
  OpenStreetMap data and licensed under the Open Database License, which asks for attribution
  where the data is used. The owner wants it on the device. A text that covers it: "Time zone
  boundaries: timezone-boundary-builder 2026d, © OpenStreetMap contributors, ODbL 1.0. Zone rules:
  IANA time zone database 2026d." The two release names are data the screens have, so the text
  stays current when the data is rebuilt. It can be shortened, but must keep the copyright holder
  and the licence.
- Firmware version: the build carries a version number, `0.1.0` today. A build date or source
  revision would need adding.
- Battery: percentage, voltage, charging, and USB power present (grade 2).
- GNSS: fix, satellites in use, and position (grade 2).

### What does not exist

Do not design controls for these. Name one as a proposal if the panel needs it.

- Sleep, screen timeout, always-on or raise to wake.
- A 12-hour clock. The spec rules it out.
- Units, languages, sounds or vibration.
- Pairing, the location mesh, Wi-Fi or Bluetooth.

### Open questions the design settles

1. How the panel is reached, and how it is left. It could be a page in the ring, somewhere reached
   from a face by a gesture the faces do not use yet, or a button. Weigh accidental entry: the
   owner removed taps from the clock face for that reason. The faces themselves are fixed, so a
   design that adds an entry point to one says exactly what it adds.
2. What the panel holds and in what order, from the lists above.
3. How each setting shows its current value, and how editing one opens and closes. The zone
   setting opens the designed picker; brightness is a slider.
4. What a horizontal drag does on the panel, and so in the picker.
5. Whether the panel needs more than one level, and how its information sits beside its settings.

## Data a screen can show

A screen shows only data the system has: no invented readings, and no status word without a
condition behind it. A design that needs data the screens do not receive yet must say so, since
plumbing it is firmware work. Three grades:

1. **Reaches the screens today:**
   - The time, as UTC from the real-time clock, read about every 250 ms, with whether GNSS has
     set the clock since boot and whether the clock stopped since it was last set.
   - The time zone in force: its IANA name, UTC offset, daylight saving flag and abbreviation,
     and whether it was chosen automatically or by hand. The screens convert to local time
     themselves. The offset the zone last had while the clock was trusted is kept for the
     clock's fault states.
   - The compass: `live`, calibration percentage, heading in tenths of a degree (magnetic),
     pitch and roll in whole degrees, and a `disturbed` flag.
   - Touch contacts and the cover report.
   - Built into the screens' own code: the zone table, with all 444 zones' names and rules, and
     the releases of the tzdata and boundary data. Listing the offsets in force today, or the
     zones at one offset, as the picker needs, is code still to write over data that is there.
2. **Known to the firmware, not passed to the screens:**
   - Battery percentage, voltage, charging, and USB power.
   - The GNSS fix, position, time to the millisecond, satellites in view and in use, and fix
     quality. When the clock was last set from GNSS.
   - The compass calibration's internals, and the firmware version.
3. **Does not exist:** see the list above, plus alarms, timers, step counting and notifications.

## What the renderer draws

The firmware is `no_std` Rust drawing into an RGB565 framebuffer. Every draw goes through a
target that takes antialiased coverage a row at a time and blends it with what is already there.

- **Fonts:** three faces, all at any pixel size, antialiased.
  - Marathon Shapiro Wide 65, the display face. Printable ASCII.
  - PP Fraktion Mono Regular. Printable ASCII.
  - PP Fraktion Mono Bold. Printable ASCII and `°`.
  - Any other glyph in these fonts' full files can be added, at a cost in flash, not draw time.
    `assets/` also holds PP Fraktion Sans (Light, Bold and italics), the Mono italics, and a
    trial of KH Interference. Adding a face costs flash and needs its licence checked.
- **Upright text:** aligned in a box, or pen on a baseline. Ink bounds can be measured before
  drawing, so a design can centre by ink and tests can check that text fits.
- **Rotated text:** a string centred on a point, turned to any angle, as the compass's `N E S W`.
  The costliest text.
- **Solid rectangles:** exact and the cheapest thing to draw.
- **The 5 × 5 icon system:** outlined, at any module size: a frame a quarter of the module thick
  (at least 2 px), black inside, and the modules in the state colour. The clock uses 96 px, the
  compass 66 px. The glyphs in use are in the spec's "Symbols" and the compass's table there.
- **A band clipped to the page's circle:** full-width rows whose ends meet radius 232 with an
  antialiased pixel, as the clock's band and the picker's field rules.
- **Antialiased ring:** a full circle between two radii. No arc or partial ring yet.
- **Antialiased polygons:** any polygon, and curves once flattened. A fill needs about 5 bytes of
  scratch per pixel of its bounding box, from a 252 KiB heap, so very large shapes do not fit.
- **Motion primitives, all in use:** a colour fade toward black in steps; the icon row build; the
  cell reveal of Mono text; the rule draw-out; the dial sweep. The spec and the addendum define
  them. There is no group alpha, no transparency and no sliding content besides the page swipe.

Constraints:

- **Draw order is the layering.** A later draw covers an earlier one.
- **An antialiased edge must know its background to look right.** Both screens draw everything on
  a known flat colour: text on `BLACK`, knockout text on its band or slab colour. Anything over an
  image, pattern or another shape's edge is possible but slower, and a deliberate choice.
- **Symbols are built in code** from rectangles and polygons on integer geometry, never bitmaps.
- **No images, photographs or textures.**

## What drawing costs

The frame loop runs on one core and the transfer to the panel on the other, with two framebuffers
between them. A frame draws only when something changed, and on a settled screen only the parts
that changed redraw. Measured on the device, per frame:

| Frame | Draw |
| --- | ---: |
| Either screen at rest | nothing drawn |
| Clock, a second ticks | about 2.3 ms |
| Clock, a minute changes (one 136 px digit) | 5.3–5.9 ms |
| Clock, a step of its entry, after the ring fade | 1.4–4.8 ms |
| Compass, tilt changes by a degree | 1.7 ms |
| Compass, a step of the dial sweep | 7–12 ms |
| Compass, heading changes by a degree | about 13 ms |
| Clock drawn in full | 20–26 ms |
| Compass drawn in full | 13–23 ms |

The clock's wordmark was added after these measurements. Its reveal steps and its share of a full
clock draw are not measured yet.

- A full redraw happens during a page swipe, a fade of the perimeter ring, and a change that
  recolours a large area such as the clock's band.
- Cost grows with the area a change touches and with how many separate places it touches. Each
  separate region sent to the panel costs about as much as 1,000 more pixels.
- A new screen redraws in full on every change until its own change tracking is written. That is
  firmware work the design does not plan around, but the design should say which elements change
  and how often, as the spec's "What changes, and how often" table does.

## The two screens

Captures in `screen-captures/`, drawn by the firmware's own code. The square's corners are off
the panel. The fixture is 13:07:42 on Thu 24 Sep 2026 in Europe/Dublin, and heading 047°, pitch
+05, roll −12, calibration 54 %. None of it is a reading.

| File | Shows |
| --- | --- |
| `clock-gnss.png`, `clock-rtc.png`, `clock-manual.png` | Local time, set from GNSS; unconfirmed since boot; a zone chosen by hand |
| `clock-stopped.png`, `clock-no-zone.png`, `clock-no-data.png` | The clock's fault and unknown states |
| `clock-longest-zone.png` | The longest zone name, with a manual zone |
| `clock-entering.png`, `clock-marking.png`, `clock-swiping.png` | 200 ms and 380 ms into the entry; part way through a drag out |
| `compass-heading.png`, `compass-interference.png`, `compass-calibrating.png`, `compass-top-edge-up.png`, `compass-no-data.png` | The compass's states |
| `compass-entering.png`, `compass-sweeping.png`, `compass-swiping.png` | 150 ms into the entry; the dial half swept; part way through a drag out |

The spec and the addenda give every position, face, size, colour, state and timing. Owner
decisions from both rounds that bind later screens:

- Colour tokens only, and `RED` only for a fault.
- `BLUE` marks a live, valid reading on a status icon. `GRAY` as an icon colour marks a value that
  is valid but unconfirmed. `LIME` and `PURPLE` are not used on either screen.
- Every icon is outlined. The bands and slabs carry the colour where a state must be loud.
- Filled means reading, outlined means editing. A mode in force is filled `GRAY`.
- Elements that come and go keep their space, so nothing else moves between states.
- Wording is shortened before it is shrunk.
- Fades may look stepped; durations are not stretched to add frames.
- Digits never animate. Accents build after the page settles and leave with the swipe, driven by
  its offset. A fault shows at once.
- Nothing on a face can be changed by an accidental touch.

## Colour

Colours come only from these tokens. Each is a swatch from the reference board except `BLACK`.

| Token | Hex | Role |
| --- | --- | --- |
| `LIME` | `#C0FE04` | Unused on the current screens |
| `RED` | `#F24723` | Faults only |
| `ORANGE` | `#F1710D` | Attention: calibrating, interference, a stopped clock; the compass's `N` |
| `PURPLE` | `#5500E4` | Unused on the current screens |
| `BLUE` | `#409DE4` | A live, valid reading's status icon |
| `GRAY` | `#888E98` | Frames, minor marks, captions, secondary text, a mode in force, an unconfirmed value |
| `WHITE` | `#D2D3D6` | Primary text, major marks, neutral bands and slabs |
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

## Deliverable

The earlier rounds worked well with:

1. A concept image of every state at 466 × 466, on a pure black field, with the circle shown.
   For the panel, that includes each setting's value states and its editing, and the panel's
   entry and exit.
2. A specification an implementer can build from without guessing: each element's position in
   panel pixels, face, size and colour token; the states, their conditions and precedence; what
   changes when, and how it animates; the gestures and the regions they act on; and the flows
   between the faces, the panel and the picker.
3. A list of what the design needs that the firmware does not have yet: data, gestures, glyphs,
   primitives, settings. Each item is firmware work, and the owner decides on it.
4. A decisions section for what the owner settles during the round.

The implementer comes back with questions where the specification leaves a choice open. The owner
reviews the result in a desktop simulator that runs the firmware's own drawing code, then on the
panel.
