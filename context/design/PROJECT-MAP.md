# OCTOWHERE project map — 26 September 2026

## Read in this order

1. `SCREEN-SYSTEM-REVIEW.md` — current family review, proposed bounded tests.
   `SCREEN-FAMILY-BOARD.png` shows the screen set at native size; its map identifies every tile.
   `renderer/concept/family-pass-v3-out/family-pass-v3-board.png` is the current combined board. K1 is selected; D3 colored settings, replay and H2b remain proposals. V1/V2 remain for comparison.
2. `docs/marathon-ui-design-language.md` — portable design rules, including new lessons from this project.
3. `DESIGN-LOG.md` — chronology; its opening supersession note prevents older G2 decisions being mistaken for the current startup.
4. `references/VIDEO-TIMING-MAP.md` and `references/INDEX.md` — timecodes, use, still provenance.
5. The relevant `specs/` document, `inputs/briefs/SCREEN-DESIGN-BRIEF_4_2026-09-25-round4.md`, and the latest corresponding firmware capture. Older specs still describe built behavior unless a later accepted design explicitly changes that part.

## Current design authority

| Area | Current design / source | Status and caveat |
| --- | --- | --- |
| Startup identity | `renderer/concept/startup-g17-out/README.md`, `startup-g18-out/README.md`, `startup-g19-matched/README.md` and `identity-matched.gif` | G17 defines unlock, marks and 19-frame card; G18 centered title/plume exploration; G19 two-field rectangle plume is the selected visual target. The included Python point hash is a surrogate for the firmware's `ui::scatter`. No post-G19 firmware capture is present. |
| Self-test, fault, compass transitions, AOD/timeout | `specs/DISPLAY-AND-MOTION-SPEC.md` with subsequent brief/implementation decisions; `renderer/concept/startup-s1-out/` for the later self-test | S1's vertical six-row self-test supersedes the 3 × 2 captured layout as the visual target. Its G2 identity section is historical. Preserve functional holds; use the identity's 30 fps frame table only for startup. |
| Clock | `renderer/concept/family-pass-v1-out/clock-K1-*.png` and `renderer/concept/family-pass-v2-out/family-pass-v2-board.png`; Round 4 clock specs for behavior | K1 is the owner's chosen visual direction: quieter two-field rectangle scatter and a 24-hour rail. Not yet implemented. The dated firmware capture predates it. |
| Settings | `specs/SETTINGS-PANEL-SPEC.md`, `specs/SETTINGS-DESIGN-RESPONSE.md`; `renderer/concept/settings-g5-out/settings-S1-sparse-scatter-page-{1,2}.png` | S1 vertical index with eight options and sparse scatter in the outer arcs is the later selected visual direction, not a captured implementation. S2 matrix and blue-line variants remain studies. Existing behavior stays: cover returns to clock; only COMPASS restarts calibration; clear is a two-stage orange drag confirm. |
| Compass visual treatment | `renderer/concept/compass-c1-out/compass-state-sheet.png`, `compass-C1-noise.gif` | C1 is the later preferred entry/settled background concept. It retains the captured readout, dial and status semantics. Top-edge-up and swipe have only the older firmware captures on the board. |
| AOD visual treatment | H2 LOCAL in `renderer/concept/glitch-package/`; H2b variants in `renderer/concept/family-pass-v2-out/` | H2 is the preferred LOCAL concept, unimplemented. H2b adds battery and proposes UTC time for NO ZONE per the owner's latest direction; STOPPED and NO DATA remain distinct. Device luminance and redraw policy still need validation. |
| Core language | `docs/marathon-ui-design-language.md` | Portable source; exact OCTOWHERE constants and approvals belong to project specs. The older `inputs/marathon-ui-cross-project-handoff.md` remains provenance. |
| Current visual pass | `renderer/concept/family-pass-v3-out/README.md` and review boards | K1 selected. D3 uses centered text and proposed violet `#B32BE5` for ordinary settings selections; the replay stepper and H2b AOD remain proposals. C1 top-edge/swipe remain V1 proposals. None is a firmware capture. |

## Where the evidence is

- `references/firmware-captures/2026-09-25/`: device-generated clock, compass, AOD, startup and fault frames/video. Earlier compass captures are in the 24 September folder.
- `renders/`: earlier concept explorations, not uniformly current.
- `renderer/concept/startup-g19-matched/`: current four-second design GIF, stills, source and G18 timing input. Run `python3 matched_scatter.py` there with Pillow installed. The source retains the original title/marks GIF and replaces its dots with rectangle scatter.
- `renderer/concept/startup-g17-out/`: selected unlock, registration marks and card cadence; `startup-g18-out/`: historical plume study.
- `renderer/concept/settings-g5-out/`: selected S1 sparse-scatter pages and other studies.
- `renderer/concept/compass-c1-out/` and `startup-s1-out/`: later compass and self-test concepts recovered from the work following the old backup.
- `renderer/concept/family_pass_v1.py`, `family_pass_board.py`, `family-pass-v1-out/`: reproducible cross-screen design pass and review boards.
- `renderer/concept/family_pass_v2.py`, `family_pass_v2_board.py`, `family-pass-v2-out/`: selected K1 anchor beside colored editors, DEVICE replay and battery/UTC AOD proposals. V2 corrects V1's obsolete ten-step brightness description.
- `renderer/concept/family_pass_v3.py`, `family_pass_v3_board.py`, `family-pass-v3-out/`: current combined board and D3 settings. V3 centers the visible text in each selection and changes ordinary settings color to the reference board's violet swatch; H2b images are reused from V2.
- `handoffs/`: numbered implementation packages; Round 3 v5 was built. These are historical deliverables, not a claim that G19 exists on device.
- `history/concept-packages/`: legacy G-series packaged studies. Source and renders are also extracted under `renderer/`.
- `CONVERSATION.md`: session transcript and compacted earlier context, useful for provenance, not an approval index.

## Continuation sequence

First get a fresh firmware capture for the Round 4 clock and completed identity, then compare it with G19 and K1 at 466 × 466. Prototype selected S1 settings with D3 editor alignment/color and actual touch/scroll behavior. Capture the built replay chooser before settling its exact layout. Test C1 entry and H2b AOD at actual luminance; confirm battery update cadence and UTC on NO ZONE. Check actual `ui::scatter` hash, clipping and damage against the matched visual; do not copy the Python surrogate as firmware.

## Archive preparation state

`AGENTS.md` and `handoffs/IMPLEMENTATION-HANDOFF-CURRENT.md` now lead the video-free implementation backup. `ARCHIVE-PREPARATION.md` records the package and exclusions. The working tree retains source videos, while the backup deliberately excludes all MP4/WebM/MOV/MKV files. Its own `BACKUP-MANIFEST.sha256` covers only packaged files. Preserve reference stills and licensed fonts for the owner's private handoff; do not redistribute them as public product assets.
