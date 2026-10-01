# Design response: charging, startup and pixel shift — 1 October 2026

Reviewed `IMPLEMENTATION-RESPONSE-2026-10-01-2(1).md`, the accompanying `design-captures-2026-10-01-2.7z`, and its current `DECISIONS.md` / `SCREEN-DESIGN-BRIEF.md`. The owner-approved entries 23–26 supersede the earlier design proposals where they differ. The new captures are consistent with those decisions. No additional design correction is requested from this review.

This supersedes the earlier charging-motion criticism for the new implementation: the previous recipe could only re-space separate slices; this implementation genuinely docks and splits them. Preserve the new implementation instead of restoring the earlier design renderer.

## Charging: docking, splitting and stability

The first held loop beat in the supplied `charging/clock-charging.mp4` visibly changes the grouping, rather than just moving an always-separated pattern. To check this directly, I decoded the movie at native resolution and inspected the filled row across the 150 px reference charge envelope.

| Movie time | Observation |
|---|---|
| 4.30 s | 20 separated slices in the open pose. |
| 4.32–4.40 s | Gaps close progressively, with 18 connected groups at 4.40 s. |
| 4.42–4.94 s | 15 connected groups in the docked pose; several slices have become contiguous. |
| 4.96 s | The split begins, giving 18 connected groups. |
| 5.00–5.06 s | The 20-slice open arrangement returns. |

Counts are measured connected runs of colored ink in the decoded capture, not a change to segment identity or a new prescribed segment count. The underlying segments keep their narrow/broad proportions. The mirrored second beat also docks and splits visibly.

The outer fill endpoints stay fixed at the reference level. In the inspected dock transition, tracked boundaries move monotonically toward their target rather than showing the old per-frame reversal. For example, the narrow slice beside the left endpoint starts at relative x8, then x7, x5, x4, and joins at x3. Other groups remain still until their intended movement. The held pose stays stable, and the returning pose matches the initial arrangement. This is the intended correction to the cumulative rounded-gap movement in the old renderer.

The reference's joining/splitting grammar is now present. Keep the measured five-frame movement profile, joined hold, mirrored beats and roughly 2.3-second beat cadence recorded in the implementation brief. It need not reproduce the logo's pixel dimensions; it correctly adapts its grouping behavior to the fixed battery envelope.

Charging start and interruption also follow the owner's new decision: the solid closes toward the middle, the seed builds into end slices and then the barcode, and the reverse restores the solid. The recording includes interruption/reversal examples; the visual result retains one gauge and a stable percentage endpoint. Preserve the accepted absence of reference hairlines, placement, colors and low/unknown treatment.

The archive does not contain `ui/charging.rs`; this is capture-based evidence of the correction, not an independent audit of its interpolation code. Keep positions a function of immutable poses and absolute phase so skipped redraws cannot introduce drift. Do not reintroduce per-frame global gap apportionment or previous-frame displacement accumulation.

## Identity: continuous upper scatter turn

Keep entry 25. The change reduces the large batch jumps while preserving the authored total rotation and final composition. In the decoded identity-turn interval around 3.88–5.32 seconds of `startup/startup.mp4`, the upper purple field changes in 37 adjacent recorded-frame comparisons out of 72, instead of changing only at a handful of widely separated beats. Duplicate frames are expected when a logical 30 fps sequence is recorded at 50 fps. The lower purple region remains unchanged in that interval.

The field still reads as discrete marks, which is appropriate at this pixel scale; continuous angle increments do not require adding blur or moving the individual marks smoothly between grid cells. Keep its current tones, lower-field hold, title flicker, Maratype proportions, KH Regular subtitle, logo, plus marks and one-second square-dot period. No additional color, density or motion change is requested.

## Gauge after startup

Keep entry 26 and the difference between the two supplied startup recordings:

- `startup/startup.mp4`: after the impact and black separator, the charging gauge builds from the central seed into the barcode slices.
- `startup/startup-unplugged.mp4`: the noncharging solid grows outward from the fill's middle.

The gauge remains secondary to the clock's time reveal and does not visually compete with the impact. Preserve the specified start at 160 ms into clock entry, the shared flight curve and existing entry for arrivals from other pages. The screenshot sampling verifies the visible behaviors; it is not an independent certification of every scheduler deadline.

## Pixel shift as built

The owner has settled scope and cadence in entry 23. Keep the nine positions:

`(0,0), (3,0), (2,2), (0,3), (-2,2), (-3,0), (-2,-2), (0,-3), (2,-2)`

- Active screens advance on page/panel settling and wake. If none occurs, they advance at a minute boundary after ten minutes since the previous move.
- AOD advances on each minute redraw.
- Startup and replay hold at (0,0); the held active offset returns at clock handover.
- Shift the complete composition, retain the fixed glass aperture and subtract the visual offset from touch coordinates.

These policies avoid shifting midway through an authored startup transition and avoid treating high-frequency compass/charging redraws as shift triggers. Keep the extended r236 bands/rules/clear and edge repeat: the supplied page/panel transition captures do not expose a black sliver at the band ends.

The `pixel-shift/pixel-shift.mp4` samples show page/panel settling with the complete picture moved coherently. The cadence is accepted from the recorded owner decision and implementation brief; this short movie is not a ten-minute dwell test or an independent minute-by-minute AOD sequence test. Pixel shift is now built, so future design documents must stop describing it as deferred.

## Previously missing states and atlas baseline

The new archive includes direct native captures of AOD local, UTC/no zone, stopped and no data, plus failed self-test, one/two-fault held screens and the fault exit. Their typography, state hierarchy and battery reporting remain consistent with the approved design. Keep Shapiro sweep and Mono diagnostics. The AOD UTC digits and active-clock withheld local digits are intentionally different behaviors.

The settings overview, inner values, DEVICE and replay stills retain the selected Shapiro/KH/Mono hierarchy and centered selected slabs. Their current halftone, inset marks, tone law and breathing remain owner-approved. The whole atlas reads as one UI family; no new font or color experiment is requested.

Use the supplied `screen-atlas.png` as the current visual baseline. It contains 61 tiles in five sections at native 466 px per tile, headed with render revision `03b61ff`. The package identifies implementation master `776a3ad` and states that it draws the same screens. Preserve the fixed tile order and source filenames in later atlases so revisions can be compared directly. This baseline replaces older study boards as the first comparison target for future implementation captures; the original designs remain historical references.

The PWR/BOOT bezel arcs and simulator controls are review aids, not new on-glass UI elements. Keep the simulator private as described in the implementation response. No publishing action or new font licensing decision is part of this review.

## Review limits and remaining measurements

This review covers the supplied native stills, atlas and decoded sequence frames. It does not claim a fresh physical-panel usability test, a firmware source audit, or complete interactive testing of the web simulator. The owner's device validation is recorded separately in the response.

Retain previously noted measurement work for AOD panel luminance/power and identity frame cost; the captures do not establish those measurements. The reported flash-save optimization needs no additional design change. Keep all owner-approved behavior above, and use the current atlas and movies as the ongoing implementation baseline.
