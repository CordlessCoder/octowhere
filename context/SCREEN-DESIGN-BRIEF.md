# Screen design brief

This brief is for a design agent working on this device's screens. "This round" says where
the rounds stand. The rest covers the hardware, the screens as built, the gestures, what the renderer draws and what that costs, and
what data and settings exist. Read it with:

- [`design/handoffs/IMPLEMENTATION-HANDOFF-CURRENT.md`](design/handoffs/IMPLEMENTATION-HANDOFF-CURRENT.md):
  the design agent's hand-off of 2026-09-26, which redesigns every screen. The owner approved
  all of it ([`design/DECISIONS.md`](design/DECISIONS.md)), and it is being built; until it is,
  the rest of this brief describes the screens as they were before it.
- [`design/docs/marathon-ui-design-language.md`](design/docs/marathon-ui-design-language.md), the
  design doctrine, and [`design/docs/OCTOWHERE-COLOR-ROLES.md`](design/docs/OCTOWHERE-COLOR-ROLES.md).
  The doctrine was written for an earlier product. Where it disagrees with this brief about this
  device, this brief wins.
- [`design/specs/CLOCK-FACE-SPEC.md`](design/specs/CLOCK-FACE-SPEC.md): the clock
  face, the compass's layout, the zone picker, the icon system and the motion primitives.
- [`design/specs/COMPASS-ANIMATION-ADDENDUM.md`](design/specs/COMPASS-ANIMATION-ADDENDUM.md):
  the compass's motion.
- [`design/specs/CLOCK-WORDMARK-ADDENDUM.md`](design/specs/CLOCK-WORDMARK-ADDENDUM.md): the
  OCTOWHERE wordmark on the clock face.
- [`design/specs/SETTINGS-PANEL-SPEC.md`](design/specs/SETTINGS-PANEL-SPEC.md): the settings
  panel and its second-level screens.
- [`design/specs/IMPLEMENTATION-RESPONSE.md`](design/specs/IMPLEMENTATION-RESPONSE.md) and
  [`design/specs/SETTINGS-DESIGN-RESPONSE.md`](design/specs/SETTINGS-DESIGN-RESPONSE.md):
  what the build interpreted in the settings round, and the design's reply. The reply's two
  changes are built.
- [`design/specs/DISPLAY-AND-MOTION-SPEC.md`](design/specs/DISPLAY-AND-MOTION-SPEC.md):
  round 3, the compass's changes of state, the start-up, screen timeout with the always-on
  face, pixel shift and two panel cells.

Everything those specs describe is implemented and has been approved on the panel, except
round 3's pixel shift, which the owner deferred (see "Owner decisions that bind later
screens"). The 2026-09-26 hand-off changes how they look; their behaviour stands where the
hand-off does not change it.
Where a spec and this brief disagree about what is built, the spec's own Decisions section and
the responses are newer, except for round 3: its spec has no decisions section, and the owner's
later decisions are in this brief's "as built" sections.

## This round

Round 3 answered the owner's last three questions: smoother changes between the compass's
states, pixel shift against burn-in, and a screen timeout with dimming. Its spec is built
except pixel shift, and the sections below marked "as built" record where the build
interpreted it.

The owner sets the next round's questions. One topic they have named is where outlined text
belongs in the screens and animations. The primitive exists and is costed under "What the
renderer draws", and `screen-captures/outline-*.png` show it at the sizes in use. No screen
uses it yet.

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
| Brightness | Set from the stored setting at boot (10–100 %, default 120 of 255). Changes apply at once, and the firmware can step it every frame at no draw cost. No ambient light sensor |
| Sleep | The panel's display-off and sleep, used by the screen timeout. Touch still reports while it sleeps |
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

- **Start-up:** the 2026-09-26 hand-off's S1 self-test, G19 identity with G17's opening and
  marks, and G17's card, with round 3's section 2 for the behaviour they keep. "Start-up as
  built" below has where the build interpreted them.
- **Timeout, dimming and the always-on face:** as section 3 of the round 3 spec describes,
  except that the level fades rather than steps (owner). "Timeout and rest as built" below
  has the details.
- **Clock face:** the hand-off's K1, on the round 4 spec: the band `LIME` while the time is
  local, `ORANGE` stopped, `WHITE` without a zone and `RED` without data, with UTC and the
  battery in it beside a hatched battery column; the date and zone as token lines; a 24-hour
  rail with a lime marker on a known local hour; and two dim `PURPLE` scatter fields. "Clock
  face as built" below has where the build interpreted it. A time that a fix or zone change
  replaces types in again by cell reveal: the hours, minutes and seconds over 180 ms, and the
  first token line over 160 ms from 120 ms. A tick never animates the digits.
- **Compass:** as the clock face spec's compass section and the animation addendum describe, with
  the 66 px outlined icon. It no longer has cover-to-recalibrate, its `COVER SCREEN TO RECAL`
  hint, or the divider above it. The tilt line ends the stack. Calibration restarts from the
  panel's COMPASS cell.
- **Settings panel:** a registration grid of eight cells in four columns, two in view,
  scrolling sideways: ZONE, BRIGHTNESS, TIMEOUT, ALWAYS ON, COMPASS, GNSS, BATTERY, DEVICE
  (round 3 spec section 5). ZONE opens the picker, BRIGHTNESS the editor and TIMEOUT the
  timeout screen. A tap on ALWAYS ON toggles and stores it. COMPASS restarts calibration and closes to the compass. GNSS, BATTERY
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
It draws the faces' stills and `panel-rest`, `panel-middle-always-on`, `panel-end`,
`panel-scrolling`, `panel-pulling`, `panel-device`, `panel-device-end`, `settings-brightness`,
`settings-timeout`, `settings-clear`, `picker-offset` and
`picker-zone`, and the start-up's `startup-selftest-*`, `startup-frame-*` and `startup-fault-*`;
`examples/outline.rs` draws the outline samples, which `context/screen-captures/` keeps as
`outline-shapiro-40`, `outline-fraktion-bold-136` and `outline-fraktion-16`;
`context/screen-captures/` keeps `startup-selftest`, `startup-selftest-failed`,
`startup-identity` (frame 77), `startup-card` (its frame 3) and `startup-fault`. It also draws the
always-on face's `always-on-local`, `always-on-stopped`, `always-on-no-zone` and
`always-on-no-data`, which `context/screen-captures/` keeps. `tools/ui-sim` records scenes as
GIF or MP4 at 20 ms per frame, with the finger marked: `--record compass-states`, `--record
startup`, `--record startup-failed`, `--record settings`, `--record rest-always-on`, `--record
rest-off` and `--record tour`; `--scenes` lists the rest. The simulator shows the display's
level by scaling colours against the stored level. The captures' fixture is 13:07:42 on Thu 24 Sep 2026 in Europe/Dublin, and heading 047°,
pitch +05, roll −12, calibration 54 %. None of it is a reading.

## Clock face as built

K1 (`design/renderer/concept/family-pass-v1-out/clock-K1-*`) on the round 4 spec
(`design/specs/CLOCK-FACE-ROUND4-SPEC.md`). The captures `clock*`, `clock-charging`,
`clock-battery-low` and `clock-battery-unknown` show it. Where the build interpreted them:

- **The halo.** The scatter leaves out every mark within 2 px of an element's box (the hours,
  the icon, the wordmark's letters off the band, each token line and the rail), where the
  design grows each element's ink by 2 px in black. Marks the design lets sit between letters
  do not show. The elements then draw on known black, and a changed hour moves no mark.
- **The scatter** is the firmware's generator with K1's two fields, facings and densities, in
  `PURPLE` dimmed to 49 %, clear of the band's rows by about 4 px. NO DATA has none.
- **The seconds** show `--` while the time is withheld, as K1 has them.
- **The battery line** reads `BAT 87%`, `BAT 87% CHG` while charging, `USB` with USB and no
  battery, and `BAT --` while unknown. With no battery the hatch shows as unknown, `GRAY` at
  full height.
- **The charging crawl** moves the hatch up a pixel every 33 ms, the start-up's frame, while
  the face shows, dimmed or not (owner). It redraws the hatch alone.
- **The rail** appears with the zone's line in the entry. Its cells are `GRAY` dimmed to 29 %.
- **The entry** follows the round 4 spec's windows and curves: the icon's rows over 150 ms by
  out-cubic, where the old face landed a row every 30 ms, so it shows no row on its first frame.
- **Redraws.** Every part redraws in its own region, as before. A token line that changes also
  redraws the scatter marks its box clears or frees.

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

The round 3 spec's section 1 (`design/specs/DISPLAY-AND-MOTION-SPEC.md`)
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

The self-test, identity and card follow the 2026-09-26 hand-off
(`design/handoffs/IMPLEMENTATION-HANDOFF-CURRENT.md` §3.1–3.2), whose frame timings come from
its `renderer/concept/startup_s1_g17.py`. The fault screen and the behaviour around all of it
are section 2 of `design/specs/DISPLAY-AND-MOTION-SPEC.md`. The captures `startup-*` and the
recordings `startup.gif` and `startup-failed.gif`, also as `.mp4`, show it. Where the build
interpreted them:

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
- **A demonstration's self-test** reads `DEMO, NOT A HARDWARE TEST` where a boot's reads its
  version.
- **Colours.** The design's dim marks are tokens dimmed toward black (`chrome::shade`): the
  outlined title and unlit ticks are `LIME` at 34 %, the partly lit ticks 58 %, the opening's
  block rows 96, 82, 91 and 100 %, the registration marks `GRAY` at 32 % and their hairlines
  25 %, and the scatter `PURPLE` at 27 %. BOOT and the dim blocks behind the opening are
  `DEEP_BLUE`, `#000DF6`, the intro cinematic's blue, at full, 55 % and 10–18 % (owner).
- **BOOT** is set in KH Interference Bold at 38 px, turned a quarter clockwise, as the design
  has it (owner). Only its three letters are embedded.
- **The opening's backdrop** of 185 dim blocks comes from a fixed sequence of the firmware's
  own, not Python's generator, so the blocks sit elsewhere at the same density.
- **The title's outline** is 2 px inside each glyph's edge, where the design's is 1.55 px. A
  glyph types in by its outline's shade rising over 55 ms, which at 30 fps is one frame at part
  shade. Its partly lit frames show 2 px slices of the filled word.
- **The registration marks' centres** are 2 × 2 squares, where the design has a 4 px diamond.
- **The small logo** draws the mark's 15 × 15 rows at 1.3 px modules rounded to whole pixels,
  so its strokes are one or two pixels wide.
- **The scatter** uses the firmware's generator, not the design's Python surrogate, with the
  design's two fields, facings, densities and turns. The scatter is its own module
  (`ui::scatter`), with its grid origin, the band it stops short of and its colour as
  parameters, and one or more fields on that grid, each a circle and a seed with its own
  facing and density. Where fields overlap, the first to show a point gives its mark. A mark
  shows only if it lies wholly inside the glass. The owner wants it on more pages later.
- **Redraws.** Every identity frame before frame 67, when the small logo settles, redraws in
  full. From then on a frame redraws only the scatter marks its turns move and the UTC digits
  when the minute changes.
- **The card's lime page** runs to the glass's edge, where the design's stops at radius 232,
  since the frame clears straight to lime rather than painting a disc over black.
- **The UTC digits** show dashes (`000 000 111 000 000`) when the clock has no time or its
  oscillator stopped, since a time it cannot vouch for is not data.
- **The mark** is rectangles and a stripe test, drawn at any size from one description. In the
  microtext row it has no hatch. The replay chooser shows it as `GOOD`'s icon.
- **The running line** on the fault screen is stretched to 1.3× by the glyph renderer, not
  stored as a bitmap.
- **The giant name** is rasterized at 100 px and drawn with each pixel as a 2 × 2 block. At
  200 px its largest glyph needs a 140 KB raster, and that allocation failed on the device.
  The edges show 2 px antialiasing steps.
- **The fault screen's field** runs red to the glass's edge, where the spec stops it at
  radius 229 (owner, 2026-09-25). The band and the strip run to the edge with it, and the text
  in them is no longer clipped to 229. The black rim cost a clear of the whole panel under
  the red.
- **Touch.** A touch during the identity or the card goes to the clock face with its entry
  finished. A touch during the fault screen goes to the clock face, which runs its entry. The
  finger that skipped is not a gesture: the faces ignore it until it lifts.
- **After the card** the clock runs its entry, and its time and date type in as the spec
  describes. After the fault screen it runs its entry with the time whole.
- **Timing.** The identity, the card and the fault screen hold 30 fps on the device. Their
  frames are counted by the clock, so a slow frame is skipped rather than stretching the
  sequence. The identity runs 120 frames and the card 19, 4.6 s together.
- **Replay.** The spec's Replay section matches what is built, except that the identity's
  line reads `SELF TEST 5/6 OK` after a failed boot, as the fault screen's does, where the spec
  has `SELF TEST 5/6`. Beyond the spec, `REPLAY START-UP` opens a chooser of a good start-up
  or a demonstration of one part failing (settings spec decision 12). The hand-off's D3 look
  for it is not built yet.

## Timeout and rest as built

Section 3 of the round 3 spec is built. The recordings `rest-always-on.mp4` and `rest-off.mp4`
show it with a 15 s timeout. Where the build differs from the spec or interprets it:

- **Fades (owner, 25 Sep 2026).** The level fades instead of stepping. The dim fades down over
  500 ms. On the way to off, the level fades from the dim to dark over 300 ms, and then the
  panel gets its display-off and sleep commands. A wake fades up over 250 ms, from the dim, the
  always-on face or dark. The cut to the always-on face is still one frame, as the spec has it.
- **The dim** lasts 5 s from its start, fade included, then the always-on face or the fade to
  off.
- **Off** is the panel's display-off and sleep. The touch controller still reports with the
  panel asleep, so the spec's fallback (a black frame at level 0) is not needed.
- **A wake from off** runs the page's entry and the fade up after 140 ms, once the panel is
  out of sleep, so neither plays on a dark panel.
- **The timer restarts** on any contact, a cover, the pager, the panel or the grid moving, a
  second-level screen opening or closing, and on the compass a heading more than 10° from the
  heading at the last restart. It does not run during the start-up or a replay, and restarts
  when the clock face takes over.
- **The levels.** The dim is 30 % of the level that shows, at least 8 (3 % of 255) and never
  above the level itself. The always-on face is at 26 (10 %), or the set level if lower. With
  the brightness editor open, the dim is taken from the level being previewed. A wake from the
  panel discards the preview and fades up to the stored level.
- **The always-on face** is the hand-off's H2b: regular-weight digits, dim blue blocks where
  the rules were, a 24-hour rail and the battery in every state. With local time the blocks are
  H2's, which step sideways with the minute; NO ZONE and STOPPED take the quieter bars their
  renders have. NO ZONE shows UTC with a subdued blue marker on its hour, STOPPED
  dashes and no marker, and NO DATA red dashes, the word and `CLOCK`. It redraws in full when
  its minute, date or state changes, and at no other time. The battery it shows is taken again
  only at those redraws, and once a minute by the stage's own clock while the time stands still
  (STOPPED or NO DATA), so a reading never wakes the panel on its own (owner). Its dim colours
  are `BLUE` and `LIME` dimmed toward black.
- **The cells** are section 5's. The timeout screen is the replay chooser's stepper with the
  spec's text. The grid rests at a whole column, so it has three resting places, and a tap on
  a cropped column scrolls one column, whichever side it is on. The clear warning, which the
  spec leaves open, reads `ERASES ZONE, LAST FIX ZONE,` `BRIGHTNESS, TIMEOUT` `AND ALWAYS ON`
  on three lines.
- **Pixel shift** is not built yet, so nothing moves at a wake or on the always-on face's
  minute.

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
| Dimmed | A contact | Fades back to the level and does nothing else. The finger is ignored until it lifts |
| Always-on face, off | A contact | Wakes the screen (see "Timeout and rest as built"), and does nothing else |
| Dimmed, always-on face, off | Cover | Nothing |
| Any screen | Cover | Goes to the clock face, discarding any edit in progress. From the panel it closes with the released-drag motion, and the face under it becomes the clock. It is the only cover gesture |

Physics:

- **Page and sheet:** follow 1:1. A release commits past a quarter of the width or height, or
  faster than 600 px/s. The remainder, or the way back, eases out cubically over 160 ms
  whatever the distance, and the page's entry starts on the frame it lands.
- **Grid snap:** a cubic ease-out over 160 ms, to the nearest whole column, or one column on in
  the flick's direction past 600 px/s.
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
3. **Does not exist:** raise to wake, or any wake but touch (the IMU's wake-on-motion and the
   buttons are unused); a 12-hour clock (ruled out by the clock spec); units, languages, sounds or vibration; pairing, the location mesh,
   Wi-Fi or Bluetooth; alarms, timers, step counting and notifications.

## Settings

| Setting | Stored | On the panel |
| --- | --- | --- |
| Time zone mode, automatic or manual | yes | ZONE, then the picker |
| Manual zone | yes | the picker |
| Last zone GNSS found (automatic mode's memory) | yes | shown through ZONE |
| Brightness | yes | BRIGHTNESS |
| Compass calibration | no. It is learned at run time and restarted from the COMPASS cell | COMPASS |
| Screen timeout | yes | TIMEOUT, then the timeout screen |
| Always-on face | yes | ALWAYS ON |

- Settings live in an ekv database in flash. Clearing erases all six stored keys. The device
  returns to its defaults: automatic zone, brightness 120, a 1 min timeout, and ALWAYS ON off.
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
- **Outlined text:** upright text drawn as a ring just outside each glyph's edge, 1 or 2 px wide
  (any whole number works; wider ones cost more). Drawn alone it gives hollow letters; with the
  text drawn over it in another colour it gives a halo. The ring keeps the glyphs'
  antialiasing. Counters and gaps narrower than twice the width fill in: at 16 px, Fraktion's
  small counters close even at 1 px. Each glyph's ring is its own, so letters set closer than
  the width get rings that cross their neighbours. No screen uses it yet. The captures
  `outline-*.png` show it at the sizes in use. Measured on the device, against the same text drawn plainly:

  | Text | Plain | 1 px ring | 2 px ring |
  | --- | ---: | ---: | ---: |
  | Shapiro 40 px, `OCTOWHERE` | 2.1 ms | 4.4 ms | 6.4 ms |
  | Fraktion Bold 136 px, `48` | 2.0 ms | 4.6 ms | 6.8 ms |
  | Fraktion 16 px, `WED 25 SEP` | 0.6 ms | 1.0 ms | 1.4 ms |

  A halo costs the ring plus the plain text. Stretched, doubled and rotated text have no outline
  yet.
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
| Clock, a second ticks, or a step of the charging crawl | about 3.6 ms |
| Clock, a token line typing in | 6.5–8.8 ms |
| Compass, tilt changes by a degree | 1.7 ms |
| Compass, a step of the dial sweep | 7–12 ms |
| Compass, heading changes by a degree | about 13 ms |
| Clock drawn in full | about 28 ms, at most 36 ms |
| Compass drawn in full | 13–23 ms |
| Start-up identity, a frame of its opening grid | about 15 ms, at most 19 ms |
| Start-up identity, a frame while its title and marks build | about 23 ms, at most 31 ms |
| Start-up identity, once settled | nothing but the scatter's turns and the digits |
| Start-up card, a frame | about 12 ms, at most 22 ms |
| Start-up fault screen, a frame | about 25 ms, at most 28 ms |

Of a full clock draw, the scatter is about 5.7 ms and the wordmark 0.2 ms. The new clock face's minute change and its entry's other steps were not measured on their own. The screens of the settings round are not
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
- The flash image is 1,135,552 bytes, 7.25 % of the app partition. Flash is not a constraint.

## Owner decisions that bind later screens

- Colour tokens only. `RED` is only for a fault.
- `BLUE` marks a live, valid reading on a status icon. `GRAY` as an icon colour marks a value that
  is valid but unconfirmed.
- Each colour's meaning is in [`design/docs/OCTOWHERE-COLOR-ROLES.md`](design/docs/OCTOWHERE-COLOR-ROLES.md).
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
- Pixel shift is deferred, since the timeout makes burn-in unlikely. When it is built, the
  whole picture moves, the perimeter ring and bands included, so layouts will keep a margin
  between the ring and the glass's edge. The margin is not settled yet.

## Colour

Colours come only from these tokens. Each is a swatch from the reference board except `BLACK`.

| Token | Hex | Role |
| --- | --- | --- |
| `LIME` | `#C0FE04` | The identity and its card |
| `RED` | `#F24723` | Faults only |
| `ORANGE` | `#F1710D` | Attention: calibrating, interference, a stopped clock, the clear confirm; the compass's `N` |
| `PURPLE` | `#5500E4` | The identity's scatter |
| `BLUE` | `#409DE4` | A live, valid reading's status icon |
| `VIOLET` | `#B32BE5` | Not yet used; the choice being edited in settings, once the 2026-09-26 design is built |
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
