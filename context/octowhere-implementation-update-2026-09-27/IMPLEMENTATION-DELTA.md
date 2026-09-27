# Implementation delta: charging bar and startup fault

**Baseline:** the firmware implementation of the complete 26 September design backup. Apply the changes below to that implementation. The reference renders are 466 × 466 concept targets; source-video timecodes are not firmware timings. The older backup still controls all unaffected screens, state transitions, driver checks, touch handling and data semantics.

## 1. Clock battery gauge

### Replace the old treatment

The original handoff calls the battery interior a hatch and animates it by an upward 1 px/frame crawl on USB charge. **Replace both**. A known battery level uses one continuous solid fill whenever `charging == false`. When charging, divide exactly that same filled height into asymmetric, horizontal **full-width** bands separated by black gaps. Stripe motion is solely vertical. The top and bottom of the gauge stay fixed at the measured level throughout the loop; the entire bar never shifts sideways or changes width to express charging.

This applies to all six K1 full-clock states: GNSS, RTC, MANUAL, STOPPED, NO ZONE and NO DATA. Preserve each state's surrounding composition and existing gauge color: local GNSS/RTC/MANUAL lime `#C0FE04`, STOPPED orange `#F1710D`, NO ZONE and NO DATA neutral white `#D2D3D6`; known levels at or below 15% are orange. An unknown level remains static gray `--` with no inferred fill; 0% is empty. The existing NO DATA band still omits battery microcopy; adding it was a separate unaccepted proposal. Battery truth is independent of clock-time validity. The AOD's compact static cells are outside this change.

### Pixel geometry and periodic rhythm

| Parameter | Reference target |
| --- | --- |
| Gauge well | x 394–439, y 206–310; frame x 395–438, y 207–309 |
| Filled width | x 397–436 inclusive, always the complete width |
| Known fill height | `h = round(99 × clamp(percent, 0, 100) / 100)`; top `308 − h`, bottom 307 |
| Bands | `clamp(round(h × 11 / 86), 1, 13)` at nonzero h; 87% fixture gives h=86, 11 bands |
| 87% motif | Uneven 2–11 px bands, ten gaps of 1–6 px, combined gap height 25 px; x/y envelope fixed |
| Period | 72 frames at 30 fps = 2.4 s, repeating while charging |

The `source/render_fill.py` compositor specifies integer apportionment of band heights and gaps at other levels. At 12%, h=12 and the bar has two bands with one visible gap. Keep the bands legible as the segment count grows naturally with height. Do not make a uniform ladder. The loop starts with dispersed groups gathering during frames 0–17, holds an assembled irregular stack on 18–39, regroups into closer pairs during 40–53, holds on 54–61 and spreads toward its starting pattern on 62–71. Gap redistribution is staggered by band, with eased moves and no closed gap. The initial phase can follow the existing charging oscillator, except the entry wipe below holds the assembled phase until it completes.

### Change between solid and segments

Render the **segment layer above a solid layer** of the same color and height. On charging entry, move only the solid layer's top edge **downward** from the fill top to just below its bottom. Its retreat reveals the gaps from top to bottom. When charging stops, move that edge **upward** from below the fill to its top, covering the gaps from bottom to top. Keep the gauge frame, fill height and horizontal boundaries stationary. Use smoothstep easing, `3u² − 2u³`, for the layer edge in the review target.

| Case | Reference timing | Text and motion |
| --- | --- | --- |
| Known level ≥35%, charge entry or stop | 16 samples at 30 ms; destination key at 450 ms | `CHG` tracks the charging flag on the first frame; assemble/cover the bar independently |
| Short level <35%, including 12% fixture | 7 samples at 30 ms; destination key at 180 ms | Same direction and easing over less travel |
| Unknown or zero level | No interior wipe | Truthful text/state update immediately |

During the wipe, hold the assembled segment layout; after charge entry, start the 2.4 s loop at that layout so the first cyclic frame does not jump. If the signal reverses mid-wipe, preserve the current exposed fraction and travel toward the new endpoint with duration proportional to remaining distance. If battery percentage changes mid-wipe, recalculate height and retain the normalized exposed fraction. On page entry or wake while charging was already active, display segments directly; the charging-edge wipe is for a **change of charge state**, not every visit to the clock. The clock's existing page-entry battery slot may reveal the current gauge, but must no longer reveal a hatch or start a crawl. `CHG` is absent from the selected NO DATA microcopy as before.

Redraw only the gauge interior and its line on changes where possible, respecting the original page-entry, region-clear and round-clip rules. Schedule cyclic frames by elapsed time so a slow transfer skips to the due phase rather than stretching the loop. Profile the actual 40 × up-to-99 px interior and verify RGB565 colors and power cost at normal brightness.

## 2. Startup fault screen

### Preserve the implemented cause and hold

The six real tests, deadlines, six-row S1 status, first-failed-part priority and truthful reason stay as built. After the 300 ms any-fail post-decision hold, cut into the red fault composition in one frame, without a success identity. The full fault presentation still lasts **120 frames / 4 s at 30 fps**. A touch skip cuts immediately to the destination clock, consumes that touch until lift and does not play the new exit. A demo replay must be marked `DEMO` and must not replace the stored real hardware test result.

### Replace the white ticker

Retain the real red field, black horizontal strip, giant first-failed-part lettering, glyphs and factual labels. In the strip at approximately y 234–281, use **one** horizontally scrolling line, alternating the following every 12 frames (400 ms):

| Frames modulo 24 | Ink | Content for one MAGNET failure | Meaning |
| --- | --- | --- | --- |
| 0–11 | Blue `#001DFF` | Repeated `MAGNET_MAGNET_` | Failed part name(s) |
| 12–23 | Yellow `#ECDB0B` | Repeated `FAULT_FAULT_` | Generic fault motif |

Advance **both strings by 4 px per 30 fps frame using the absolute frame index**, even while one color is hidden. A return to blue or yellow therefore shows ongoing travel rather than replaying the same 400 ms displacement. The color switch is a hard cut; the strips do not scroll vertically. The two-lane/two-speed alternative was explored and rejected. This blue is a special fault-channel ink, **not** the normal `#409DE4` live-reading blue and not a claim of healthy subsystem data; yellow is likewise confined to this red fault state. Other uses of the original color-role table remain unchanged.

For multiple actual failures, retain their full set and individual reasons in canonical self-test order (`POWER`, `CLOCK`, `TOUCH`, `MOTION`, `MAGNET`, `GNSS`). The giant part and lower reason stay tied to the **first** failed test. Show the count and first two failed names in a fixed top summary with `+N` for any beyond two. Concatenate **all** failed names into the blue ticker in canonical order; yellow remains the generic fault word. The two-failure render uses MAGNET + GNSS as a synthetic fixture. The ticker cannot guarantee a viewer sees every name during a touch-skipped hold: keep all failure data available for later diagnostics, with its eventual list UI designed separately. Do not turn a missing GNSS fix into a self-test peripheral fault.

### Add damaged exit after the four-second hold

At natural timeout only, append the accepted **18-frame / 600 ms** damaged slide. Conceptually the red fault field is a single **466 × 880** rectangular surface extending below the round screen, clipped only by the stationary circular display aperture. The lower red is part of the same material, with a horizontal edge broken by large black block omissions. Do not draw a separate red rectangle with a gap, a thin residual red hairline, or the black square corners of a screenshot as part of the moving artwork.

| Exit frame at 30 fps | Visible action |
| --- | --- |
| 0, 0 ms | Intact last fault composition |
| 1, 33 ms | Bottom 58 px of initially visible content drops to black; coarse bites interrupt the edge |
| 2, 67 ms | Dropout reaches source row 338, just beneath giant letters; the broad upper cap above row 170 also drops away except three original-art fragments; roughly 35% of initial red remains |
| 3–16 | Surviving red, lettering and ticker translate upward over 340 px with quadratic **ease-in**: `offset_y = −round(340 × u²)`, `u = clamp((frame − 2)/15, 0, 1)` |
| 17, 567 ms | No red remains in the aperture; hand over to clock entry at 600 ms |

The dropout is an actual loss of initially visible red, including most of the lower fault screen, before the moving remnants leave. Keep the black damage attached to the surface. Render the destination clock after the slide clears; the review GIF shows a GNSS fixture, while a real failed clock peripheral must enter the actual NO DATA state. Total natural fault-to-clock time is roughly **4.6 s** after the fault cut. This is intentionally longer than the ~430 ms cinematic reference exit; the supplied timing comparison identifies the difference. A direct touch skip retains the original immediate path.

Use deterministic rectangles, saved original-art fragments, translation and round clipping in the firmware renderer. Source GIF timing is a visual target, not an asset pipeline. Measure full redraw/transfer cost for the 18 exit frames; a delayed frame should select the due elapsed-time state. Preserve real reason text and demo marking until the composition is removed.

## 3. Authority and integration checklist

- These two sections supersede only the corresponding old handoff/spec paragraphs about the **white fault running line**, **hard four-second fault exit**, **battery hatch**, and **charging crawl**. Do not reimplement the entire project from this archive.
- The original color-role document remains authoritative except for the fault-ticker blue/yellow exception above. The source study samples the dated fault capture red (`#EF4521`); retain the firmware's selected red treatment and verify it against the original color role `#F24723` on the actual panel, rather than silently changing the whole fault ground to match a compressed screenshot.
- Keep fixture values out of firmware constants. Reuse actual boot results, battery percentage, charging flag, clock validity and stored replay behavior.
- The reference compilers are concept Python. Where a 466 px raster differs due to font rasterization, RGB565 or the firmware scatter hash, preserve geometry, timing and truthful semantics and record the measured difference.
