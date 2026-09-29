# Power-off confirmation validation

Use native 466 × 466 captures from the current firmware. Compare the changed screen against the references while allowing the existing firmware scatter hash, RGB565 quantization and font rasterization to differ slightly from Pillow. The fixed text, position and interaction should be apparent at normal and dim brightness.

| Capture or exercise | Reference / expected behavior |
| --- | --- |
| Long key from active clock, compass, settings index and a settings subpage | `references/power-off-rest.png`; immediate full-screen cut when the long event is reported; no press-following animation |
| Long key from dim, AOD or dark | `references/power-off-entry-board.png`; fade directly to the confirmation over 250 ms, with no intermediate clock frame |
| Drag partway | `references/power-off-sliding.png`; orange track and black arrow follow finger; release short of target resets without shutdown |
| Release inside far target | `references/power-off-confirmed.png`; handle stays at right, CANCEL/instructions disappear, `POWERING OFF` appears |
| Shutdown | `references/power-off-fade-board.png`; whole composed panel uniformly darkens at 0/150/300 ms; panel then turns off and board powers down |
| Cancel cap, cover, idle ten seconds | Exact prior page and display state return; timeout held during modal; no shutdown |
| Short key during confirmation | Modal cancels and display rests according to AOD setting, as fixed by the power-key behavior |
| Power-on key and forced power-off | Power-on proceeds to normal self-test without triggering a new modal; controller's six-second forced-off does not depend on UI |

`references/power-off-state-board.png` is the single visual review sheet; `power-off-family-comparison.png` shows the relationship to the existing CLEAR SETTINGS slide. A physical-panel check should verify contrast of gray hints, orange visibility at the selected brightness, the release hitbox, cancel/cover precedence, and that the shutdown fade actually reaches a dark panel before power removal.
