# Owner decisions on the 26 Sep hand-off

Recorded 2026-09-26. These override the hand-off where they differ.

1. **Everything is approved.** Every new design in the hand-off is to be built: S1 self-test,
   G19 identity with G17's opening and marks, the G17 19-frame card, K1 clock, C1 compass, S1
   settings overview, D3 settings screens, H2b always-on, and `VIOLET` `#B32BE5` as the settings
   editing colour. Items the hand-off marks as proposals count too: C1's top-edge-up and swipe
   views, the D3 replay chooser and the other D3 screens, and H2b.
2. **Replay route.** The replay chooser and its route are already built. Only its look changes, to
   D3's stepper.
3. **Compass background stays blue.** The C1 field breaks the colour table's "blue means a valid
   reading" in letter only: its brightness and density are low enough that it reads as
   unimportant. It stays blue, including while calibrating.
4b. **No perimeter ring** (owner, 2026-09-27). Every screen drew a gray ring at radius 231;
   during a swipe the moving page's ring was cut off by the glass's circle. It is removed from
   every screen, including NO DATA's red ring on the clock and the compass.
4a. **The resting scatter breathes** (owner, 2026-09-27), departing from the hand-off's static
   texture: on the clock face and the settings overview its density falls to 75 % and back
   over 10 s, only while the screen is awake. Settings blooms its scatter in as the clock does,
   and both thin theirs as the page leaves.
4. **Charging stripes may move every frame.** The clock's charging crawl moves 1 px a frame while
   charging. The implementation update of 2026-09-27 replaced it with bands on a 2.4 s loop, and the owner added a 4 s rest between passes so the animation is less distracting. The screen still dims and may go to the always-on face while charging, so it is not
   a cost to avoid.
5. **Always-on battery updates with the minute redraw**, so the shown percent is at most a
   minute old and the panel never wakes for the battery alone. A STOPPED or NO DATA clock has no
   minute to redraw on, and its battery must still update every minute.
6. **The backup stays out of git.** `context/octowhere-design-project/` is ignored; this folder
   holds what the build needs. The older design folders in `context/` are removed.
7. **BOOT is set in KH Interference Bold**, as the design has it, embedded from the copy
   already under `assets/` (owner, 2026-09-26).
8. **The start-up's blue is `#000DF6`**, the colour of the intro cinematic's opening, added as
   a token of its own for the BOOT word and the grid behind it. It is an exception to board
   colours only (owner, 2026-09-26).
9. **HOLD LEVEL replaces TOP EDGE UP** (owner, 2026-09-28). The heading goes when the screen
   stands within 11.5° of vertical whichever edge is up, not only the top or bottom edge, since
   the dial lies in the screen's plane and stops matching the ground. The state line reads
   `HOLD LEVEL` and the icon is a letter L; the thresholds, hysteresis and 750 ms grace are
   unchanged. This overrides the round 3 spec's TOP EDGE UP and the hand-off's state wording.
