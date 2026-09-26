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
4. **Charging stripes may move every frame.** The clock's charging crawl moves 1 px a frame while
   charging. The screen still dims and may go to the always-on face while charging, so it is not
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
