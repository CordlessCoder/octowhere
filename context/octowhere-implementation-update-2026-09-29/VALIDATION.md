# Validation against current firmware

Capture at the native 466 × 466 panel resolution. Compare changed regions at pixel scale while preserving already validated firmware scatter, rail and state behavior. RGB565 and font raster differences should be measured and noted, not covered by image blits. Use real data for functional tests; the renders use fixtures.

| Check | Primary reference | What to inspect |
| --- | --- | --- |
| Identity settled | `studies/octowhere-identity-maratype-study-v11/identity-frame-097.png` | Natural 112 px Maratype ink `(33,177)..(433,289)`; full-width subtitle `(33,309)..(433,338)`; small rounded-head pin and true corner pluses |
| Identity phase and impact | `identity-and-impact.mp4`, `maratype-motion-checkpoints.png`, `sequence-boundaries.png`, frames 056/082/112/119 in the same folder | 120+19 frames at 30 fps; onset, partial pin, dot period, outline/fill changes and centered octagonal impact hole |
| Full startup context | `startup-complete.mp4` | 203 frames, 6.767 s concept composition; compare only revised identity/impact to new design, retain implemented self-test and clock entry rules |
| HOLD LEVEL | `hold-level-centered-horizon.png` | Centered horizon icon only; current compass data and interaction unchanged |
| Clock baseline placement | `studies/octowhere-clock-battery-placement-v2/clock-unoutlined-solid.png` | No wordmark, unchanged large K1 icon and digits, seconds high at x388/baseline244, black well x262–440/y281–311 and no colored outline |
| Charging hold | `clock-unoutlined-charging.png`, `battery-title-barcode-proportions.png`, `battery-title-barcode-rhythm.png`, `clock-unoutlined-barcode-charge-loop.mp4` in V2 folder | 20 unequal-width slices at 87%, same amount endpoint as solid, no crawl or flickering percentage, 72-frame/2.4 s loop |
| Clock state coverage | `battery-unoutlined-all-states.png`, `battery-unoutlined-edge-states.png` in V2 folder | GNSS/RTC/MANUAL/STOPPED/NO ZONE/NO DATA; 12%, 0%, full, low orange and independent battery truth |
| No battery | `studies/octowhere-clock-battery-placement-v4/battery-hatch-15px-all-states.png`, `battery-hatch-15px-detail.png`, `battery-hatch-15px-neighbor-states.png` | 15 px gray bands / 20 px black gaps, flipped 45° direction, all states; 0% remains empty; no charge animation on `BAT --` |
| Charging edge | Earlier 27 September transition references plus this delta's horizontal adaptation | No visible level-endpoint jump, no old vertical frame, reversal from current exposure, no re-entry wipe when charge was already active. This specific horizontal transition has no approved V2/V4 movie. |

Check the settings, self-test, compass NO DATA, clock NO DATA and zone picker still use their currently approved typefaces. The separate Maratype UI exploration is deliberately omitted from this archive. Keep the implemented fault screen and its blue/yellow channel unchanged. For AOD, retain the existing compact battery treatment; this wide well is only for the full clock face.

The packaged videos were probed as 30 fps: identity+impact 139 frames/4.633 s, complete contextual startup 203 frames/6.767 s, and charging hold 72 frames/2.400 s. The original reference videos are not required to run these comparisons; the owner can supply them if a reference detail needs rechecking.
