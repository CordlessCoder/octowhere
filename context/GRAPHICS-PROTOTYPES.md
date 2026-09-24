# Graphics prototypes

`crates/octowhere-ui/src/ui/prototypes.rs` renders one map/status screen through three architecture
boundaries. `ACTIVE_ARCHITECTURE` selects the version flashed by the firmware.

| Architecture | Ownership model | Strength | Risk to measure |
| --- | --- | --- | --- |
| Immediate | One ordered composition function | Smallest conceptual surface and easy screen-specific composition | A partial redraw can repeat work outside the changed element |
| Retained | Fixed scene nodes with per-node bounds | Natural place for stateful widgets and invalidation | The scene model can become a widget framework before the product needs one |
| Tiled | Fixed visual regions aligned with flush damage | Matches the current double-buffer and partial-flush path | Region boundaries can force unrelated content to redraw together |

The prototype uses a dark neutral field, saturated functional slabs, black
knockout text, an orthogonal map grid, block-built symbols, and coordinate
metadata. The map markers and their coordinates are fixed placeholder values,
not a position fix. The three renderers intentionally share visual primitives
so the comparison is about composition and invalidation rather than three
different art directions.

Text placement uses `embedded-layout`, including vertical header flow and
alignment inside action regions. Prototype labels use the compile-time fontdue
renderer; the separate femtofont implementation remains available for its
earlier benchmark comparison.

The first implementation is deliberately fixed-size and heap-free. It is a
probe for screen composition, dirty-region behavior, and the cost of the
architecture. It is not yet the application navigation model.

The layout keeps its visible geometry inside a conservative inscribed region of
the circular panel. The square framebuffer remains the drawing surface; the
panel naturally hides anything outside its aperture.

Touching a map marker selects it. The selection persists after release and
updates the marker, node callout, header status, and primary action.

The immediate prototype has seven screens: field map, motion, clock, touch,
power, navigation and compass. Drag sideways to move between them; the page
follows the finger and settles to the next screen or back. Tapping the header
moves one screen on. `ui::gesture` turns touch samples into taps and drags with
velocity, and `ui::pager` turns drags into the page offset the renderer draws
at. During a switch both pages are drawn, each clipped to its visible part.

The compass screen has no header or footer, because a round dial fills the round
panel. It turns under a fixed mark, and its bearing labels are drawn rotated
with fontdue. It withholds the heading until the magnetometer is calibrated by
turning the board through every orientation; tapping the centre starts again.
The calibration fits a sphere to the field by least squares, and the screen
flags interference when the corrected field's strength strays from it by more
than 35%. Once complete, the calibration keeps following the environment
(`ui::compass::Calibration`):

- Samples within 15% of the sphere keep joining the fit, which forgets old ones
  over about 300 spaced samples, so a slowly drifting offset is followed.
  Forgetting stops at 20 samples' worth of the completed calibration, so a
  board kept level does not lose the offset along the axis it no longer turns
  through.
- A sample outside 15% starts a candidate fit, which takes every sample from
  then on. It replaces the calibration once it covers as much as a calibration
  does, fits its sphere to within 2 µT RMS, and has a radius within 15% of the
  calibration's. A board whose own offset changed passes, because that offset
  turns with the board. A magnet passing by, or steel that weakens the field,
  does not.
- The candidate is dropped after 40 spaced samples in a row within 15%, or
  once it covers enough without forming a sphere. The second clears samples
  taken while a change was still happening, such as a magnet being brought up.
- Each fit works relative to its first sample, so a magnet fixed to the board,
  hundreds of µT from zero, still fits.

`host-tests/examples/replay_calibration.rs` replays a serial log's magnetometer
samples through it; the thresholds were tuned against `docs/logs/compass/`.
Heading and tilt come from `ui::fusion`, a Mahony filter: the gyro turns the
orientation every sample, gravity corrects the tilt while the accelerometer
reads close to 1 g, and the field's level part corrects only the heading while
it is calibrated and undisturbed.

The axis check screen follows the compass. It steps through twelve held poses
with vertical swipes, logs 1.5 s of raw magnetometer and accelerometer readings
per tap as a `[POSE]` line, and refuses a capture if the board moved.
`tools/fit-sensor-axes.py` fits each sensor's axes to the screen's from those
lines; `IMU_AXES` and `MAG_AXES` in `src/main.rs` hold the result.
Acceleration is displayed in m/s² and angular velocity in rad/s.

Timing statistics are no longer part of the display composition. Enable the
`timing-log` Cargo feature to emit draw, vsync, flush, and swap timings through
defmt.

To compare them, change `ACTIVE_ARCHITECTURE` to `Immediate`, `Retained`, or
`Tiled`, then build and flash with the normal firmware command. The next useful
measurement is a touch-state transition that changes only the map callout and
footer, followed by a full-screen navigation transition.
