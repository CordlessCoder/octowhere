# OCTOWHERE family pass V1 — review concepts, 26 September 2026

`family-pass-v1-board.png` assembles the proposed changes beside selected S1/C1/H2 anchors at native 466 × 466 per tile. `clock-K1-comparison.png` compares the earlier Round 4 clock against K1 with the same fixture. None of the D1/K1 changes has been approved or implemented. The project’s prior specs continue to define behavior unless a decision explicitly changes them.

`edge-state-check.png` checks charging, 12% battery, unknown battery, a long zone name and S1 ALWAYS ON selected. The proposed hour rail leaves space below the long-zone third line in this fixture; panel clipping and shifted positions still need review.

## Design direction

| Area | V1 proposal | Purpose and boundary |
| --- | --- | --- |
| Clock K1 (six states) | Keep the time, band, wordmark, icon and battery treatment. Replace the full-field purple scatter with two static 8 px-grid rectangle lobes, denser upper-right and quieter lower-left. Add a 24-tick hour rail low in the circle; only a known active hour has a lime cell. | The time and state slab read first; the mark vocabulary connects to G19 without copying the identity plume. NO DATA omits scatter, zone metadata and rail, leaving red/black cause. The pattern is a deterministic concept surrogate, not an exact `ui::scatter` point match or performance claim. Test the rail against a longest-zone third line, burn-in shift and native panel brightness before adopting it. |
| Settings D1 subpages | Carry S1's large SETTINGS header, page/index line, 5 × 5 glyphs, aligned 300 px content column, thin dividers and static sparse purple outer arcs into the offset picker, zone picker, brightness, timeout, device top/end and clear confirm. | Overview stays the selected S1 design. The subpages become family members while retaining their functions. A bounded white knockout marks the selected timeout; orange remains reserved for the two-stage clear. These are visual concepts, not touch-tested layouts. |
| Compass C1 completion | Carry the preferred C1 background into top-edge-up (quieter triangle field) and a representative swipe frame (held block field). | Keeps warning/readout foreground from the dated capture. Does not design an animated swipe or change heading/state holds. Verify how texture is clipped and moves with the page during the real gesture. |
| AOD H2 missing states | NO ZONE and STOPPED retain hollow unknown time, a faint fixed H2 bridge and 24 inactive rail cells; NO DATA removes blue and rail, retaining a short red interruption around the fault label. | H2 LOCAL remains selected and unimplemented. These variants address the absent states as proposals. No continuous movement: LOCAL blocks change on minute redraw; an unknown or stopped time has no active-hour marker. Check lit area, AOD brightness, pixel shift and whether any fixed texture should remain in STOPPED. |
| Fault, S1 self-test, G19 identity | Leave their compositions alone for this pass. | Their current hierarchy is stronger than additional borrowed ornament. Existing fault capture on the board is a comparison anchor, not a new design. |

## Functional notes to protect during implementation review

- **Zone picker:** D1 `settings-D1-manual-time.png` represents step 1, which selects the UTC offset or stores AUTO through the filled button. `settings-D1-zone.png` represents step 2; BACK returns to the offset list, and selecting the displayed zone stores it. The location/order values are fixtures from the prior renderer.
- **Brightness:** ten steps, 10–100%; the shown 47% fixture lights five cells. Drag previews the controller value; CANCEL restores the entry value; tapping keeps it.
- **Timeout:** the shown selection is 1 MIN, third of five choices; CANCEL and tap behavior remain as specified.
- **Device:** the top facts and scrolled attribution/clear state are separate renders. The attribution and license text must remain readable; the CLEAR SETTINGS control still opens the second-stage drag. The fixture values are not fresh telemetry.
- **Clear:** `settings-D1-clear.png` and `settings-D1-clear-drag.png` show start and 55% progress. Only a release in the target erases settings. The warning wording is the old fixture; confirm actual partition contents before implementation.
- **Compass and AOD:** C1 and H2 are concept output, not device captures. The new top-edge, swipe and non-LOCAL AOD frames in this folder are proposals.

## Files and reproduction

Run from `renderer/` with Pillow and NumPy available:

```sh
PYTHONPATH=. python3 concept/family_pass_v1.py concept/family-pass-v1-out
python3 concept/family_pass_board.py
```

The scripts reuse the existing clock renderer, S1 layout helpers, C1 captured foreground compositor and H2 concept assets. They do not alter firmware or overwrite the approved source renders. This board is a visual review tool; it does not establish device draw costs, touch target success or a final animation cadence.
