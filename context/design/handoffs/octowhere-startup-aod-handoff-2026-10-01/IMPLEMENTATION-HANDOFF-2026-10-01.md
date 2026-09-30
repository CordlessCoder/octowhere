# OCTOWHERE startup and AOD handoff — 1 October 2026

Implement the approved self-test name typography, identity subtitle typography and clock AOD update. Keep the current BOOT label and fault typography. This package follows `IMPLEMENTATION-HANDOFF-2026-09-30.md`, which covers the selected active clock, compass, settings, compass background colors and halftone behavior. Those decisions remain in force. All designs here have been approved by the owner.

## Approved changes at a glance

| Screen / element | Decision |
|---|---|
| Self-test component names | KH Interference Bold 18 px; selected study D. |
| Self-test statuses, indices, metadata | Existing Fraktion Mono; OK/FAIL/-- remain Mono Bold 16 px. |
| Identity title | Existing Maratype 112 px, natural proportions. |
| Identity subtitle | KH Interference **Regular** 18 px for both complete lines; selected study B. |
| Vertical BOOT | Existing KH Interference Bold 38 px, clockwise 90°. |
| Fault scrolling sweep | Existing Shapiro 34 px. |
| Fault small diagnostics | Existing Fraktion Mono and existing geometry. No font change. |
| Clock AOD | KH Bold 136 px digits matching active clock centers/baselines; compact KH label + sans battery data. |
| Other screens | No new changes in this package. |

## Self-test: KH names with Mono statuses

Replace only POWER, CLOCK, TOUCH, MOTION, MAGNET and GNSS names with KH Interference Bold 18 px, preserving their existing visible-top positioning. Keep the Shapiro 27 px SELF TEST title, Mono Regular 13 px indices and DECIDED/version metadata, and Mono Bold 16 px status values. The successful, pending and failing colors, indices, icon rows, failure rail and component order are unchanged. Preserve the ring-free current family treatment.

The reference row tops are y89,134,179,224,269,314. Top/bottom rows use left=82,right=384; other rows left=62,right=404. Name ink starts at x=left+82,y=row+12; status remains right aligned to right−12 and vertically centered at row+23. Keep actual glyph ink positioning; changing nominal font size must not shift the name up or down unexpectedly.

Reference `references/startup/selftest-pass.png` and `selftest-fail.png` are native 466×466. `selftest-failure-fixture.mp4` is a 45-frame, 1.5-second typography fixture, **not a prescribed boot duration**. Hardware answer/deadline behavior remains authoritative. The original glyph-row construction cadence is 30 ms per row; retain the implementation's agreed behavior rather than resetting state when fonts change.

## Identity: KH Regular subtitle, fixed Maratype title

Use KH Interference Regular 18 px for the **entire** `VERSION 0.1.0` and `SELF TEST 6/6 OK` lines. Do not split labels and data into different fonts; do not use KH Bold here. Their visible tops are y309 and y325. Preserve title ink `(33,177)..(433,289)`, centered at (233,233), and the selected rounded needle logo with centered octagonal hole, diagonally registered plus marks and existing placement.

The complete subtitle still spans the title's 400 px visible width, x33..433. Maintain 25 px symbol height and original pixel-digit construction. For the normal reference copy, maximum text ink width is 172 px. Allocate the remaining width to the barcode, with no font distortion:

`barcode advance = 400 − (25 + 72 + 25 + maximum copy ink width + 4×8)`

For the reference strings this yields 74 px barcode advance; scale the original barcode's 67 px source advance by 74/67. The barcode begins x33. Four inter-element gaps are 8 px. The square begins at round(33+74+8)=115; the final copy block begins x261. Preserve bar-width differences and spaces inside the barcode. This formula concerns barcode geometry, not horizontal scaling of text or symbols.

Live version/result strings must fit. Measure the actual longest line in the supplied Regular font, and retain the established full-width composition. Validate the longer `VERSION 0.12.10` and `SELF TEST 5/6 FAIL` strings before fixing runtime positions; do not truncate status or draw beyond x433. If existing firmware uses a fixed version string and layout, match the supplied reference first; this handoff does not require adding a new animated reflow behavior.

Keep the octagonal unfilled-region square, its flashing dot, glyph height, micro-symbols and subtitle reveal order. Keep the current vertical BOOT label. References: `identity-subtitle.png` and `selected-identity-and-impact.mp4` under `references/startup/`.

## Animation timing map

All movie times below are local to the described phase, at 30 fps. No hardware self-test deadline is replaced by a render fixture.

| Phase / event | Existing timing to retain |
|---|---|
| Identity phase | 120 frames / 4.000 s; frame indices 0–119. |
| Title type-on starts | About 430 ms into identity. |
| Subtitle reveal starts | Frame 21 / 700 ms. |
| Title outline/fill flicker starts | Frame 42 / 1.400 s. |
| Upper pin appears | From frame 56 / 1.867 s. |
| Subtitle square dot | Existing 30-frame / 1-second period; do not accelerate. |
| Full-screen impact | 19 frames / 633 ms, indices 0–18, immediately after identity. |
| Selected identity + impact movie | 139 frames / 4.633 s; impact starts at movie time 4.000 s. Final impact frame 18 is black. |
| Fault held reference | 120 frames / 4.000 s. |
| Fault alternating colors | Every 12 frames / 400 ms. |
| Fault sweep travel | 4 px/frame / 120 px/s. |
| Fault exit | 18 frames / 600 ms; approved V8 dropout and quadratic ease-in. |
| AOD | Existing minute redraw and battery/state refresh; no continuous charging motion introduced. |

Preserve already implemented clock entry, touch skips and real diagnostic decisions. Changing typography does not introduce additional holds or replay delays.

## Fault: retain existing fonts and accepted motion

The user explicitly chose Shapiro for the sweep and the current Fraktion Mono for the small text. The alternative KH/Sans fault fonts are rejected. Keep the existing small-text positioning, weight and reporting; do **not** inherit the repainted A sample from the small-text study as a geometry update.

Keep blue `#001DFF` peripheral text and yellow `#ECDB0B` FAULT text. Both streams advance on absolute phase while hidden; switching colors must not restart the crawl. Retain the canonical peripheral order, existing first-two-plus-N compact summary and truthful simulation/replay labels. The giant background word/reason follows the first actual fault; reference screenshots contain the MAGNET fixture.

Keep the approved red-surface exit: a continuous red rectangle extending below the viewport, a dramatic removal of the lower content and broad top cap, three surviving upper fragments, and blocky knocked-out edges. No circular-surface gap or bottom red hairline. Local exit frame 1 removes the lowest content from y408 down; from frame 2 the lower cutoff is y338 and the top-cap knockout reaches y170, with surviving source rectangles `(40,92)..(96,158)`, `(176,102)..(280,156)`, `(344,100)..(414,158)`. The four lower-edge knockouts are x0..63/rise8, x104..159/rise20, x224..287/rise36 and x352..415/rise24. These coordinates describe the approved reference, not new motion proposals.

Slide offset is `−round(340*u*u)` px with `u=max(0,(frame−2)/15)` over 18 frames. The dropout precedes most travel; content disappears instead of merely moving out of view. Keep the actual firmware fault red; the old capture used by the reference renderer is RGB(239,69,33), not a new palette instruction.

Reference `references/fault/fault-single-A.mp4` contains the unchanged selected Shapiro sweep and 600 ms exit. `fault-multi-A.mp4` shows the unchanged two-fault held fixture. `A-blue.png` and `A-yellow.png` are native reference frames. None of these adds new fault behavior to an implementation that already matches the prior handoff.

## Clock AOD: match KH active-clock typography

This approved update supersedes the 30 September handoff's sentence “AOD typography is not changed.” Keep the accepted sparse H2b composition while applying the following typography.

| Element | Font | Geometry |
|---|---|---|
| Hours / minutes | KH Interference Bold 136 px | Individual visible glyph centers x103,x185; ink-bottom baselines y184,y305, identical to active clock. |
| Unknown time dashes | Fraktion Mono Bold 136 px | Same centers and baselines; retain quieter unavailable-time marks. |
| BAT label | KH Interference Bold 14 px | Entire BAT+value group ends at x367, ink top y96. |
| Battery value | Fraktion Sans Light 14 px | Same line; normal 87%, low 12%, unknown -- fixtures. |
| Local date | Fraktion Mono Regular 16 px | Existing AOD treatment; center x233, ink top y334. |
| UTC / NO ZONE; STOPPED | KH Interference Bold 16 px | Center x233, ink top y334. |
| NO DATA | Shapiro 28 px | Existing central red fault treatment near y256. |
| CLOCK under NO DATA | KH Interference Bold 14 px | Center x233, ink top y301. |

Render the digit positions from **visible ink**, with no stretching. In the reference, glyphs are drawn at 4×; the KH baseline anchor is adjusted by the maximum digit ink-bottom offset across 0–9, then individual glyphs are centered from their ink bounding boxes. Downsample using coverage; preserve the existing renderer's antialiasing equivalent.

Retain black ground, minute-bound blue bridge blocks, 24 hour-rail cells, muted hour marker, no perimeter ring, no seconds, no active-face filled band and no icon. GNSS, RTC and manual local time share the local face. NO ZONE uses trusted UTC and its UTC hour marker. STOPPED shows dashes when time is untrustworthy and has no active hour marker. NO DATA suppresses time and blue texture but retains the battery. Battery reporting remains independent of time validity.

Keep ten tiny battery cells at x276+i×10, y112..115. Normal battery ink uses RGB(132,142,154); unfilled cells RGB(24,38,57). <=15% uses orange `#F1710D`; unknown reads BAT -- and leaves cells unfilled. Do not apply the active-clock continuously moving barcode bar to this AOD.

Retain the existing redraw policy and shift sequence `(0,0),(3,0),(2,2),(0,3),(−2,2),(−3,0),(−2,−2),(0,−3),(2,−2)`. Translate all content together and then clip to the fixed circular aperture. Reference textures retain their prior dark blue palette; `source/render_aod.py` contains exact geometry and colors.

Reference map under `references/aod/`:

| File | Use |
|---|---|
| `01-aod-all-states.png` | Local, UTC/no zone, stopped, no data, low and unknown battery. |
| `02-active-aod-comparison.png` | Previous H2b, new AOD, selected active clock at 13:07. Previous tile is context only. |
| `03-time-fit.png` | 00:00, 08:59, 13:08, 23:59 digit-fit checks. |
| `04-pixel-shift.png` | Centered and shifted content/clipping. |
| `aod-local.png`, `aod-nozone.png`, `aod-stopped.png`, `aod-nodata.png`, `aod-low.png`, `aod-unknown.png` | Native 466×466 implementation validation targets. |

For the 13:07 image, lit area above channel intensity 16 rises from approximately 7.14% to 12.15%; normalized maximum-channel coverage rises from 4.38% to 8.60%. These are raster metrics, not measured power. Keep the chosen heavier typography and validate actual panel AOD luminance/power on the device.

## Implementation validation and source usage

Compare native screenshots at 466×466 against the selected references. Check self-test pending/pass/fail names and unchanged status alignment; complete identity subtitle and long-copy fit; 1-second dot period; complete impact; fault hidden-stream progress and unchanged exit; AOD local/UTC/stopped/no-data, battery 0/12/87/100/unknown and pixel-shift edge clipping. Preserve actual hardware diagnostics and replay truthfulness. Earlier active-clock, compass and settings changes remain as handed off on 30 September.

`source/` contains implementation-reading snapshots: AOD renderer, selected startup typography study logic, previous V11 identity source, self-test layout and approved fault baseline. Imports and relative paths refer to the existing full design working tree; adapt paths to your checkout. They are reference recipes, not a complete standalone build. The startup study source contains A/B/C/D experiments: only **D for self-test** and **B for subtitle** are approved. Existing supplied fonts are `KHB.otf`, `KH.otf` (Regular), `MonoB.otf`, `MonoR.otf`, `SansL.otf`, `Shapiro.ttf` and Maratype. Do not substitute KH Bold for Regular or synthesize/stretch font weights. Use licensed production KH assets before release.

Original cinematic/logo source videos are omitted; the user will provide them. Our rendered MP4 validation references are included. No new source-video timestamps or reference interpretation are required for these typography changes.
