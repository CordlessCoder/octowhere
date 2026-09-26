# OCTOWHERE screen family review — 26 September 2026

> The current combined board is `renderer/concept/family-pass-v3-out/family-pass-v3-board.png`.
> K1 is the chosen clock direction. D3 centered violet editors, replay route and H2b battery/UTC
> states are proposals; S1/C1 remain selected visual anchors. V1/V2 record earlier comparisons.

Reviewed together: current concept renders, 25 September firmware captures, the selected S1/C1/H2 directions and the completed matched identity. This is a design review, not an assertion that the latest concepts have shipped in firmware. See `PROJECT-MAP.md` for authority and paths.

| Screen | What holds the family together | Finding and next design test |
| --- | --- | --- |
| Startup self-test | S1 six-row index, 5 × 5 glyph grammar, pass/fail labels, hard color changes | The later S1 design matches the settings index without hiding any of six results. The earlier 3 × 2 firmware capture is stale for visual review. Keep its pass/fail cadence legible; do not add the identity's plume here. |
| Identity and impact card | Natural-proportion wide title, centered type, coordinated registration marks, lime/black inversion, rectangle scatter | This is the most expressive screen. G19 matched plume is the current design target; verify hash, color and damage on actual firmware. Its 30 fps flicker and 19-frame card belong to startup alone. |
| Clock | Lime normal-state slab, knockout minutes, small real-data lines, static purple scatter, 5 × 5 symbol and hatch battery | K1 is now the chosen visual direction. Its bounded two-field scatter and hour rail keep the resting time dominant; validate against a new firmware capture. |
| Compass | C1 keeps circular ticks, outlined symbol, heading/state and color roles, adding a dim blue field and a bounded page-entry build | The later C1 concept is the preferred visual direction. Its activity should stop after entry; calibration/interference use quieter treatments and NO DATA stays clear. Top-edge-up and swipe still need revised C1 views. |
| Settings overview, editors and device | S1 index uses four broad rows per page, 5 × 5 glyphs and sparse outer-arc scatter; D3 proposes violet selected-value slabs with vertically centered ink in offset, zone, brightness and timeout | D3 gives settings its own color role from an existing reference swatch, leaving lime to clock/identity. Brightness is continuous whole percent 10–100, with fractional filling in one of ten track cells. DEVICE includes built REPLAY START-UP; the rendered chooser is a stepper proposal pending capture. Keep CLEAR separate in orange. |
| Always-on | H2 removes horizontal rules, keeps an hour rail and places interrupted scanlined blue blocks between time groups | H2b proposes compact battery on every state and UTC time with explicit NO ZONE label. STOPPED uses dashes; NO DATA remains sparse. Verify battery cadence, brightness, lit area, pixel shift and clipping on the panel. |
| No data and fault | Reserved red, blunt state text, black knockout, strong contrast | Red is coherent across clock/compass/startup failure. The fault composition can be confrontational; do not let decorative noise compete with cause or exit. |

## Cross-screen decisions

1. **Shared construction, varied density.** The ring, square-grid symbols, hard slabs, registration details, mono data and bracket/comment tokens are the common vocabulary. Identity, clock, compass and settings should retain distinct centers of gravity.
2. **Color carries state.** Lime normal/identity, purple pattern, blue valid live reading, orange attention, red fault, white/gray neutral or unavailable. The clock spec has explicit exceptions and governs its states. Check the full state matrix when borrowing a color.
3. **Marks must register to content.** G19's gray crosshair arms and squares form a measured frame. S1's index/glyph rhythm and H2's hour rail connect the quieter screens by construction, without copying the identity's composition.
4. **Texture must be spatially stable.** The clock's approved scatter is static at rest. The identity's two-field plume changes only a small number of points at scheduled beats. Settings should first be tested as static, sparse edge texture; compass needs no texture to be coherent.
5. **Information beats visual effect.** Keep true UTC, battery, zone, heading, calibration and failure data. C1 entry texture settles before use and H2 changes on minute redraw. The primary reading has a solid or cleared ground and adequate round-edge margin.

## Priority for another pass

| Priority | Work | Review criterion |
| --- | --- | --- |
| 1 | Compare chosen K1 with a fresh firmware capture | Normal, no zone, no data and charging remain readable at 466 px; no pattern crosses a number or edge. |
| 2 | Implement/test the selected S1 settings direction | Both overview pages, selected cell and gestures remain clear; measure redraw and check panel brightness. S2 remains a study. |
| 3 | Validate matched identity in `ui::scatter` | Shape and cadence survive the real point hash and clipping; redraw stays inside the 30 fps budget. |
| 4 | Test C1 compass entry and H2b AOD states | C1 holds a stable field while heading updates; AOD shows truthful battery and UTC on NO ZONE at actual always-on brightness, with checked redraw cadence and pixel shift. |

No firmware change is implied by this review. S1, C1 and H2 are selected visual directions whose implementation details still require panel validation; older functional specs and implementation responses govern behavior until revised.
