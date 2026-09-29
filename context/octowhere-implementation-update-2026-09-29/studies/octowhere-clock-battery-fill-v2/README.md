# OCTOWHERE K1 clock battery — solid resting fill and charge transition

This is a **post-handoff design study**. It does not modify the previously handed-off firmware/design archive. It builds on `octowhere-clock-charge-states-v1` and the selected K1 clock stills in `octowhere-design-project/renderer/concept/family-pass-v1-out`. The render scripts reference that project at a sibling path; the PNG and GIF review assets in this folder are self-contained.

## Decision represented here

- At a known level, the clock battery gauge is a **single solid fill whenever charging is false**, including GNSS, RTC, MANUAL, STOPPED, NO ZONE, and NO DATA. Its height continues to encode the measured percentage.
- While charging, the same filled height is divided into uneven horizontal bands, spanning the entire gauge width. Bands shift only along the **y axis** on the established roughly 2.4-second cycle. The number of bands grows with fill height. The level never becomes an animated sweep or a shorter horizontal bar.
- The frame, percentage, and level remain present through the change. An unknown battery percentage has no inferred fill; retain the static `--` gauge. At zero, the interior stays empty. There is no wipe to perform in either case.
- This study retains the selected K1 gauge colors: lime for normal local time, orange for STOPPED or low battery, and white for NO ZONE and NO DATA. The earlier proposal to make a healthy STOPPED gauge white, and to add battery microcopy to NO DATA, remain **open proposals** in the preceding cross-state study; neither is introduced by this fill change. The AOD has its own compact static battery cells and is outside this clock transition.

## Charge entry and interruption

The bands are drawn over a full solid bar at the *same measured height*. The solid layer's **top edge moves downward** on charge entry, revealing the gaps from top to bottom. On charge interruption the edge **moves upward** from the bottom, closing the gaps. There is no horizontal motion or change in the charge level during the wipe.

| Event | Start | End | Status text |
| --- | --- | --- | --- |
| Charging begins | Solid visible beneath all bands | Solid layer fully retracted below fill | `CHG` follows the charging signal immediately |
| Charging ends | Bands and gaps visible | Solid layer covers every gap | `CHG` disappears with the signal immediately |

The review animation uses smoothstep easing and 16 frames at 30 ms for a normal height (final key at 450 ms), or seven frames (final key at 180 ms) for a short bar below 35%. These are **proposed timings for review**. The 12% bar has two bands and one visible gap. The bands hold their assembled pattern during the wipe and can resume the periodic charging cycle from its assembled hold after entry. The continuous cycle only runs when charging is active; NO DATA remains static in this study.

On an interruption partway through either wipe, retain the current solid-edge position and animate toward the new target. Scale the remaining duration by the distance to the target. Do not snap back to a fully segmented or solid endpoint. A measured level change during the wipe should update the fill height and rebase that edge by its normalized exposed fraction. When the clock appears while charging was already active, render the segmented state directly instead of replaying entry. AOD or sleep can render its own static representation.

## Review map

| File | What to inspect |
| --- | --- |
| `clock-solid-all-states.png` | Six known noncharging states at 87%; solid fill in each |
| `clock-solid-charging-pairs.png` | Resting/charging full-clock comparison for all six |
| `clock-solid-edge-states.png` | Low and unknown battery cases |
| `clock-charge-transition-storyboard.png` | Five moments in each direction for 87%, 12%, STOPPED, NO ZONE, and NO DATA |
| `clock-charge-transition-full-screens.png` | Full K1 composition at start, middle, and finish |
| `clock-charge-entry.gif` / `clock-charge-interruption.gif` | Normal level moving transitions |
| `clock-charge-entry-low.gif` / `clock-charge-interruption-low.gif` | Short-bar transitions |
| `clock-charging-across-states.gif` | Ongoing charging cycle at several levels and states |
| `clock-level-scaling.png` | One through thirteen bands as fill height increases |
| `render_fill.py` / `render_transition.py` | Reproducible PIL compositor and endpoint checks |

The stills and GIFs are illustrative K1 design targets, not firmware captures. The supplied fixture is generally 87%, with a separate 12% low-battery view. The original K1 design reference and implementation handoff remain in the earlier archive.
