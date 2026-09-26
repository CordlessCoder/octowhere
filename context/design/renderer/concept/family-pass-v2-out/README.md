# OCTOWHERE family pass V2 — 26 September 2026

`family-pass-v2-board.png` is the current combined review surface. Each screen tile is 466 × 466 pixels. K1 is the owner's chosen clock direction; S1 settings overview and C1 compass remain selected visual anchors. D2 setting editors, DEVICE/replay treatment and H2b always-on states are new proposals, with no firmware implementation claimed. The earlier V1 folder is kept for visual comparison; its brightness note about ten fixed steps is superseded by the Round 4 brief and this V2 pass.

| Screen set | What changed | Functional constraint |
| --- | --- | --- |
| K1 clock | No new changes from V1. Its calmer two-field purple rectangle pattern and 24-hour rail are now the selected visual direction. | The deterministic renderer is a concept surrogate for `ui::scatter`; check the actual hash, damage and round clipping on device. |
| D2 offset, zone, brightness, timeout | The selected value gets a lime slab with black knockout type. Neutral adjacent choices and S1's sparse outer arcs preserve hierarchy. | OFFSET still stores an offset/AUTO then opens the zone choice. Zone BACK returns to offsets. CANCEL restores the entered brightness; a drag previews and a tap keeps it. TIMEOUT remains a five-value stepper. |
| D2 brightness | 47% fixture lights four full track cells and 70% of the fifth. | The built editor accepts every whole percent from 10–100, mapping the finger across its track. Ten cells each represent a tenth of full; they are not ten selectable levels. |
| DEVICE/replay | Scrolled DEVICE shows attribution, REPLAY START-UP and CLEAR SETTINGS. The replay chooser uses the same selected-value stepper grammar as TIMEOUT; `GOOD` and `05 MAGNET FAIL` are example positions. | The actual chooser and its exact layout have not been visually captured. A good replay runs identity + logo card and returns to the clock, without rerunning the hardware self-test or brightness ramp. A failure selection is a marked demonstration of the chosen peripheral, not a claim of a live failure. Touch skip and live microtext follow the existing replay spec. The six labels match the self-test parts. |
| H2b AOD | Adds compact battery percentage and ten tiny cells to every state. NO ZONE now displays UTC 12:07 and labels `UTC / NO ZONE`. STOPPED retains unknown dashes; NO DATA stays red and sparse. Battery-low 12% turns orange; unknown reads `BAT --`. | LOCAL remains H2's rule-free hour rail and minute-bound blue blocks. Battery is independent of clock validity. UTC in NO ZONE is a new owner direction, superseding the built AOD's dashes there; dashes still apply when time itself is untrustworthy. Actual AOD luminance, redraw on battery changes, pixel shift and panel clipping need device checks. |

The rendered telemetry and times are fixtures, not current readings. The DEVICE attribution is a visual spacing fixture; use the current bundled license/version strings. The CLEAR drag is carried from V1, and its three-line warning in the built firmware supersedes V1's older two-line placeholder. C1 top-edge/swipe remain V1 proposals.

## Review files

- `family-pass-v2-board.png`: K1, S1/D2, replay/compass and H2b together.
- `settings-D2-comparison.png`: D1 versus D2 for offset, zone and brightness.
- `aod-H2b-comparison.png`: H2 local and earlier missing-state study against H2b.
- Individual 466 px images are suitable for native-scale review.

## Reproduce

From `renderer/`, with Pillow and NumPy installed:

```sh
PYTHONPATH=. python3 concept/family_pass_v2.py
python3 concept/family_pass_v2_board.py
```

The scripts read selected assets from earlier concept folders and do not change firmware. No archive has been created.
