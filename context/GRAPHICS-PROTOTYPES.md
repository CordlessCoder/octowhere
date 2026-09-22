# Graphics prototypes

`src/ui/prototypes.rs` renders one map/status screen through three architecture
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

The immediate prototype now has four live screens. Tap the header to cycle
between the field map, SI-unit IMU motion, RTC time, and touch diagnostics. RTC
and IMU values refresh every 250 ms. The touch screen shows the active point and
count. Acceleration is displayed in m/s² and angular velocity in rad/s.

Timing statistics are no longer part of the display composition. Enable the
`timing-log` Cargo feature to emit draw, vsync, flush, and swap timings through
defmt.

To compare them, change `ACTIVE_ARCHITECTURE` to `Immediate`, `Retained`, or
`Tiled`, then build and flash with the normal firmware command. The next useful
measurement is a touch-state transition that changes only the map callout and
footer, followed by a full-screen navigation transition.
