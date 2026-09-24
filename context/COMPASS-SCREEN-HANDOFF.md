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
  a window: the mouse is the touchscreen, holding H is a hand covering it, and the keys set
  heading, tilt, calibration and disturbance. The source's header lists the keys, and P saves a
  PNG.

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

### States, interaction and layout

The screen implements [`compass-implementation-handoff.md`](compass-implementation-handoff.md),
the design agent's specification, and matches the approved concept in
[`compass-concept-states.png`](compass-concept-states.png). Read those for the design. The code is
`crates/octowhere-ui/src/ui/compass_screen.rs`: `Mode::of` holds the state precedence, and the
geometry, icon patterns and colour mapping are constants at the top of the file. The stage in
`stage.rs` owns the cover gesture and the fades.

Where the implementation differs from the handoff, or fills a gap in it:

- The icon sits at y 87 and the caption's baseline at 138, not 83 and 132. The owner asked for
  even gaps between icon, caption and slab; the inks now stand 7 px apart.
- The direction abbreviation is 36 px, not 32, with its baseline unchanged. The owner asked for
  it to sit evenly between the slab and the tilt line; it now has 16 px of space above and below.
  `MAG INTERFERENCE` keeps 24 px on the same baseline.
- Unstated sizes were chosen to match the concept: `NO DATA` in Shapiro 28 px, the cardinal
  letters in Shapiro 40 px, `---` in the readout's own style, captions `GRAY` except
  `CALIBRATION`. `TURN ALL WAYS`, `TOP EDGE UP`, the tilt and the hint are PP Fraktion Mono
  Regular; the readout, captions and the cardinal and interference lines are Bold.
- A cover is taken only on the compass page at rest with `live` set, once per hand: another
  cover needs a touch report without one in between. The firmware reads the controller on its
  interrupt, so if the controller does not interrupt when the hand lifts, the next cover waits
  for any touch. An ordinary tap on this page does nothing.
- On an accepted cover the stage clears the heading and shows `000%` at once, before the motion
  task confirms the reset on its next sample.
- Open questions for the designer are in
  [`compass-design-questions.md`](compass-design-questions.md).

## Colour tokens

Colours come only from these tokens in `crates/octowhere-ui/src/chrome.rs`. A new colour is added
there, with a value from [`palette-reference.md`](palette-reference.md), which records the reference
board's swatches.

| Token | Hex | Role today |
| --- | --- | --- |
| `LIME` | `#C0FE04` | Accent, ok and live states; the heading icon and cardinal line |
| `RED` | `#F24723` | Faults only: the NO DATA ring, slab and icon |
| `ORANGE` | `#F1710D` | Attention: calibrating, interference, the `N` letter |
| `PURPLE` | `#5500E4` | Header slab on other screens |
| `BLUE` | `#409DE4` | Unused since the lubber mark went |
| `GRAY` | `#888E98` | The ring, minor ticks, captions and secondary text |
| `WHITE` | `#D2D3D6` | Major ticks, most cardinal letters, the heading and top-edge slabs |
| `BLACK` | `#000000` | The field; knockout text |

The reference board has unused swatches too: pale yellow, orange-red, deep pink, violet, yellow,
green, pale green, and a neutral ramp.

## What the renderer can draw

The firmware is `no_std` Rust. The screen is `compass_screen::draw`, called from `draw_compass`
in `crates/octowhere-ui/src/ui/prototypes.rs`. It draws into a `chrome::CoverageTarget`: an
embedded-graphics `DrawTarget` over an RGB565 framebuffer that also takes antialiased coverage a
row at a time.

- **Fonts:** three, indexed by `chrome::SHAPIRO`, `FRAKTION` and `FRAKTION_BOLD`.
  - Marathon Shapiro Wide 65, the display face, printable ASCII.
  - PP Fraktion Mono Regular, printable ASCII.
  - PP Fraktion Mono Bold, printable ASCII and `°`, built from the full font file with fontdue's
    `chars:` option. Any other glyph in the font files can be added the same way.
- **Upright text:** any size, aligned in a box with `draw_aligned`, or with its pen on a given
  baseline with `draw_on_baseline`. Each glyph's coverage is cached by font, glyph and size, so
  repeated text costs little.
- **Rotated text:** `draw_rotated` centres a string on a point, turned by any angle. The cardinal
  letters use it. It rasterizes every glyph every frame, so it is the costliest text.
- **Text bounds:** `aligned_bounds` and `baseline_bounds` give a string's ink bounds before
  drawing. The tests use them to check that every readout fits the slab.
- **Fades:** a colour faded toward the black field with `RgbColorExt::lerp`. There is no group
  alpha; each faded element is drawn in its faded colour.
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
  frame, so the compass redraws in full, against a 20 ms sensor period. Measured full draws,
  clear included, median of 16:

  | Frame | Draw |
  | --- | ---: |
  | Heading | 16.4 ms |
  | Interference | 17.0 ms |
  | Calibrating | 13.6 ms |
  | Top edge up | 12.8 ms |
  | No data | 11.9 ms |
  | Entering, 150 ms after settle | 13.2 ms |
  | Swipe just begun, dial still showing | 17.2 ms |
  | Swipe halfway, dial faded out | 12.6 ms |

  Clearing the visible circle alone was about 8.3 ms before this design. Rotated text is the
  costly element. A design that adds rotated labels, or large antialiased areas, will lower the
  frame rate. Ask for a measurement before committing to one. The bench is
  `bench/compass-states`.
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
- The heading is withheld until calibration completes, and the dial shows only while there is a
  heading.

## Working on it

Build and check:

```text
cargo build --release --offline
cargo clippy --release --offline -- -D warnings
```

The owner flashes the board and judges the result on the panel. Keep changes inside
`compass_screen.rs` and its constants unless a design needs new data or a new primitive. Say so when
it does, since both are more than a visual change. Commit messages follow "Writing conventions" in
`AGENTS.md`. Leave committing to the owner unless asked.
