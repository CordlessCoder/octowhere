# Implementation delta: identity, HOLD LEVEL icon and clock battery placement

**Baseline:** the owner's validated current firmware after the 26 September complete backup, the 27 September accepted fault/charging update, and the implementation agent's later scatter/compass/hour-rail changes. Apply only the differences here. Fixture time 13:07, battery 87%/12% and fault examples in PNGs are not device constants.

## 1. Identity and impact

Use the selected V11 identity in `studies/octowhere-identity-maratype-study-v11/`. At 466 × 466, the title's visible ink occupies `(33,177)..(433,289)` (right/bottom exclusive). Render `OCTOWHERE` in the supplied **Maratype.otf at 112 px**, natural x/y proportions, centered at `(233,233)`; do not compress its intentionally tall letters. The complete subtitle occupies `(33,309)..(433,338)` so its visible width equals the title's. The opening barcode uses the same twenty narrow/broad marks and order as before, stretched **horizontally as a whole** to about 113.625 px advance. The subtitle's square, digits, GNSS symbol and two microtext lines are sized to the same visible height, with 8 px inter-element gaps as in the source. Do not substitute an extra OCTOWHERE wordmark for the square.

The top-right companion mark is the selected small map pin with a **smooth round outer head**, long pointed tip and an octagonal void centered in that round upper section. Its origin is `(437,212)`; the source pattern uses 0.80 px modules at scale 1.17. The left/top and right/bottom registration marks are equal-arm **pluses** set diagonally 10 px beyond the actual title ink bounding corners (approximately `(23,167)` and `(442,298)`); 1 px stroke. Preserve their established arrival/flicker, not an always-on cross. The same silhouette and centered opening appear in the full-screen impact frames. The centered horizon glyph is the selected icon for the compass `HOLD LEVEL` state; `hold-level-centered-horizon.png` is its visual target. Preserve the rest of that compass state.

| Segment | Target |
| --- | --- |
| Identity | 120 frames at 30 fps = 4,000 ms |
| Title type-on | Begins 430 ms into the identity phase |
| Subtitle | Begins at identity local frame 21 |
| Outline/fill flicker | Begins at identity local frame 42 |
| Upper pin | Appears from identity local frame 56, retaining the prior partial arrival |
| Square dot | 30-frame / 1,000 ms brightness cycle, twice the earlier period; dark at local frames 22, 52, 82, 112 and peaks at 37, 67, 97; eased factor `0.5 × (1 − cos(2π(frame − 22)/30))` |
| Impact | Separate 19 frames after identity, preserving the existing cadence; the pin's octagonal hole remains centered |

The generated `identity-and-impact.mp4` has 139 frames at 30 fps. `startup-complete.mp4` has 203 frames (30 self-test context, 120 identity, 19 impact, 16 clock entry and 18 final hold), lasting 6.767 s. The contextual self-test and clock sections are older concept fixtures; retain the current firmware behavior outside the V11 identity/impact. The pin replaces the former upper logo; there is no duplicate logo in the subtitle.

## 2. Clock: wordmark-free composition and horizontal battery

Remove the experimental clock wordmark entirely. Retain the current K1 96 × 96 status icon at x262–358/y90–186 and its existing state semantics. Keep large hours/minutes and the rest of the current firmware's accepted composition, including its wider scatter and animated hour rail. In the band, keep `LOCAL`, `UTC`, battery copy and the seconds; place seconds at x388, baseline 244. Put the horizontal battery well below them at **x262–440, y281–311**. There is no colored one-pixel gauge outline. The black well stays to distinguish lime fill from the lime band. The measured interior is **x265–437, y284–308** (172 × 24 px). Fill proceeds left to right. The current percent or `BAT --` text remains above the well where that state has battery copy.

For known battery percentage `p`, measured length is `round(172 × clamp(p,0,100)/100)`. A noncharging known level is one continuous solid bar over that length. At charging hold, replace only that measured span with full-height **vertical** barcode slices; the rightmost measured endpoint does not oscillate. At the 87% fixture the 150 px span has **20 bars/19 gaps**, 62 lit px and 88 gap px, closely matching the identity subtitle barcode's approximately 41.5% mark-to-internal-gap proportion. The one/two-unit narrow/broad identity barcode data pattern supplies slice widths. The count follows fill length: three bars at 12%, twenty at 87%, twenty-three at 100% in the concept renderer. Preserve the asymmetric 72-frame / 2.4 s charging rhythm as a subtle redistribution of gaps; slice motion is horizontal within the well, with no moving hatch crawl, no changing measured endpoint and no periodic reset in the middle of a visible beat.

The prior accepted solid-to-segment principle still applies: render the segment layer over the solid of the same measured length and reveal separations by retracting the solid vertically across the 24 px well; reverse when charging stops. The **horizontal geometry's exact transition frames were not separately rendered or approved** in this study. Adapt the already implemented interruption/reversal behavior to this slot without reinstating the old 99 px vertical frame or changing the measured endpoint; validate entry/interruption on hardware. The packaged MP4 is the 72-frame **charging hold loop**, not that transition.

Known 0% leaves the black well empty. For the existing absent/unavailable `BAT --` branch, replace the two small gray dashes with a static **45° reversed hatch**: gray `#888E98` bands 15 px wide, 20 px black gaps, 35 px repeat, clipped to x265–437/y284–308. From top to bottom each band moves 24 px **left** across the 24 px interior. This is an availability texture across the whole well, not an amount. Apply it even if a clock state has no battery microcopy, including NO DATA. Do not animate it on charge. It supersedes the trial 20 px hatch and the older unknown dashes.

Keep the existing state color meanings: lime `#C0FE04` for local normal fill, orange `#F1710D` for a known low level at or below 15% and the STOPPED treatment, neutral white `#D2D3D6` for NO ZONE/NO DATA as appropriate to their bands; preserve the firmware's live data and state precedence. The red NO DATA face still omits the BAT microcopy, although its independent battery well may show a level or absent hatch. The compact AOD battery indicator is outside this change.

## 3. Typography and exclusions

Maratype is **not** approved on SELF TEST, SETTINGS overview or subpages, zone labels/values, compass NO DATA, clock NO DATA, AOD, battery copy or other functional text. The owner reviewed those mockups and rejected every shown context. Keep the current UI typefaces there. The older clock wordmark explorations are likewise rejected; no OCTO/WHERE or OCTO/ split remains on the clock face.

The accepted 27 September fault ticker, multiple-fault reporting, 4 s hold and damaged 600 ms exit remain unchanged. Settings, replay behavior, AOD, compass data handling, touch/skip semantics, real self-test deadlines, and sensor truth follow the earlier handoffs and current firmware. Do not infer a redesign of those screens from the identity study's contextual startup video.
