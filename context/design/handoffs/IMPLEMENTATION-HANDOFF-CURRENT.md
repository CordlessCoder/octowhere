# OCTOWHERE implementation handoff — current design, 26 September 2026

**Target:** Waveshare ESP32-S3-Touch-AMOLED-1.75, 466 × 466 round RGB565 AMOLED, `no_std` Rust firmware. This backup is a design package; bring the firmware checkout separately. Concept PNGs and GIFs are visual targets, not runtime image assets. The sample clock is 13:07:42, date 24 September 2026, Dublin; battery 87%, heading 047°, pitch +05°, roll −12°, compass calibration 54%. These are fixtures.

## 1. Authority and implementation order

The owner chose **K1 clock**, likes **V3/D3 settings**, and the selected visual set includes **G19 identity**, **S1 self-test and settings overview**, **C1 compass**, and **H2/H2b always-on**. H2b adds the owner's battery and UTC-with-no-zone requirements. No matching firmware capture proves these later visuals are already built. D3 replay chooser and C1 top-edge/swipe frames remain design proposals whose precise built layout/motion should be checked.

Use this order for conflicts:

1. The owner's later decisions in `AGENTS.md`, this handoff and `docs/OCTOWHERE-COLOR-ROLES.md` for the visual target and the explicitly changed AOD/brightness behavior.
2. `inputs/briefs/SCREEN-DESIGN-BRIEF_4_2026-09-25-round4.md` for what the dated firmware actually did, including its corrections to older specs.
3. `specs/CLOCK-FACE-ROUND4-SPEC.md`, `specs/DISPLAY-AND-MOTION-SPEC.md`, `specs/COMPASS-ANIMATION-ADDENDUM.md`, `specs/SETTINGS-PANEL-SPEC.md` and `specs/SETTINGS-DESIGN-RESPONSE.md` for detail that the later brief has not changed.
4. The selected render and its README for composition; historical ZIPs only for provenance.

In particular, the old settings spec says ten fixed brightness steps; **the built editor and current target use every whole percent from 10 through 100**. The old AOD spec and dated capture use dashes in NO ZONE; **the new AOD target shows known UTC time**. The old G2 identity description is historical; **G19 + G17 card** govern the current identity visual. The newer S1 six-row self-test supersedes the captured 3 × 2 composition, without changing what the hardware checks mean.

## 2. Color and shared drawing rules

Implement the formal table in [`docs/OCTOWHERE-COLOR-ROLES.md`](../docs/OCTOWHERE-COLOR-ROLES.md): lime identity/normal clock, violet `#B32BE5` active settings edit, dark purple texture, blue valid live reading, orange attention or deliberate action, red genuine fault, white/gray neutral information. State color takes precedence over the screen's ordinary color. The remaining reference swatches are reserved; do not add a role for them during this implementation.

- Clip all geometry to the round display; judge at native 466 px and at actual brightness. Use pure black as the field and black knockout ink on bright slabs. Keep the 5 × 5 glyph construction, hard rectangular fields, sparse registration detail and aligned monospaced data. Preserve the 2 px black clearance around field elements laid over scatter.
- Make status true without color: labels, icon, fill/outline and count must agree. `OK` in boot means only a peripheral responded by its deadline. GNSS boot `OK` is not a fix. A replayed failure is a marked demonstration, not a new live hardware result.
- At rest, texture is static. A second tick does not animate digits. Use damage-aware redraw for changing regions; do not redraw a full static scatter every sensor sample. Concept Python hashes, masks and simulated luminosity are not measured firmware output.
- The existing firmware `ui::scatter` chooses fixed hollow 6 × 6 or solid 4 × 4 rectangles on an 8 px grid from point index and seed; reuse its actual hash and region redraw. G19's Python hash is only a deterministic visual approximation. The K1 two-field clock study is also a concept surrogate.

## 3. Screen and state contract

### 3.1 Boot self-test S1

Render **six vertical indexed rows** with a canonical outlined glyph, part name and `--`, `OK` or `FAIL`: `01 POWER`, `02 CLOCK`, `03 TOUCH`, `04 MOTION`, `05 MAGNET`, `06 GNSS`. The status counter counts decided cells. Use `GRAY` for pending, `WHITE` for responsive and `RED` for failed. A passing glyph builds by rows at 30 ms per row; a failure cuts red in one frame. Keep the checks concurrent with startup, rather than delaying hardware behind ornamental animation. The saved self-test result feeds later identity microtext.

Driver deadlines as built: POWER and CLOCK **200 ms**, MOTION and MAGNET **500 ms**, TOUCH **600 ms**, GNSS **1.5 s**. A clock with readable registers but invalid time can pass its driver test and still show STOPPED time. GNSS passing means command/output response, not a position fix. The panel rises from dark to stored brightness during the first **200 ms**. After all cells decide and the last glyph finishes: all-pass hold **200 ms**, then identity; any-fail hold **300 ms**, then fault. Boot completion depends on the real checks; the approximately 2 s last-decision observation is not a fixed delay.

Validation: `renderer/concept/startup-s1-out/selftest-S1-all-pass.png` and `selftest-S1-fail.png`. Dated `references/firmware-captures/2026-09-25/startup-selftest*.png` prove the older build's data, not S1's layout.

### 3.2 G19 identity, impact card and replay

On a successful boot, play the selected **G19 four-second identity: frames 0–119 at 30 fps**. G17 establishes the blue BOOT unlock, outline title, frame/mark choreography, centered native-proportion Shapiro word, flicker and delayed small logo. G19 replaces G18's dot plume with two quiet rectangle fields. The upper field is centered (270,145), radius 150, facing −0.65 rad, density 0.65; the lower is (190,334), radius 125, facing 2.55 rad, density 0.40. Both use an 8 px grid with origin (12,−2). The lower field is static; the upper facing advances by +0.09 rad on identity frames **66, 77, 88, 99 and 110**. Scatter never touches rows 197–317, including a mark's full extent; below the band it shifts 2 px down. G19 uses dim purple `(24,7,62)` in its concept. The actual firmware hash and RGB565 value may differ slightly; match density, orientation, excluded band, incremental damage and calm hold rather than copying a Python bitmap.

Identity frame landmarks: 0–12 unlock/grid; 13 onward title field; the title's short flicker begins around frame **42** and the small logo around **56**; gray marks clear before the settled upper-field changes. Inspect `renderer/concept/startup-g19-matched/identity-matched.gif`, `outline.png`, `filled.png`, `settled.png`, and G17's `identity-G17-marks-timing.png` for the full beat. A frame due at n × 33.3 ms is selected by the clock; a late draw skips to the due state instead of extending the sequence.

**Then play the separate 19-frame card at 30 fps (0–18, about 633 ms):** 0–2 partial black-on-lime, 3–8 full black-on-lime, 9–12 lime-on-black, 13–16 enlarged lime-on-black, 17 enlarged black-on-lime, 18 fully black. The clock enters on the next frame and performs its own millisecond entry. Use `renderer/concept/startup-g17-out/identity-G17-card-impact.gif` and `identity-G17-card-timing.png` for the card. Do not accidentally fold those 19 frames into the 120-frame G19 identity or replay the S1 test as part of a normal replay.

`REPLAY START-UP` lives at the end of DEVICE. Its chooser offers `GOOD` and a marked demonstration of one specific failed part; the D3 chooser frames show the proposed visual grammar. A good replay runs identity frame 0, card and clock, with the current stored brightness (no boot brightness ramp and no new self-test). Microtext uses live UTC and the **stored last-boot test result**, even if the preceding boot failed. A simulated failure must say `DEMO`/demonstration and must not be stored as a new hardware result. The exact chooser UI has not been supplied as a firmware capture: verify its built navigation before replacing it with the D3 proposal. A touch during identity or card skips to the settled clock, ignores that finger until lift, and restarts the timeout when the clock takes over.

### 3.3 Actual startup fault

If a real peripheral fails, cut from S1's post-decision hold to fault K in one frame, **without a success identity**. The field is full red to the glass edge, with black band, failed-part glyphs, giant part name, repeated moving white `PART FAIL_` line and a truthful reason (`NO REPLY BY DEADLINE` or `REPLY NOT AS EXPECTED`). Several failed parts may be listed. It holds **120 frames = 4 s at 30 fps**, then cuts to clock. A touch skips to clock and runs its entry; the skipping finger is consumed. Use the dated `startup-fault.png`, `startup-failed.gif`, `specs/DISPLAY-AND-MOTION-SPEC.md` section 2 and the later Round 4 brief's edge/scale corrections. Keep a demonstration visibly identified as such wherever it uses this presentation.

### 3.4 K1 clock

Use the six K1 states: **GNSS local, RTC local, manual local, STOPPED, NO ZONE, NO DATA**. The normal local band is lime, STOPPED orange, NO ZONE neutral white, NO DATA red. The clock keeps the existing time hierarchy, wordmark, 5 × 5 state icon, UTC and battery lines in the band, lower date/mode/zone tokens and hatch battery column. K1 replaces the all-over purple field with two stable, 8 px-grid rectangle lobes (upper right denser, lower left quieter), and adds a 24-cell hour rail low in the circle. Only a known active hour gets a lime marker; STOPPED and NO ZONE do not pretend to know a *local* hour. NO DATA omits decorative scatter and local metadata so the cause reads first. K1 is the chosen visual direction; its generated hash is not the exact firmware pattern.

RTC/MANUAL/GNSS provenance remains legible in microtext. NO ZONE on the full clock can show known UTC; STOPPED or invalid oscillator time uses dashes. Battery must remain independent of the clock fault. At or below **15%**, its hatch is orange; unknown is gray; charging stripes crawl upward **1 px per frame** while charging, with only the battery region damaged. Test a long zone name, battery unknown/low/charging and the ring margin. See the K1 stills and `edge-state-check.png` in `renderer/concept/family-pass-v1-out/`.

Functional clock entry is **0–460 ms after page settling**, nominal 20 ms steps: scatter 0–120 in-quad, ring 0–110 out-back, icon 60–210 out-cubic, band label/rules 100–220, battery hatch 160–240 out-back, lower line 1 160–280, lower lines 2/3 240–400, wordmark 300–460 in-expo. Time types over 0–180 ms and date over 120–280 ms after startup card as built; a running tick replaces digits directly. Preserve the exit tied to page offset. Compare `specs/CLOCK-FACE-ROUND4-SPEC.md` for exact band geometry, field clearing and redraw regions. The dated `clock-*.png` captures precede K1 and should be used for data/behavior, not to veto the newer composition.

### 3.5 C1 compass

Five state precedence: **NO DATA** (actual orientation unavailable) → **INTERFERENCE** → **HEADING** → **CALIBRATING** → **TOP EDGE UP**, according to their conditions. Keep the existing dial, readout, pitch/roll, outlined 5 × 5 status icon and state wording. C1 adds a dim blue block/pixel field on entry and a stable quiet field at rest. NO DATA stays black/red and uncluttered. The C1 top-edge-up and representative swipe stills in V1 are extension proposals; the settled C1 heading/calibration/interference/no-data stills are selected anchors.

As built, interference enters after >35% field-strength deviation for **200 ms** and clears after within 30% for **1 s**; top-edge loses heading within about **11.5°** of vertical and returns only past **15°**. The heading is removed immediately when unavailable. On state change the slab wipes in four steps at **0, 20, 40, 60 ms**, the glyph builds a row each **30 ms**, a departing state line untypes for **60 ms**, incoming line/caption types over **120 ms**. A new heading dial sweeps **170 ms**; back from TOP EDGE UP within **750 ms** can restore dial/icon immediately. Actual fault cuts in one frame. Readout/tilt digit changes are hard cuts, never a decorative numeral animation.

The functional compass page entry completes at **440 ms**: ring 0–110, icon rows at 95/125/155/185/215, caption 120–240, dial sweep 190–360, divider 240–340 and hint 280–440. `compass-C1-noise.gif` illustrates a longer exploratory texture cadence (375 ms block, 125 ms dark, 375 ms tiles, 375 ms fragments, 375 ms refreshed block); it is **not an approved extension of the functional 440 ms entry**. Fit any C1 background build around the operational reading and measure it; hold the settled texture through heading updates. See `renderer/concept/compass-c1-out/` and `specs/COMPASS-ANIMATION-ADDENDUM.md`.

### 3.6 S1 settings overview and D3 inner screens

The panel opens with a downward face drag and closes upward/cover; a horizontal panel drag scrolls the eight cells. S1's selected **vertical index** shows four broad rows per page with sparse, static purple marks in the outer arcs. Page 1: `01 ZONE`, `02 BRIGHTNESS`, `03 TIMEOUT`, `04 ALWAYS ON`; page 2: `05 COMPASS`, `06 GNSS`, `07 BATTERY`, `08 DEVICE`. Preserve actual data, the state of ALWAYS ON and the special COMPASS action (restart calibration, close to compass). GNSS/BATTERY/DEVICE access the device details. The dated firmware grid used four columns/two visible and is an older visual; adapt its touch/scroll behavior to S1's actual hit regions instead of blindly retaining old cell geometry. The selected stills are `settings-S1-sparse-scatter-page-1.png` and `...page-2.png` in `renderer/concept/settings-g5-out/`.

**D3 is the current inner-screen design.** Offset, zone, brightness, timeout and GOOD replay selection use the reference violet `#B32BE5` as the active slab, with black text. The visible glyph bounds are vertically centered; brightness `47%` must have clear top/bottom margin. Neighbors stay gray. CANCEL/BACK/AUTO and DEVICE utility labels are centered within their boxes. Orange remains on simulated failure and the two-stage clear. These are layouts to implement and touch-test, not captured firmware views.

| Inner state | Preserve this operation | Validation render |
| --- | --- | --- |
| OFFSET step 1 | Drag offset list; choose offset or AUTO. A chosen offset leads to zone choice. CANCEL restores entry selection. | `renderer/concept/family-pass-v3-out/settings-D3-offset.png` |
| ZONE step 2 | Show zone and abbreviation; drag to choose, tap to store, BACK returns to offset list. Without a trusted date, list each zone under every offset it keeps during a year and show `--:--` rows. | `.../settings-D3-zone.png` |
| BRIGHTNESS | **Any whole 10–100%**: `p = clamp(round(100 (x − 66) / 334), 10, 100)`, controller `round(255p/100)`. Ten visual cells each represent 10%; at 47%, four are full and the fifth 70% full. Drag previews live; CANCEL/cover restores original, tap keeps. | `.../settings-D3-brightness.png` |
| TIMEOUT | Choose `15 S`, `30 S`, `1 MIN` (default), `5 MIN` or `NEVER`, through the existing stepper operation. | `.../settings-D3-timeout.png` |
| DEVICE top/end | Live version, battery/power/GNSS/satellites, attribution from bundled zone data and license, then REPLAY START-UP and CLEAR SETTINGS. Scroll vertically and keep the legal strings readable. | `.../settings-D3-device-top.png`, `.../settings-D3-device-end.png` |
| REPLAY | The D3 GOOD and `05 MAGNET FAIL` frames propose the same selected-value stepper grammar; successful replay versus one marked simulated part failure. Confirm exact built chooser navigation. | `.../settings-D3-replay-choice.png`, `.../settings-D3-replay-failure.png` |
| CLEAR | Tap clear from DEVICE, then drag an orange handle into the target and release. Early release resets it; CANCEL/cover leaves settings intact. Built warning: `ERASES ZONE, LAST FIX ZONE,` / `BRIGHTNESS, TIMEOUT` / `AND ALWAYS ON`. | `renderer/concept/family-pass-v1-out/settings-D1-clear-drag.png` for visual grammar; warning text there is obsolete |

An editor opening/return is a one-frame hard cut; its 5 × 5 icon can build by rows in **150 ms** and hint reveal. The older grid entry ends **420 ms** after settling (ring 0–110, title 0–120, rules start 40 for 160, icon rows stagger 80 + 30i for 150, indices/names then hint 260–420). S1 reorganizes the grid, so map the cadence to the new rows and avoid claiming untested old hit rectangles fit. On release, page/panel or grid snap uses **160 ms cubic ease-out**; a flick threshold is **600 px/s**. The picker list advances one row per **40 px** of drag and coasts with a **200 ms** decay until under **60 px/s**. A saved value should display its confirming frame before flash storage blocks for about **330 ms**. Read the later brief and `SETTINGS-DESIGN-RESPONSE.md` for these interaction details.

### 3.7 H2b always-on, timeout and rest

H2 LOCAL keeps large hollow regular-weight hours/minutes, a 24-hour low rail and restrained broken blue scanline blocks rather than full horizontal rules. The blue bridge changes only on a minute redraw; no continuous idle animation. H2b adds a compact **battery percentage and ten tiny cells in every state**, independent of clock validity. A battery at or below 15% is orange; unknown reads `BAT --`. The 87%, 12% and unknown renders are fixtures.

| AOD state | Time and label | Validation render |
| --- | --- | --- |
| LOCAL | Local HH:MM and date; normal hour marker. | `renderer/concept/family-pass-v2-out/aod-H2b-local-1307.png` and `...1308.png` |
| NO ZONE | **Known UTC HH:MM**, explicitly `UTC / NO ZONE`; use UTC hour, with a subdued marker. This supersedes the built AOD dashes in this state. | `.../aod-H2b-nozone.png` |
| STOPPED | `--:--`, orange `STOPPED`; no active hour. A time the RTC cannot vouch for is not shown as UTC. | `.../aod-H2b-stopped.png` |
| NO DATA | Sparse red clock fault label, no blue bridge/hour marker; battery still independent. | `.../aod-H2b-nodata.png` |

Redraw in full on minute, date or state changes as built. **Also redraw when displayed battery information changes** or coalesce with a bounded policy agreed with the owner; do not leave a shown percent stale indefinitely. This battery trigger is new design work. AOD pixel shift was not built in the dated firmware and remains a separate validation item. Test lit area, clipping, RGB565 color and real panel brightness; no concept still proves burn-in safety.

The stored timeout values above apply everywhere. At timeout, brightness fades to **30% of the shown level over 500 ms**, at least controller level 8 (3% of 255), never above the starting level. The dim phase lasts **5 s including that fade**. With ALWAYS ON enabled, cut to AOD in one frame at level **26/255** (10%) or the lower user-set level. With it disabled, fade dim-to-dark over **300 ms**, then send display-off/sleep. A wake fades up over **250 ms**; after true display-off wait **140 ms** for panel wake before page entry/fade. The waking contact is consumed until lift. A wake from settings discards a brightness preview and returns to clock. The rest timer restarts on contact, cover, page/panel/list movement, opening or closing a second level, and compass heading changes >10° from the last restart. It pauses during boot/replay and restarts when clock takes over. `NEVER` disables the dim/rest flow.

## 4. Timing summary and status

| Motion | Timing source | Duration/cadence | Design status |
| --- | --- | --- | --- |
| Boot hardware test | Round 4 brief + Round 3 spec | Per-part deadlines 200/200/600/500/500/1500 ms; all-pass hold 200 ms; fail hold 300 ms | Built behavior, S1 visual update |
| G19 identity | G17/G19 concept, `identity-matched.gif` | 120 frames at 30 fps = 4 s | Selected visual target, not firmware verified |
| Impact card | G17 `identity-G17-card-impact.gif` | Separate 19 frames at 30 fps ≈ 633 ms | Selected visual target |
| Actual fault | Round 3 + later brief | Hard onset, 120 frames at 30 fps = 4 s | Built K treatment |
| Clock entry | Round 4 clock spec | 0–460 ms; nominal 20 ms functional steps | Preserve functional cadence while drawing K1 |
| Compass entry/state | Compass addendum + later brief | Entry 0–440 ms; wipe 0/20/40/60 ms; icon 30 ms/row; state line 60/120 ms; dial 170 ms | Built semantics, C1 texture proposal |
| Panel entry/scroll | Settings spec/response | Entry 0–420 ms; snap 160 ms; icon 150 ms; editor hard cut | Adapt to S1 rows and D3 content |
| Timeout/rest | Later Round 4 brief | 500 ms dim fade; 5 s dim; AOD cut 1 frame or 300 ms off fade; 250 ms wake, plus 140 ms after off | Built behavior, H2b visual/data update |
| AOD | Later brief + H2b concept | Redraw on minute/date/state; add battery-change handling; no idle frame loop | New visual/data target, hardware test needed |

The G19 GIF contains **only** the 120-frame identity, not the subsequent card. The C1 experimental GIF is an entry texture study and does not replace the functional compass timeline. The source-video timecodes in `references/VIDEO-TIMING-MAP.md` are approximate visual references, never firmware timing. Source MP4/WebM files are omitted from this backup; the user will provide them if needed.

## 5. Validation map and acceptance work

| Compare | Selected render or evidence | What to prove on the target |
| --- | --- | --- |
| Whole family | `renderer/concept/family-pass-v3-out/family-pass-v3-board.png` | Hierarchy, colors, circle clipping, type scale across states at native size. |
| Boot test | `renderer/concept/startup-s1-out/selftest-S1-{all-pass,fail}.png` | Six rows and real asynchronous status/deadline truth. |
| Identity/card | `renderer/concept/startup-g19-matched/identity-matched.gif`; `renderer/concept/startup-g17-out/identity-G17-card-impact.gif` | Separate frame counts, line/flicker/mark placement, firmware scatter hash density and redraw cost. |
| Fault | `references/firmware-captures/2026-09-25/startup-fault.png`, `startup-failed.gif` | True failed part/reason, red edge, 4 s and skip behavior. |
| Clock | `renderer/concept/family-pass-v1-out/clock-K1-{gnss,rtc,manual,stopped,nozone,nodata}.png`; `edge-state-check.png` | All six states, long zone, battery modes, charging, hour rail and performance. |
| Compass | `renderer/concept/compass-c1-out/compass-state-sheet.png`, `compass-C1-noise.gif`; V1 top-edge/swipe stills | Five truthful states, settled texture, no reading obstruction, functional 440 ms entry. |
| Settings overview | `renderer/concept/settings-g5-out/settings-S1-sparse-scatter-page-{1,2}.png` | Eight cells, scroll/hit targets, complete values, static edge scatter. |
| Inner settings | `renderer/concept/family-pass-v3-out/settings-D3-*.png`; `settings-D3-comparison.png` | Visible-ink centering, violet contrast, whole-percent brightness, cancel and replay distinction. |
| AOD | `renderer/concept/family-pass-v2-out/aod-H2b-*.png`; `renderer/concept/family-pass-v3-out/aod-H2b-reference.png` | Battery in every state, UTC with no zone, unknown-time dashes, actual dim-panel legibility. |

For a firmware handoff, capture native 466 × 466 stills for each state, a 30 fps startup/replay/fault recording, functional gesture/entry recordings, and panel photos at normal and AOD brightness. Compare the new captures with these render paths. Document deviations explicitly: visual approval, as-built semantics, known hash differences, font/rasterizer differences, measured draw/transfer time, touch limits and open panel-brightness questions. Preserve the attributed zone data strings and owner-supplied font license terms. Do not infer pass/fail from fixture data in a PNG.
