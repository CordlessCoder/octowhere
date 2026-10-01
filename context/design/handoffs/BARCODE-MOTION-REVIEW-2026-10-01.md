# Barcode motion comparison — 1 October 2026

Closer comparison requested by the owner reveals two shortcomings in the charging-bar motion. The earlier capture review found the overall visual composition coherent but did not examine this motion mechanism closely enough. Its “no corrections” statement should not be used to close this charging-motion issue.

## Reference: joining and splitting

Re-examined the original supplied `marathon-official-logo-animation.mp4`, including consecutive 30 fps source frames around 2.4–2.8 seconds. The sampled strip `reference-joining-splitting.png` uses a full-resolution crop at x630,y1510,width296,height100.

Narrow bars dock beside wider bars to form visually contiguous groups. Those groups hold briefly and subsequently open again; the visible grouping changes. The sparse appearance/build earlier in the intro is a separate event from this joining/splitting beat. The effect is more specific than unequal spacing moving around.

## Current: separated bars re-spacing

The latest firmware charging recording still shows separate bars shifting within the charge-level envelope. The current design recipe actively prevents docking:

- `octowhere-clock-battery-placement-v3/render_study.py:title_pattern` keeps a positive gap budget and allocates every gap at least one pixel.
- It reduces the V6 gap variation to `1 + 0.35*(value/mean - 1)`.
- At the 150 px reference fill, the minimum allocated gap across all 72 logical frames is **4 px**. No gap reaches zero.

Thus the reference's change in grouping cannot occur in our recipe, regardless of how accurately firmware reproduces it. This is a design-source limitation, not just an implementation discrepancy.

`current-shifting.png` contains actual crops decoded from the supplied charging recording, enlarged by nearest-neighbor 2×. These are the current implementation, not a proposed replacement.

## Jitter: what is confirmed

The design renderer evaluates phase absolutely; it does not integrate prior-frame offsets. However, it sums dynamically rounded gap widths to obtain successive bar positions. In addition, the underlying V6 function renormalizes the entire gap vector on every frame and apportions its integer remainder again. A local gap adjustment therefore moves subsequent bars, and integer remainder reassignment can reverse their motion.

For example, the first moving bar's start in the 150 px reference span changes through `9,9,9,9,8,9,8,8,7...` during one transition. Another boundary follows `44,44,44,44,43,45,44,43,43,45...`. Those direction reversals are computed directly from the handed-off recipe, before video encoding. This explains a visible jitter mechanism without needing to assume frame-rate failure.

The implementation source was not supplied, so this review does not claim to have proved a separate firmware bug that accumulates displacement over time. Check for that too: firmware should render from immutable poses and absolute phase, not modify the previous frame's positions. The cumulative sums of per-frame rounded gaps are already enough to produce instability in the design source.

## Direction for the next motion revision

1. Define a small number of complete **absolute segment poses** within the battery's fill span. Keep stable segment identities and uneven widths, matching the approved barcode proportions and level-dependent count.
2. Include genuinely joined poses: selected adjacent gaps reach exactly zero. Narrow segments dock against wider ones, remain joined briefly, then split away. Joining is the union of existing segment rectangles, not an unrelated newly drawn bar.
3. Compose local groups deliberately. Stationary groups stay stationary while another docks/splits; avoid renormalizing every gap on every frame.
4. Interpolate each segment's absolute position from source pose to target pose using absolute local elapsed time. Quantize final coordinates once. Do not sum individually rounded animated gap changes or accumulate previous-frame offsets.
5. Make the keyframe poses preserve the fixed charge-level envelope and ensure the last bar ends at the real percentage endpoint. Any compensating space belongs in planned target poses, not a global per-frame correction. A simple shared progress per moving group helps preserve relative ordering and avoid accidental crossings.
6. Retain a quiet overall cadence every couple of seconds, with a small number of brief, legible docking/splitting beats and useful holds. Do not add continuous crawling.
7. Retain the accepted placement/orientation, full fill in noncharging states, percentage semantics, low/unknown treatment, solid-to-segments wipe and reverse wipe. No new layout or palette change is required.

Test the proposed motion with gaps actually closing, stable untouched groups, monotonic travel during individual moves, frame-skipping independence and a seamless loop. Segment counts should continue to scale with actual fill length. The future render should compare docking/splitting, not merely demonstrate a different irregular static pattern.

These are findings and a direction for a new review render. No replacement charging animation has been rendered or approved in this comparison.
