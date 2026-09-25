# Screen design brief

This brief is for a design agent working on this device's screens. "This round" has the
current round's questions. The rest covers the hardware, the screens as built, the gestures, what the renderer draws and what that costs, and
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

## This round

The owner set three questions on 2026-09-25. Each changes how approved screens behave, so each
answer is an explicit proposal against the documents above.

The design's answer is `octowhere-round3-display-motion-v5/DISPLAY-AND-MOTION-SPEC.md`, which
also adds a start-up sequence and an always-on face. Its compass section and its start-up are
built; the rest is being built a part at a time.

### 1. Smoother changes between the compass's states

Today only a heading arriving animates. Everything else, interference coming and going above
all, changes in one frame. "The compass's states and changes, as built" below has the states,
the conditions behind them and every change. Things to settle:

- How each change in that section's table moves, or which stay a cut.
- The flicker. Interference is decided on every reading with no hysteresis, so near its
  threshold it switches 50 times a second, and top edge up does the same near vertical. A
  motion design cannot smooth that on its own. A hold time or hysteresis in the firmware can,
  and the design may ask for one: say how long, and which states it covers.
- The bound rules that meet this question: a fault shows at once (NO DATA), and digits never
  animate on a change of value. INTERFERENCE is attention, not a fault.

### 2. Pixel shift against burn-in

The panel is lit whenever the device is on, and much of each screen never moves: the ring, the
clock's band and wordmark, the compass's slab and captions. The owner wants the picture shifted
by a few pixels over time on every screen, whether or not an always-on face follows.

- **How the firmware would do it:** in the transfer to the panel, which copies every row
  through a small buffer already. Reading each row from an offset moves the whole picture
  at no drawing cost. A new offset costs one full transfer. The side the picture leaves is
  filled black, and whatever moves past the far edge is lost.
- **The catch:** the ring sits at radius 231 and the bands meet radius 232, on a visible circle
  of radius 233. A shift of 2 px or more clips them on one side. Choose between accepting that,
  bringing them inward on every screen, or shifting only what is inside them. The last one
  means redrawing in full on every shift, and every screen's layout gains an offset.
- **Also settle:** the step and range, the pattern, how often it moves, whether a move is
  animated or waits for a moment that hides it (a page change, the panel opening, the screen
  waking), and whether touch follows the picture. Touch following is cheap.

### 3. Screen timeout and dimming

Neither exists. The screen stays at its set brightness until power is removed.

- **What the hardware offers:**
  - Brightness is a command to the panel, from 0 to 255, and takes effect at once. The firmware
    can step it on every frame without drawing, so a dim or a fade costs nothing in draw time.
    The stored setting is 10–100 %.
  - The panel's driver can switch the display off and put it to sleep, and back. Nothing calls
    that yet.
  - Waking:
    - Touch and the cover report arrive on an interrupt pin. Whether the touch controller keeps
      reporting while the panel is off is not tested.
    - The IMU has wake-on-motion: any movement over a threshold raises an interrupt pin. It is
      not a raise gesture. Detecting a raise, or a turn of the wrist, would be firmware work on
      the accelerometer.
    - The two buttons are wired, but the firmware reads neither and their pins are unverified.
    - There is no ambient light sensor.
- **To settle:**
  - The timeout, and whether it differs by screen: the compass in use, the panel, an editor, the
    picker.
  - Whether it dims before switching off, and by how much.
  - What wakes the screen, and what the waking touch does. For example, a waking touch might do
    nothing else.
  - What a wake shows: where it lands, and whether the page's entry builds again.
  - How it meets cover. Cover means "go to the clock face" and is the only cover gesture.
  - Whether the timeout is a setting, and where it lives on the panel. A setting is a new cell
    or lives under an existing one, BRIGHTNESS being the obvious host.
  - An always-on face is not asked for. A timeout design that leaves room for one later is
    welcome.

Letting the processor sleep while the screen is off is firmware work the design need not plan
around.

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
| Before the pager | Start-up: the self-test, then the identity and the logo card, or the fault screen |

- **Start-up:** as section 2 of the round 3 spec describes. "Start-up as built" below has
  where the build interpreted it.
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
  and DEVICE open the device page, which ends with the attribution, `CLEAR SETTINGS` and
  `REPLAY START-UP`, which opens a chooser: the identity and logo card again, or a marked
  demonstration of one part failing (settings spec decisions 11 and 12).
- **Brightness:** any whole percentage from 10 to 100, set from the finger's x over the track:
  p = clamp(round(100 (x − 66) / 334), 10, 100), and the controller gets round(255 p / 100). Each
  of the ten track cells is a tenth of full. The cell the level falls in fills from its left as
  far as the level reaches.
- **Zone picker:** without a trusted date (the clock reads 1 Jan 2000 after a power loss until
  GNSS sets it), a zone is listed under every offset it keeps in a year. Rows then show `--:--`.

Captures drawn by the firmware's own code come from `crates/octowhere-ui/examples/render.rs`.
It draws the faces' stills and `panel-rest`, `panel-end`, `panel-scrolling`, `panel-pulling`,
`panel-device`, `panel-device-end`, `settings-brightness`, `settings-clear`, `picker-offset` and
`picker-zone`, and the start-up's `startup-selftest-*`, `startup-frame-*` and `startup-fault-*`;
`context/screen-captures/` keeps `startup-selftest`, `startup-selftest-failed`,
`startup-identity` (frame 40), `startup-card` and `startup-fault`. `tools/ui-sim` records scenes as GIF or MP4 at 20 ms per frame, with the finger
marked: `--record compass-states`, `--record startup`, `--record startup-failed`, `--record
settings` and `--record tour`; `--scenes` lists the
rest. The captures' fixture is 13:07:42 on Thu 24 Sep 2026 in Europe/Dublin, and heading 047°,
pitch +05, roll −12, calibration 54 %. None of it is a reading.

## The compass's states and changes, as built

The compass's original specification was removed once it was built, so this section is the
record of its states and of what happens between them. The layout is in the clock face spec's
"Compass changes"; the entry, swipe-out and heading-arrival motion is in the animation addendum.
The captures `compass-calibrating`, `compass-heading`, `compass-interference`,
`compass-top-edge-up` and `compass-no-data` show the five states. The `ui-sim` scene
`compass-states` plays a change of each kind below, and
`context/screen-captures/compass-states.gif` is its recording.

### States

Listed in precedence order: the first whose condition holds is shown.

| State | Condition | Ring | Dial | Icon | Caption | State line | Slab | Readout | Tilt line |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| NO DATA | the motion sensors have not yet given an orientation | `RED` | none | no-data glyph, `RED` | `COMPASS`, `GRAY` | none | `RED` | `NO DATA` | none |
| INTERFERENCE | a heading, and the field is disturbed | `GRAY` | turned to the heading | interference glyph, `ORANGE` | `MAGNETIC`, `GRAY` | `INTERFERENCE`, Mono Bold 24 px `ORANGE` | `ORANGE` | degrees | shown |
| HEADING | a heading | `GRAY` | turned to the heading | arrow, `BLUE` | `MAGNETIC`, `GRAY` | none | `WHITE` | degrees | shown |
| CALIBRATING | no heading, calibration under 100 % | `GRAY` | none | open loop, `ORANGE` | `CALIBRATION`, `ORANGE` | `TURN ALL WAYS`, Mono Regular 19 px `GRAY` | `ORANGE` | percent | shown |
| TOP EDGE UP | no heading, calibrated | `GRAY` | none | top bar, `WHITE` | `MAGNETIC`, `GRAY` | `TOP EDGE UP`, Mono Regular 20 px `GRAY` | `WHITE` | `---` | shown |

The slab, icon and caption never move or resize. The state line's space is kept when it is
empty.

### What drives the states

The screens get a reading every 20 ms while the compass shows, and every 250 ms otherwise. The
motion task holds the two conditions that would otherwise flicker, before the screens see them.

- **Interference:** shown once the field's strength has been more than 35 % above or below the
  strength the calibration measured for 200 ms without a break, and cleared once it has been
  within 30 % for 1 s without a break. The fusion still stops using a field on the first
  reading more than 35 % off, and the heading carries on from the gyroscope, without magnetic
  correction.
- **Top edge up:** the heading goes when the top edge points within about 11.5° of straight up
  or straight down, at once, and comes back only past 15°.
- **Calibration:** the percentage rises as the device turns through orientations and never
  falls. Only the panel's COMPASS cell restarts it, and that re-enters the compass page, so the
  page runs its entry rather than a change of state.
- **NO DATA:** shows only at start-up, until the first orientation. Nothing returns the compass
  to it afterwards today, but the screen handles the change as if something could.

### Changes on the settled page

The round 3 spec's section 1 (`octowhere-round3-display-motion-v5/DISPLAY-AND-MOTION-SPEC.md`)
specifies these, and they are built as it says. In short:

- **Slab wipe:** a new slab colour covers it from the top in four steps, 0, 20, 40 and 60 ms
  after the change, with the readout redrawn over both colours.
- **Icon rebuild:** the frame takes its new colour at once, and the new glyph builds a row
  every 30 ms.
- **State line:** a line that goes untypes right to left over 60 ms, and a new one types in over
  the next 120 ms, or over the first 120 ms if there was none. A changed caption retypes over
  120 ms.
- **Cuts:** a change into NO DATA is a single frame. On leaving NO DATA, the ring, slab, readout
  and tilt take their new state in the first frame. The dial and readout go in the first frame
  when the heading goes, so no stale bearing shows.
- The dial sweeps as before when a heading arrives. Back from TOP EDGE UP within 750 ms, the
  dial and icon show at once, and only the state line and slab change.
- A change that arrives while another runs starts from the state shown.

A digit change in the readout or the tilt line always happens in one frame, whatever the
state.

A change of state redraws only the parts that changed, a step at a time: the rows of the slab
the wipe covers, the icon's rows, and the state line's band. None of these frames has been
measured on the device.

## Start-up as built

Section 2 of `octowhere-round3-display-motion-v5/DISPLAY-AND-MOTION-SPEC.md` is built. The
captures `startup-*` and the recordings `startup.gif` and `startup-failed.gif`, also as `.mp4`,
show it. Where the
build interpreted the spec:

- **Boot order.** The firmware loads its settings, starts the panel, and draws the self-test
  while the parts come up behind it. The panel comes on dark and climbs to the stored level over
  the first 200 ms. The parts come up in cell order, and the clock, touch, motion and magnet
  checks run during the second the GNSS module needs to settle after its reset. On the device
  the last cell decides about 2 s after power-on.
- **What passes.** A cell passes when its part's driver brings it up: the power controller's
  chip ID, the clock's registers read (a clock that holds no valid time still passes, and the
  clock face shows it as stopped), the touch controller's start-up, the IMU's mode set, and the
  magnetometer's first compensated sample. GNSS passes if the receiver accepts any command or
  a read of its output succeeds. A part that fails is left out: the compass shows NO DATA
  without the IMU, the clock face shows NO DATA without the clock, and a failed touch
  controller leaves the device without touch.
- **Deadlines.** POWER 200 ms, CLOCK 200 ms, TOUCH 600 ms (its start-up waits 220 ms),
  MOTION 500 ms, MAGNET 500 ms, GNSS 1.5 s.
- **The reason line** reads `NO REPLY BY DEADLINE` for a part that ran out of time or did not
  answer on the bus. A part that answered wrongly, such as a wrong chip ID, reads
  `REPLY NOT AS EXPECTED`, since the spec's line would not be true of it. With several failures
  it is the first failure's.
- **The UTC digits** show dashes (`000 000 111 000 000`) when the clock has no time or its
  oscillator stopped, since a time it cannot vouch for is not data.
- **The scatter's generator** is a hash of each point's index, not Python's generator with seed
  4, so the pattern differs from the renders. Its density and the turn of its dense side match
  the renders to within a few per cent on every frame. The scatter is its own module
  (`ui::scatter`), with its circle, grid origin, the band it stops short of, its colour and its
  seed as parameters, so another screen can use it. The owner wants it on more pages later.
- **The mark** is the spec's (since v4): rectangles and a stripe test, drawn at any size from
  one description.
  In the microtext row it has no hatch, as the spec's text and its 15 × 15 rows say; the row
  preview in `marks-hatched.png` shows one. The replay chooser shows it as `GOOD`'s icon.
- **The stretched text** (the word at 1.8× and the running line at 1.3×) is drawn by the glyph
  renderer with a vertical scale, not stored as bitmaps.
- **The giant name** is rasterized at 100 px and drawn with each pixel as a 2 × 2 block. At
  200 px its largest glyph needs a 140 KB raster, and that allocation failed on the device.
  The edges show 2 px antialiasing steps.
- **Touch.** A touch during the identity or the card goes to the clock face with its entry
  finished. A touch during the fault screen goes to the clock face, which runs its entry. The
  finger that skipped is not a gesture: the faces ignore it until it lifts.
- **After the card** the clock runs its entry, and its time and date type in as the spec
  describes. After the fault screen it runs its entry with the time whole.
- **Timing.** The identity and the card hold 30 fps on the device. The fault screen draws a
  frame in about 49 ms, so it shows about 20 frames a second. Its frames are counted by the
  clock, so it still ends on time, with frames skipped. Making it faster is open work.
- **Replay.** The spec's Replay section matches what is built, except that the identity's
  line reads `SELF TEST 5/6 OK` after a failed boot, as the fault screen's does, where the spec
  has `SELF TEST 5/6`. Beyond the spec, `REPLAY START-UP` opens a chooser of a good start-up
  or a demonstration of one part failing (settings spec decision 12), which the design has not
  seen.

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
- **Stretched text:** upright text scaled taller than its font without widening, as the
  start-up's word and running line.
- **Text larger than about 100 px** needs a glyph raster of 4 bytes a pixel of its box on the
  240 KiB internal heap. Past that it is drawn at half size with each pixel doubled, as the
  fault screen's giant name.
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
  scratch per pixel of its bounding box, from the 240 KiB internal heap, so very large shapes
  do not fit.
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
| Start-up identity, a frame (scatter, microtext, word, hatch) | under 33 ms: it holds 30 fps |
| Start-up fault screen, a frame | about 49 ms |

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
- The flash image is 1,074,976 bytes, 6.86 % of the app partition. Flash is not a constraint.

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
