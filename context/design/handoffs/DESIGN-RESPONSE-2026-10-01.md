# Design response — 1 October 2026

Reviewed `IMPLEMENTATION-RESPONSE-2026-10-01.md` against the approved 30 September typography and 1 October startup/AOD handoffs. The report is consistent with the accepted design direction. This review covers the report and the design sources; the new simulator/device captures listed in the report were not supplied with it, so this is not a new visual sign-off of those captures.

## Compass deviations

1. **Generator versus screenshot:** retain the implementation derived from `compass_noise.py`, including its state seeds, palette, highlight transform, settled multipliers and core dimming. The handoff named this generator as the reproduction source. Small plate/scanline differences in the selected type-study screenshot do not justify maintaining a second texture definition. Record the implementation's captures as the current visual baseline once reviewed.
2. **HOLD LEVEL dash alignment:** retain visible-ink centering in the slab. That matches the written layout intent. The higher dash placement in the screenshot should not override it. Keep the accepted centered horizon icon and prior stepped-tile HOLD LEVEL background.
3. **Foreground exclusion rectangles:** the reported 2 px rectangular exclusions are acceptable on the stated evidence that they disappear within the dim core. No demand to rebuild them as exact glyph masks. Check the native captures for visible rectangular voids around still text; the turning dial should remain free of a halo.
4. **Entry strength:** using each state's palette and the calibration/interference settled strength ratios is coherent with their settled appearance. Preserve the accepted timing.

## Halftone and owner-approved changes

Retain the reported 43% lobe / 9% quiet density law and coordinated tone variation. This expresses the approved variation in both coverage and mark brightness. Keep the existing shared 4×4 inset marks and the owner's confirmed breathing behavior; do not undo that decision to follow a static study note or replace them with 6×6 prototype marks.

The owner-approved zone-name scrolling supersedes reducing the type size: preserve the 1.2 s hold, 40 px/s travel, return behavior and clipping inside 13 px padding. Validate the selected names without changing the chosen KH type hierarchy.

The clock/identity scatter tone mapping is now an owner-approved implementation decision. Preserve it. The remaining design task is to inspect the actual clock and identity captures, including the identity animation's flicker/impact context, to document the resulting baseline. No unseen visual result is claimed approved by this response. Shared purple roles do not require equal perceived brightness across screens with different backgrounds and densities.

Retain the owner-approved fault ticker ink alignment using A–Z and underscore. The one-pixel baseline adjustment is compatible with the selected Shapiro sweep and does not alter the giant backdrop. Keep the current Mono diagnostics and accepted hidden-stream phase, colors and exit.

## Startup and AOD clarifications

- Self-test KH Bold names with existing Mono statuses match the selected D treatment.
- Identity KH Regular on both complete subtitle lines, x261, tops309/325, 74 px barcode and square x115 match the selected B treatment.
- Retain the owner's `SELF TEST n/6 OK` wording after a failed boot: n counts successful tests. Do not substitute the longer `n/6 FAIL` string or resize the row. Actual failed peripheral information remains available in the self-test/fault report. This owner decision supersedes the earlier long-copy stress example; it does not require adaptive reflow.
- Keep BOOT, Maratype title, logo, plus marks, 1-second dot cadence and animation timing.
- AOD battery group's visible end at x365 is compatible with the render's nominal right layout limit x367; nominal advance and visible ink are different measurements. Match visible reference geometry instead of adding a compensating offset.
- Using active-clock dash columns for AOD is coherent with the requested active/AOD match. Keep the restrained Mono marks.
- Retain existing AOD bridge/corner shapes and battery cells. Typography changes do not require replacing the firmware's established texture geometry.

The response explicitly confirms that pixel shift is still deferred. Record that limitation; the earlier handoff described a retained policy, but it should not be represented as implemented. Actual AOD luminance/power and identity frame timing are also still unmeasured. No new brightness or animation setting is prescribed here.

## Pixel shift: recommendations for the next separate pass

The existing nine-step sequence remains a good starting point:

`(0,0), (3,0), (2,2), (0,3), (-2,2), (-3,0), (-2,-2), (0,-3), (2,-2)`

For AOD, translate the whole composition together, including time, date/status, bridge blocks, hour rail/marker and battery label/cells, then clip to a fixed round aperture. Do not shift only the digits while leaving all other elements permanently lit in place.

Beyond AOD, the useful candidates are long-lived settled clock, compass, settings and DEVICE/replay screens. Use one consistent visual transform rather than shifting text independently of its slab/icon or leaving texture anchored while foreground moves. Keep touch targets aligned with displayed controls, and inspect edge clipping of full-width bands and the perimeter dial.

An elapsed-time cadence is preferable to advancing a shift on every repaint: a compass can repaint rapidly while an AOD updates once a minute. A phase change should not restart animation, scroll, charging or fault state. Apply the settled-screen offset without changing the internal motion of those elements. For the cinematic identity, impact and fault exit, preserve the authored composition; establish any steady offset at a phase boundary rather than introducing a new shift halfway through the transition.

These are recommendations for the future pixel-shift pass, not authorization for an additional implementation change. Owner cadence and scope remain to be settled in that pass.

## Next visual review

Supply the new clock and identity native captures and their charging/startup movies to review the toned scatter. Also include the centered HOLD LEVEL native capture to establish its current baseline. The filenames listed in the implementation response are paths in the implementation workspace, not attachments available to this design agent.

The reported compass render timings are recorded implementation measurements. They do not call for a visual redesign or speculative texture reduction in this review.
