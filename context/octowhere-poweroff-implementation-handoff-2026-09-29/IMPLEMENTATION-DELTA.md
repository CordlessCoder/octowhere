# Implementation delta: power-off confirmation

**Baseline:** the 29 September power-key flow and its placeholder screen. The owner accepted the new layout and motion direction in `references/power-off-state-board.png`. Replace the placeholder presentation while preserving the power controller's actual semantics and the existing app state on cancellation.

## Fixed hardware and interaction

- The controller reports a short or long press (1 s long threshold) up to roughly 250 ms after the physical event. Do not animate the key while it is depressed. A short press from normal UI rests the screen immediately on receipt, entering AOD if enabled or darkness otherwise; from dim/AOD/dark it wakes as a touch would. It has no separate page.
- A long press enters confirmation from any clock/compass face, settings index or subpage. From dim/AOD/dark, fade up to this confirmation over the existing **250 ms wake interval**. Do not flash an intermediate clock. From an active screen, cut to the confirmation as soon as the long event arrives.
- Only a slide whose handle reaches the far target **and is released there** confirms shutdown. A tap on the slider or an incomplete swipe must not power off. On incomplete release, return the handle to its start and keep the confirmation visible.
- Cancel by tapping anywhere in the top CANCEL cap, covering the screen, or ten seconds without interaction. Restore the exact prior screen, page, scroll position and display state. A short key press cancels and then rests the screen. Pause normal screen timeout while confirmation is open. Start the ten-second inactivity interval once the panel is fully visible after a dark/AOD wake; reset it on relevant interaction.
- On confirmation, fade the whole panel to black over **300 ms**, then switch the panel off and ask the board to power down. No UI action reverses confirmed shutdown. The controller's six-second forced-off path exists independently and has no screen animation. Startup ignores the powering-on key event and runs its normal self-test, identity and card.

## Rest layout at native 466 × 466

| Element | Target |
| --- | --- |
| Surface | Round gray ring and black ground, with restrained two-field purple registration scatter that thins toward the center. Use the current validated scatter primitive. |
| Title | `POWER OFF` in the existing settings Shapiro face at y28, white. No Maratype. |
| Breadcrumb | `SYSTEM / POWER` at center y65 in gray mono. |
| Top cap | Gray rule y83; CANCEL rectangle x83–182/y96–139, with the entire cap as the touch target; orange outlined power icon tile at x330/y92; gray rule y145. The tile has a 5 × 5 glyph (`00100 00100 10001 10001 01110`) at 6 px/module with 3 px padding. |
| Action text | `SLIDE TO POWER OFF` in orange mono at x91/y164. |
| Slider | Handle starts around x83–143/y225–289, 60 px wide; target around x323–383/y225–289; 240 px horizontal travel. A narrow gray guide joins them. Solid orange advances with a bold black right-arrow shaft and triangular head centered in the handle; far target has an orange outline until reached. |
| Help | `RELEASE IN TARGET TO CONFIRM` below the slider, followed by `HOLD KEY TO WAKE`. |
| Timeout copy | `AUTO CANCEL / 10 S` beneath the bottom rule at y370. It is static explanatory copy, **not** a ticking countdown. |

Use the existing `ORANGE` semantic for a deliberate action that requires care; this is not a fault and must not use red. The study rendered the D3 settings palette's orange sample (`#F67012`); reuse the firmware's shared orange token (color-role reference `#F1710D`) rather than introducing a second action-orange constant. Primary neutral copy remains white, secondary hints gray, the arrow black. Scatter must not carry safety meaning or obscure labels.

## Drag and confirmed states

While dragging, keep the track and arrow tied to the finger's horizontal progress, clipped to the start and target. Only release within the target confirms, matching the deliberate CLEAR SETTINGS gesture. The target's visual location and touch release bounds should agree. A cancellation while dragging restores the prior view, and an incomplete release resets the handle.

After a valid release, leave the orange handle at the far target with the track filled. Remove CANCEL, the instruction and lower hints. Show one white `POWERING OFF` line below the track. Fade **all rendered content**, including ring, scatter, icon and orange track, uniformly from full output level to black over 300 ms. `references/power-off-confirmed.png`, `power-off-fade-150ms.png` and `power-off-fade-board.png` give the stills. No extra logo, fault flash or second shutdown animation is needed.

## State ownership and performance

Treat the confirmation as a modal view over a saved prior state, not as a navigation page that loses the user's scroll or time editor. When canceling from AOD or darkness, return to that prior display state; the short-key cancel is the explicit exception that rests it. Keep the current touch-cover detection and other firmware input precedence, ensuring a finger on the slider is not classified as a cover. Redraw only what the current architecture needs for the drag; the Pillow study's full-frame redrawing is not a performance requirement. Validate touch target, brightness fade and actual panel-off on hardware.
