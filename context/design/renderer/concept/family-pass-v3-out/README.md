# OCTOWHERE family pass V3 — settings alignment and color

`family-pass-v3-board.png` is the current combined screen review at 466 × 466 per tile. K1 remains the owner's chosen clock direction, S1 the selected settings overview, C1 the selected compass visual direction, and H2b the proposed always-on treatment. D3 replaces the D2 setting subpage proposal. V1 and V2 remain historical comparisons; none of these concepts is a firmware capture.

## D3 change

- The selected value's **visible glyph bounds** are vertically centered within its slab. The offset, timeout and replay stepper values use this directly. Zone centers its two-line value within separate, balanced line regions. Brightness uses a 54 px value with clear top and bottom padding inside the 82 px slab. CANCEL/BACK, AUTO and DEVICE utility labels are centered in their own boxes too.
- Ordinary settings selections use violet `#B32BE5` from the Round 4 reference board's unused swatches, with black knockout type. This is a **proposed settings role**, not a previously approved token. It connects to the existing purple edge scatter while separating settings from the lime clock/identity. Orange remains for CLEAR SETTINGS and a simulated failure; blue remains the valid GNSS reading and related DEVICE symbol. S1 overview retains its selected sparse treatment.
- Brightness remains every whole percent from 10–100, not ten levels. The 47% fixture fills four cells and 70% of the fifth. The current firmware finger mapping and preview/CANCEL behavior remain the functional authority.
- The DEVICE scrolled end still includes attribution, `REPLAY START-UP` and `CLEAR SETTINGS`. The replay chooser is a proposal following the timeout stepper grammar; its exact built layout has yet to be captured. A demonstration fault must be visibly marked and not overwrite the live stored self-test result.

H2b is shown on the board from V2 unchanged: compact battery across states and UTC time in NO ZONE. STOPPED still uses dashes when the time itself is untrustworthy. Rendered time and telemetry are fixtures.

## Review files

- `family-pass-v3-board.png`: K1, S1/D3, DEVICE/replay/C1 and H2b together.
- `settings-D3-comparison.png`: direct D2 → D3 alignment/color comparison for offset, zone and brightness.
- Individual `settings-D3-*.png` files: native-size screens.
- `aod-H2b-reference.png`: unchanged H2b treatment beside earlier AOD states.

From `renderer/`, reproduce with Pillow and NumPy:

```sh
PYTHONPATH=. python3 concept/family_pass_v3.py
python3 concept/family_pass_v3_board.py
```

Check slab/text alignment, contrast, touch targets and the proposed violet at real panel brightness before accepting D3. No project archive has been created.
