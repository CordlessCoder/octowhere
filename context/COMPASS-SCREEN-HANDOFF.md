# Compass screen handoff

For a design agent iterating on the compass screen's look. It says what the screen does, what it
draws today, the device it draws on, and what the renderer can and cannot do. Read it together
with [`marathon-ui-cross-project-handoff.md`](marathon-ui-cross-project-handoff.md), the
aesthetic doctrine, and the "Design language" section of [`AGENTS.md`](../AGENTS.md), whose rules
bind the code.

The owner reviews every change on the physical device. Two host tools render the same drawing
code first, pixel for pixel, at the panel's resolution:

- `cargo +stable run --manifest-path crates/octowhere-ui/Cargo.toml --target host-tuple --example
  render` writes every screen, and the compass in each state, to PNGs under
  `crates/octowhere-ui/target/renders/`. Add a state to `examples/render.rs` to see it.
- `cargo +stable run --release --manifest-path tools/ui-sim/Cargo.toml --target host-tuple` opens
  a window: the mouse is the touchscreen, and the keys set heading, tilt, calibration and
  disturbance. The source's header lists the keys, and P saves a PNG.

Neither says anything about draw time on the target, or about how colours look on the AMOLED.

## Device

| Property | Value |
| --- | --- |
| Board | Waveshare ESP32-S3-Touch-AMOLED-1.75 |
| Panel | Round AMOLED, CO5300 controller |
| Resolution | 466 × 466 px framebuffer; only the inscribed circle is visible |
| Active area | 43.76 mm diameter (see below) |
| Pixel pitch | about 0.094 mm, 10.65 px/mm, about 270 ppi |
| Colour | RGB565 framebuffer, 65,536 colours. Gradients band visibly |
| Black | An unlit AMOLED pixel is off, not dim. Pure black is the field |
| Touch | CST9217 capacitive, in the same 466 × 466 coordinates |
| Glass | 48.96 mm outer diameter |
| Board outline | 46.00 mm diameter PCB; 10.40 mm deep including the glass |

The manufacturer's dimension drawing (`~/git/octowhere-pcb/ESP32-S3-Touch-AMOLED-1.75-details-size.jpg`)
gives three front diameters without labels: 48.96, 44.16 and 43.76 mm. The 48.96 mm one is the
glass. Taking 43.76 mm as the active pixel area and 44.16 mm as the window opening is a reading of
the drawing, not a stated fact; the pixel pitch above follows from it.

Geometry the code relies on:

- The panel's centre is the pixel corner at (233, 233). Pixel (x, y) covers x..x+1, y..y+1, so
  pixels 232 and 233 straddle the centre on each axis.
- The corners of the 466 × 466 square are outside the visible circle. Anything drawn there is
  lost.
- The outermost visible pixels were checked by eye: a ring inset by 2 px looked too far in, and
  1 px was judged right. The edge of the glass shades the last pixel or so.

## What the screen does

The compass shows the board's magnetic heading and its tilt, from the magnetometer, accelerometer
and gyro fused in `motion_task` (see `context/GRAPHICS-PROTOTYPES.md`, "compass"). It is one of
eight screens:

`Map, Motion, Clock, Touch, Power, Navigation, Compass, AxisCheck`

A horizontal drag moves between neighbours, and the ring wraps. The compass sits between
Navigation and the axis check screen. The compass and the axis check screen have no header or
footer bar; every other screen does.

During a drag, the page follows the finger and settles to the next screen or back. Both pages are
drawn in that time, each shifted horizontally and clipped to its visible part. A layout must still
read correctly when cut off by a vertical edge anywhere across it.

### Data

`CompassView` in `crates/octowhere-ui/src/ui/compass.rs` is everything the screen receives:

| Field | Meaning |
| --- | --- |
| `live` | Both sensors produced a sample and the fusion has an orientation |
| `calibration_percent` | 0 to 100. Calibration is turning the board through every orientation |
| `heading_decidegrees` | Tenths of a degree clockwise from magnetic north to the top edge. `None` below 100% calibration, or when the top edge points within about 12° of straight up or down |
| `pitch_deg` | Whole degrees the top edge is raised above level |
| `roll_deg` | Whole degrees the right edge is raised above level |
| `disturbed` | The field's strength is more than 35% off the calibrated strength, so the heading is unreliable |

The heading is magnetic, not true north. It refreshes every 20 ms while the screen shows.

Calibration keeps adapting after it completes: it follows slow drift and replaces itself when the
board's own magnetic offset changes. None of that is on screen yet. The motion task also knows the
candidate fit's progress, the field strength and the learned gyro offset, and could pass them to
the screen if a design needs them. That is a code change, not only a visual one. Per the doctrine,
a screen must not show a state the system does not have.

### States

The screen is in exactly one of these, checked in this order:

| State | Condition | Centre, top line | Centre, second line | Dial |
| --- | --- | --- | --- | --- |
| No data | `!live` | `NO DATA`, `RED`, 32 px, at the centre; nothing else in the centre | none | Ticks only, not turning, no labels |
| Heading, disturbed | heading and `disturbed` | Heading `000`–`359` in a box | `MAG INTERFERENCE`, `ORANGE` | Turns, with labels |
| Heading | heading | Heading `000`–`359` in a box | 16-point cardinal (`N`, `NNE`, … `NNW`), `LIME` | Turns, with labels |
| Calibrating | `calibration_percent < 100` | `CAL nn%`, `ORANGE` | `TURN ALL WAYS`, `GRAY` | Ticks only, not turning, no labels |
| No heading | otherwise (top edge near vertical) | `---`, `WHITE` | `TOP EDGE UP`, `GRAY` | Ticks only, not turning, no labels |

Every live state also draws the tilt line and the recalibration hint.

The dial carries bearing labels only while there is a heading. A dial that held still with
bearings on it would read as pointing somewhere. Keep that rule.

`RED` is spent only on the no-data fault. Interference is `ORANGE`: a condition worth attention,
not a fault.

### Interaction

- **Tap within 100 px of the centre:** restarts calibration. The screen goes to `CAL 00%` and the
  dial stops turning until the board has been turned through every orientation again.
- **Tap elsewhere:** nothing.
- **Horizontal drag:** changes screen.
- No vertical gesture, long press or button acts on this screen.

Tap and drag are told apart by movement: more than 16 px is a drag.

## Current layout

All coordinates are in framebuffer pixels. The centre is C = (233, 233), and "r" means distance
from C. `R` is the dial radius, 232.

### Dial

Drawn in this order; later items paint over earlier ones.

| Element | Geometry | Colour | Notes |
| --- | --- | --- | --- |
| Outer ring | r 230 to 232, 2 px | `GRAY` | Antialiased |
| Minor ticks | Every 10° not on a multiple of 30. r 216 to 226, 2 px wide, flat ends | `GRAY` | Antialiased; turn with the heading |
| Major ticks | Every 30°. r 202 to 226, 4 px wide, flat ends | `WHITE` | Antialiased; turn with the heading |
| Cardinal labels | `N E S W`, centred at r 174 | `N` `ORANGE`, others `WHITE` | Marathon Shapiro, 40 px. Only with a heading |
| Bearing numbers | `30 60 120 150 210 240 300 330`, centred at r 182 | `GRAY` | PP Fraktion Mono, 22 px. Only with a heading |
| Lubber mark | x 231 to 234, y 1 to 20 (4 × 20 px), at the top | `BLUE` | Fixed; marks the top edge's direction. Drawn over the ticks and labels |

Labels are turned to follow the dial: each one's baseline is tangent to the circle, and its top
faces outward. So `S` and the numbers near the bottom of the dial read upside down when the heading
puts them there.

The dial turns so that `N` points at magnetic north. The fixed lubber mark shows which bearing the
top edge faces.

### Centre

Text is centred horizontally in a region box centred on the given point. "Box" gives the region's
size, which bounds where the text may land. It is not drawn.

| Line | Region centre | Box | Font, size | Colour |
| --- | --- | --- | --- | --- |
| Heading | (233, 173) | 260 × 80 | PP Fraktion Mono, 72 px | `BLACK` knocked out of a slab. The slab is `WHITE`, or `ORANGE` while disturbed |
| `CAL nn%` / `---` | (233, 173) | 260 × 80 | Marathon Shapiro, 32 px | As in the states table |
| Second line | (233, 239) | 300 × 40 | PP Fraktion Mono, 32 px | As in the states table |
| Tilt | (233, 283) | 260 × 34 | PP Fraktion Mono, 26 px | `GRAY`. Text is `P +05  R -12`: sign always shown, two spaces between |
| Hint | (233, 323) | 260 × 26 | PP Fraktion Mono, 18 px | `GRAY`. Text is `TAP CENTRE TO RECAL` |
| No data | (233, 233) | 260 × 40 | Marathon Shapiro, 32 px | `RED` |

The heading slab is the number's ink bounds grown by 14 px left and right and 10 px top and bottom,
so its width changes slightly with the digits.

Open questions about the current layout:

- `MAG INTERFERENCE` at 32 px was sized from estimated glyph widths and has not been seen on the
  panel yet. It may clip inside its 300 px box.
- The owner has iterated on this layout over a few rounds: the ring was moved out to the panel
  edge, the lubber mark shrunk and turned blue, the centre block raised and its lower lines
  enlarged, and the heading given a knockout slab. Treat the current state as a working surface,
  not a settled design.

## Colour tokens

Colours come only from these tokens in `crates/octowhere-ui/src/chrome.rs`. A new colour is added
there, with a value from [`palette-reference.md`](palette-reference.md), which records the reference
board's swatches.

| Token | Hex | Role today |
| --- | --- | --- |
| `LIME` | `#C0FE04` | Accent, ok and live states; the cardinal line |
| `RED` | `#F24723` | Faults only |
| `ORANGE` | `#F1710D` | Attention: calibrating, interference, the `N` label |
| `PURPLE` | `#5500E4` | Header slab on other screens |
| `BLUE` | `#409DE4` | The lubber mark |
| `GRAY` | `#888E98` | Frames, minor ticks, secondary text |
| `WHITE` | `#D2D3D6` | Primary text, major ticks, the heading slab |
| `BLACK` | `#000000` | The field; knockout text |

The reference board has unused swatches too: pale yellow, orange-red, deep pink, violet, yellow,
green, pale green, and a neutral ramp.

## What the renderer can draw

The firmware is `no_std` Rust. The screen is one function, `draw_compass` in
`crates/octowhere-ui/src/ui/prototypes.rs`. It draws into a `chrome::CoverageTarget`: an
embedded-graphics `DrawTarget` over an RGB565 framebuffer that also takes antialiased coverage a
row at a time.

- **Fonts:** two, both limited to printable ASCII (space to `~`). There is no `°` or other symbol
  glyph. A degree mark would have to be drawn as a shape. The full font files do contain `°`, and
  fontdue's `chars:` option can subset them, so adding glyphs is a small change if a design needs
  them.
  - Font 0, Marathon Shapiro Wide 65, the display face.
  - Font 1, PP Fraktion Mono, a monospace face.
- **Upright text:** any size, aligned in a box with `aligned_text`. Each glyph's coverage is
  cached by font, glyph and size, so repeated text costs little.
- **Rotated text:** `draw_rotated` centres a string on a point, turned by any angle. The bearing
  labels use it. It rasterizes every glyph every frame, so it is the costliest text.
- **Text bounds:** `FontdueRenderer::aligned_bounds` gives a string's ink bounds before drawing.
  The heading slab is sized this way.
- **Filled rectangles:** axis-aligned, solid, no antialiasing needed.
- **embedded-graphics primitives:** circles, arcs, lines, triangles, polygons and rounded
  rectangles. These are not antialiased and look stepped at this density.
- **`smooth::Ring`** (`crates/octowhere-ui/src/ui/smooth.rs`): an antialiased ring between two radii
  about a pixel corner. Its coverage is computed once and redrawn each frame.
- **`smooth::polygon_quarters`:** fills any polygon with fontdue's antialiased path fill, then
  draws it at all four quarter turns about a pixel corner. Built for the ticks, where each fill
  serves four ticks.
- **fontdue's path fill** (`fontdue::rasterize_path_clipped`) takes any polygon, and curves once
  flattened with `fontdue::flatten`. It can back any other antialiased shape.

Constraints that shape what is practical:

- **Antialiased edges blend with what is underneath.** The framebuffer reads a partly covered
  pixel back and mixes with it. Where the background is known, `chrome::OnBackground` names it
  and the read is skipped, which is faster. The compass draws everything that way: the dial and
  the status lines on `BLACK`, the heading number on its slab colour. Drawing through
  `OnBackground` over any other colour gives edges mixed with the wrong one.
- **Later draws overwrite earlier ones.** There is no alpha compositing beyond edge coverage. Draw
  order is the layering.
- **Symbols are built from primitives, not bitmaps.** This is doctrine (`AGENTS.md`).
- **The whole screen redraws when anything on it changes.** The heading changes almost every
  frame, so the compass redraws in full. A frame showing a heading measured about 18 ms, against a
  20 ms sensor period:
  - clearing the visible circle, about 8.3 ms;
  - the bearing labels, about 4.2 ms;
  - the centre text, about 2.2 ms;
  - the ring, about 1.6 ms;
  - the ticks, about 1.4 ms.

  So there is little headroom. Rotated text is the costly element. A design that adds rotated
  labels, or large antialiased areas, will lower the frame rate. Ask for a measurement before
  committing to one. The bench is `bench/compass-draw-rows`.
- **Memory:** a 260 KiB internal heap. A path-fill raster costs 4 bytes per pixel of its bounding
  box, so a raster the size of the dial is out of reach. The ring is analytic for that reason. The
  glyph cache holds at most 32 KiB.

## Rules to keep

From the doctrine and `AGENTS.md`:

- No invented data. Everything on the screen comes from `CompassView` or is a fixed label. A new
  readout needs its data plumbed first.
- `RED` means a fault. Nothing else uses it.
- Saturated fills carry black knockout text and symbols.
- Colours come from the tokens above.
- Symbols are built in code from primitives on integer geometry.
- The heading is withheld until calibration completes, and the dial carries no bearings while it
  is withheld.

## Working on it

Build and check:

```text
cargo build --release --offline
cargo clippy --release --offline -- -D warnings
```

The owner flashes the board and judges the result on the panel. Keep changes inside
`draw_compass` and its constants unless a design needs new data or a new primitive. Say so when it
does, since both are more than a visual change. Commit messages follow "Writing conventions" in
`AGENTS.md`. Leave committing to the owner unless asked.
