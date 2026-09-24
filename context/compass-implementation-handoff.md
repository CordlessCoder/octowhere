# Compass screen — implementation handoff

**Target:** Waveshare ESP32-S3-Touch-AMOLED-1.75, 466 × 466 round AMOLED. This document specifies the compass screen design, its input behavior and proposed motion. Implement against the actual `CompassView`, renderer and page navigation; judge final appearance on the physical panel.

The graphic language is industrial and editorial: black field, hard saturated slabs, black knockout lettering, strong type hierarchy and a small family of square-module status marks. The composition remains an original instrument UI.

## Device and data contract

- Framebuffer and touch coordinates: 466 × 466. Only the inscribed round area is visible; centre is the pixel corner **(233, 233)**. Draw a pure black (`#000000`) field and clip naturally at the circular display edge. AMOLED black is an off pixel.
- Panel is a round CO5300 AMOLED, driven through an RGB565 framebuffer; the touch controller is CST9217. Gradients can band, so the visual design uses solid fills and staged fades.
- The compass page is between Navigation and AxisCheck in an eight-page horizontally swiped ring. During a swipe both pages shift and clip horizontally. There is no conventional header or footer on this page.
- `CompassView` from `src/ui/compass.rs`: `live`, `calibration_percent` (0–100), `heading_decidegrees: Option<_>`, `pitch_deg`, `roll_deg`, `disturbed`. Heading is clockwise from **magnetic** north to the top edge, available only after calibration and withheld when the top edge is nearly vertical. Do not imply true north.
- Render priority: `!live` → NO DATA; otherwise a present heading with `disturbed` → INTERFERENCE; otherwise a present heading → HEADING; otherwise calibration below 100% → CALIBRATING; otherwise → TOP EDGE UP. Keep this precedence if an unusual combination of fields arrives. Only a **present heading** may display rotating indices or direction letters.

## Shared geometry and graphic rules

| Element | Design geometry at 466 px | Treatment |
| --- | --- | --- |
| Perimeter | Circle centred (233,233), nominal r 231, 2 px stroke | `GRAY`, except `RED` for NO DATA. Deliberately close to the visible edge. |
| Central slab | x 135, y 145, w 196, h 91 | Identical bounds in all states; white/orange/red background according to state, with black knockout text. Never resize between calibration and heading. |
| Top status icon | x 217, y 83, w 33, h 33 | 5 × 5 cells at 5 px, **4 px coloured padding** each side. Fixed in screen space. |
| Small caption | centred x 233, baseline approximately y 132 | `MAGNETIC`, `CALIBRATION`, or `COMPASS` as appropriate, PP Fraktion Mono bold, 16 px in the concept. |
| Main numerals | first glyph near x 143.2, baseline y 224 | PP Fraktion Mono bold, 86 px in concept; three zero-padded digits. Optical centring is achieved within the fixed slab. |
| Suffix | near x 298, baseline y 188 | PP Fraktion Mono bold, 40 px. Degree `°` for heading, percent `%` for calibration. Same position in both states. |
| Status below slab | centred x 233, baseline near y 276–278 | Direction abbreviation, interference warning, calibration instruction or top-edge warning; sizes below. |
| Tilt | centred x 233, baseline y 309 | `P +05  R -12`, PP Fraktion Mono 23 px gray. Always signed, two-digit magnitude, two spaces between terms. |
| Divider and hint | x 138–328 at y 321; hint centred x 233, baseline y 338 | 1 px gray divider and `COVER SCREEN TO RECAL` at 14 px, gray, in live states. |

The degree mark is a **real font glyph**. Add U+00B0 to the firmware's subset instead of drawing a separate circle. Zero-pad the calibration value as `000%` through `100%`; its last two digit columns match the heading's last two, and `%` occupies the `°` column. The shared first column and unchanged slab prevent a visual jump when calibration becomes a heading. Keep the slab width constant for every state.

The heading dial, when available, has major ticks every 30° (r 202–226, 4 px, `WHITE`) and minor ticks at intervening 10° positions (r 216–226, 2 px, `GRAY`). They rotate together with the heading. Four Shapiro `N E S W` glyphs sit around r 172; `N` is orange and the others white. Rotate each glyph tangent to the circle with its **top outward**, including inverted glyphs on the lower half. There are no bearing numbers or fixed tick; the top icon carries the fixed top-edge cue. The perimeter ring remains fixed.

Use `Marathon Shapiro Wide 65` for the large compass letters and `NO DATA`, and `PP Fraktion Mono` Regular/Bold for UI text and numerals. Use the supplied font files in the firmware renderer. Do not use a screenshot or bitmap in place of text. Text rotation and an arbitrary 2 × 2 transform are supported by the renderer, so the tangential labels can track the dial. Check font metrics and optical alignment on hardware; the listed positions are targets, not guaranteed rasterized glyph bounds.

| Token | Hex | Use in this screen |
| --- | --- | --- |
| `BLACK` | `#000000` | Field, knockout modules/text |
| `WHITE` | `#D2D3D6` | Heading slab, major ticks, most cardinal letters |
| `GRAY` | `#888E98` | Perimeter, minor ticks, captions and secondary copy |
| `LIME` | `#C0FE04` | Valid heading icon and direction abbreviation |
| `ORANGE` | `#F1710D` | Calibration, interference, north letter |
| `RED` | `#F24723` | **NO DATA only**: fault ring, slab and icon |

Use the existing semantic tokens in `src/chrome.rs`. Do not add decorative telemetry, fake IDs, a blue marker, gradients, soft cards or general-purpose QR decoration.

## The 5 × 5 status system

Each icon is a **solid state-colour tile**. Draw a 5 × 5 array of 5 px square black modules within it. Every row below has five columns; `1` means black rectangle and `0` leaves the tile colour showing. These are the icon bit patterns. They use the full grid without fixed corner dots or a reserved inner area. Build them with integer filled rectangles in firmware, not bitmaps or antialiased paths. All icons have the same 33 × 33 bounds, module size, padding and placement so future firmware icons can use the same grid.

| State / meaning | Tile | Row 1 | Row 2 | Row 3 | Row 4 | Row 5 |
| --- | --- | --- | --- | --- | --- | --- |
| Heading / fixed top-edge arrow | LIME | `00100` | `01110` | `10101` | `00100` | `00100` |
| Calibration / open loop | ORANGE | `01110` | `10001` | `10000` | `10001` | `01110` |
| Magnetic interference / noise | ORANGE | `10101` | `01110` | `11011` | `01110` | `10101` |
| Top edge up / top bar | WHITE | `11111` | `00100` | `00100` | `00100` | `00100` |
| No data / split | RED | `11100` | `11000` | `00100` | `00011` | `00111` |

The arrow is slender, and each tile has **4 physical pixels** of colour around its modules.

## State-by-state content

| State | Condition and central content | Dial / other content |
| --- | --- | --- |
| HEADING | Heading present, not disturbed. WHITE slab, `000°`–`359°` in black; caption `MAGNETIC`. Direction abbreviation from a 16-point compass (`N`, `NNE`, …) at y 278, lime, 32 px bold. LIME arrow icon. | Gray perimeter, turning ticks and outward-facing `N E S W`; tilt, divider and recalibration hint. |
| INTERFERENCE | Heading present and `disturbed`. ORANGE slab with black heading and degree glyph, `MAGNETIC` caption. `MAG INTERFERENCE` at y 278, orange, about 24 px bold. ORANGE noise icon. | Preserve the turning dial and bearing letters because a heading is still supplied, but warn that it is unreliable; tilt and hint remain. |
| CALIBRATING | Live, heading withheld, progress below 100%. ORANGE slab with three digit columns, normally `000%`–`099%`; `CALIBRATION` caption orange. `TURN ALL WAYS` about 19 px gray. ORANGE open-loop icon. | **Only the perimeter ring**, no ticks or cardinal letters. Tilt, divider and hint remain. Do not leave a frozen dial behind when calibration starts. |
| TOP EDGE UP | Live, calibrated, heading withheld because of orientation. WHITE slab with black `---`, `MAGNETIC` caption. `TOP EDGE UP` about 20 px gray. WHITE top-bar icon. | Only perimeter ring, no indices or bearing letters; tilt, divider and hint remain. |
| NO DATA | `!live` takes priority. RED slab of the same dimensions with black Shapiro `NO DATA`, `COMPASS` caption, RED split icon and RED perimeter. | **No ticks, direction letters, tilt, divider or recalibration hint.** Cover cannot restart calibration in this state. |

The `NO DATA` slab is fixed at the same x/y/w/h as the numeric slab. The calibration and heading digit columns and suffix positions are identical. The red outline is the perimeter ring, not a rectangular frame.

Display heading as three whole degrees from `heading_decidegrees` (integer division by 10, normalized into `000`–`359`); do not round `359.9°` to an out-of-range `360°`. The 16-point abbreviation uses the nearest 22.5° sector, with boundaries halfway between labels.

## Gesture and navigation behavior

1. Use the CST9217's **recognized screen-cover gesture** to start recalibration. An ordinary tap anywhere on this page does nothing. The screen itself shows `COVER SCREEN TO RECAL` in live states.
2. Accept a **fresh cover event** only when the compass page has settled, `live` is true and no horizontal drag is underway. Require hand release/uncover before another cover can restart calibration, so a held cover fires once. A successful event resets progress to `000%` and immediately removes the turning dial. The exact touch-controller event wiring and calibration-reset function are implementation work; the concept does not define an API.
3. A horizontal drag continues to navigate using the existing **16 px** drag threshold, horizontal page translation/clipping and wrap-around order. Navigation in progress takes precedence over cover recognition. Do not accidentally trigger calibration while entering/exiting this page.
4. While on the screen, use incoming `CompassView` as the state source. A missing heading must never leave stale ticks/letters, even if calibration reports 100%; show TOP EDGE UP then. A loss of `live` switches to NO DATA. If a valid heading returns, reveal the dial at the *current* heading, not at north/zero by default.

## Motion specification

These are restrained, overlapping opacity reveals over **black**. The centre slab, caption, status line, tilt and hint arrive with the page during the normal horizontal swipe; the dial accents begin after the swipe **settles on the compass**. A page merely crossed during a drag does not launch its entry animation. Fade via colour interpolation toward BLACK in RGB565 if the renderer has no group alpha; keep a state's semantics and draw order intact throughout the fade.

| Entry group | Delay after settle | Fade duration | Eligible states |
| --- | ---: | ---: | --- |
| Perimeter ring | 0 ms | 110 ms | All, red in NO DATA |
| Status icon | 95 ms | 110 ms | All |
| Dial indices | 190 ms | 110 ms | Heading and interference only |
| Cardinal letters | 285 ms | 110 ms | Heading and interference only |

The last stage ends around **395 ms**. Start these stages from the actual page-settled event and schedule the required redraws.

**Calibration completes on the visible page:** leave the ring and fixed-size slab in place; switch from percent to the acquired three-digit degree value, and from open-loop icon to arrow (or to the top-bar icon if the heading remains withheld). If a valid heading appears, fade the **rotated indices** in over **170 ms**, then begin fading `N E S W` **70 ms** after the indices start, also over **170 ms**. The complete dial is visible about **240 ms** after the first valid heading. Never show a momentary zero-degree dial. If calibration reaches 100% but the heading is `None`, show TOP EDGE UP with no dial; reveal the dial only when a heading actually arrives. If an accessibility/reduced-motion setting exists, show the acquired dial immediately.

**Swipe out:** once a drag is recognized, fade whichever of ring, icon, indices and letters are present together. Let normalized swipe progress be `p = abs(horizontal_drag_pixels) / 466`; the proposed accent opacity is `max(0, 1 - p / 0.35)`. The central content remains legible until ordinary page clipping removes it. Reversing/cancelling the drag restores the accents continuously; no second timer or replayed entry sequence. On completed navigation, stop compass-only animation work. Cancel queued fades when a newer state, cover event or gesture supersedes them. In static states, especially NO DATA, explicitly schedule animation redraws because sensor heading updates may not provide frames.

The ring and icon exist in all states; indices and labels exist only with a heading. Tune fade duration and RGB565 colour steps on the panel, and preserve full-brightness contrast after the animation.

## Firmware mapping and performance

- The rendering entry point is `draw_compass` in `src/ui/prototypes.rs`, over a `chrome::CoverageTarget`. Inspect the actual repository before modifying it. `smooth::Ring` and `smooth::polygon_quarters` are available for the ring and dial. Use integer rectangles for icon/tile/slab and fontdue text with the supplied font files. The renderer supports rotation and an arbitrary 2 × 2 text transform for bearing labels.
- Use the colour tokens from `src/chrome.rs`; add U+00B0 to the PP Fraktion subset. Maintain the RGB565 `OnBackground` contract: text over a coloured slab needs that slab colour as the known background, and text over the field uses black. Incorrect backgrounds make antialiasing fringes. Draw order remains significant.
- Keep icon patterns, token mapping and common slab geometry in a single source of truth. The 5 × 5 marks can be drawn as opaque rectangles with no per-frame glyph rasterization. Additional rotated labels or large antialiased surfaces would threaten the limited ~2 ms remaining at a 20 ms sample period.
- Measured heading-frame parts are clear **~8.3 ms**, labels **4.5 ms**, centre text **2.2 ms**, ring **1.6 ms**, ticks **1.4 ms**; total **~18.0 ms**. Calibration/no-data cost less because they omit ticks/labels, but animated redraws still need scheduling and measurement. Benchmark a live heading frame, each animated state transition and a swipe while both pages draw. The benchmark path is `bench/compass-draw-rows`.
- Progress is 0–100 in the real data. Under the stated state precedence, 100% with no heading displays TOP EDGE UP, not a frozen calibration dial; the three-digit slab is nevertheless sized to accommodate `100%`. Treat the state rules here as authoritative.

## Acceptance checks on the physical device

1. All five states are legible at native size and near the actual viewing distance; the red NO DATA outline and slab fit inside the circular active region. Verify text metrics with the real font subset, including `°` and `%`.
2. Heading and calibration share exactly the same slab bounds, numeric columns and suffix position. At completion the heading readout changes without a spatial jump. Inspect `000°`, `359°`, `000%`, a forced `100%` text-layout sample, and `MAG INTERFERENCE` for clipping; normal state selection may replace `100%` immediately.
3. The 5 × 5 icons remain 33 px with 5 px cells and 4 px coloured edge. The arrow is slim, every icon uses the same footprint, with no blue tick or numeric bearings.
4. Only heading/interference shows rotated ticks and cardinal letters. Calibration, vertical-edge and no-data states have no residual dial marks. Bearing labels stay tangent with tops outward at representative headings around the whole circle.
5. Cover is accepted once after release on a settled live compass page. Centre taps do nothing. Swiping and cover recognition do not conflict; NO DATA ignores cover. Transitions out of calibration wait for an actual heading and use the specified index/label reveal.
6. Entry waits until page settle; swipe exit fades reversibly; cancelled/superseded timers cannot reveal elements from an obsolete state. Check clipping halfway through a horizontal swipe.
7. Measure redraw costs and visible RGB565 fade quality on the panel. Adjust animation durations only after observing actual frames. Run `cargo build --release --offline` and `cargo clippy --release --offline -- -D warnings` where supported by the repository. The owner performs the flash and physical visual review.

**Implementation boundaries:** the symbol graphics and layout can be coded directly in the existing renderer, but the cover event, one-shot/release gating and page-settle callback may need input/navigation plumbing outside `draw_compass`. Do not simulate those events from a tap or invent state fields. If the touch controller's cover report is unavailable in the current input abstraction, expose it there explicitly and keep the behavior gated as above.
