# Implementation response: HOLD LEVEL, hour rail timing, always-on level

For the design agent. It follows `IMPLEMENTATION-RESPONSE-2026-09-27-4.md` and your
`OCTOWHERE-DESIGN-REVIEW-IMPLEMENTATION-2026-09-27-4.md`, with the owner's changes since.
`DECISIONS.md` entries 9 to 11 record them. Captures are in `context/screen-captures/`.

## Owner's changes

- **HOLD LEVEL replaces TOP EDGE UP.** The compass used to withhold the heading only when the
  top or bottom edge was up. Any edge up has the same problem: the dial lies in the screen's
  plane and stops matching the ground. So the heading now goes when the screen stands within
  11.5° of vertical, whichever edge is up. The state line reads `HOLD LEVEL`, and the icon is a
  letter L on the same 5 × 5 grid as the other icons. The thresholds, the hysteresis and the
  750 ms grace are unchanged. This overrides the round 3 spec's TOP EDGE UP and the hand-off's
  wording for that state. See `compass-hold-level.png` and `compass-states.gif`.
- **The hour rail opens more slowly.** Your review kept it opening from its middle in step with
  the zone line, 240–400 ms into the entry. Eased over those 160 ms, most of the rail opened in
  the first few frames, so a swipe showed it all but at once. It still starts with the zone
  line, 240 ms in, but now opens at a steady rate over 400 ms, a cell each side at a time. The
  hour marker still waits for its cell. On exit it still closes with the zone line.
- **ALWAYS ON sets the face's level.** The cell used to toggle, and the face rested at 10 %.
  The owner found that too dim, since the face lights only about 8 % of the panel. The cell now
  opens `ALWAYS ON / 04`, the same D3 stepper as BRIGHTNESS and TIMEOUT, with the choices OFF,
  DIM, and every whole percent from 5 % to 50 %. DIM is the default. It is the level the
  timeout dims to, so it follows the brightness setting; a percentage stays fixed. With 48
  choices the stepper moves one choice per 20 px of drag, twice the rate of the other steppers.
  The cell reads `ON` with `DIM` or the percentage, or a gray `OFF`. See
  `settings-always-on.png` and `settings.gif`.
- **The device page's list lies over the scatter.** The page's cap drew its scatter after the
  list, so marks landed on the rows. The list is now drawn last. See `panel-device.png` and
  `panel-device-end.png`.

## Engineering changes with no visible effect

- **The circular clip is gone.** Some drawings were clipped to a circle of radius 232 inside
  the panel. The panel's own mask already hides everything past its edge, so the clip only cost
  time. The captures changed only past radius 232, which the panel never shows.
- **The clear and the solid fills are cheaper.** The panel now clears in runs of rows rather
  than a row at a time, and fills cover two rows per call. Partial compass draws and settings
  panel drags are both faster. Nothing drawn changed.

## Status

The owner asked for each change above, and nothing in them is open for the design.
