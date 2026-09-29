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
10. **The hour rail opens over 400 ms** (owner, 2026-09-28), at a steady rate from 240 ms after
    the page settles, rather than with the zone line's 160 ms ease. Eased that fast, most of it
    opened in the first few frames of a swipe. It still closes with the zone line.
11. **ALWAYS ON sets the face's level** (owner, 2026-09-28). The cell opens a stepper of OFF,
    DIM and every whole percent from 5 % to 50 %, instead of toggling. DIM, the default, is the
    level the timeout dims to, so it follows the brightness; a percentage is fixed. The face was
    at 10 %, too dim though it lights about 8 % of the panel. A setting saved as on reads as DIM.
12. **The power key** (owner, 2026-09-29). A short press rests the screen at once, on the
    always-on face or dark, and wakes a resting one. A long press (1 s) opens a power-off
    confirmation over whatever showed: a swipe confirms; a cancel button, a cover, a short press
    or 10 s untouched cancels. The board powers on with a 512 ms hold. Entry 14 replaced the
    placeholder confirmation.
13. **The 2026-09-29 implementation update** (owner, 2026-09-29), in
    `context/octowhere-implementation-update-2026-09-29/`, approved in full and built. The
    identity takes V11: a Maratype title at 112 px, a full-width subtitle row, a map pin for
    the small logo and the card's mark, corner pluses for the ticks, and a square dot pulsing
    once a second. HOLD LEVEL's icon becomes a centred horizon, overriding entry 9's letter L.
    The clock loses its wordmark, moves its seconds right, and puts the battery in a wide
    outline-free well under the band lines: a solid fill, barcode slices while charging, and a
    static gray hatch with no level. Maratype stays on the identity's title only; the owner
    rejected it everywhere else. The owner cleared the font's redistribution for the public
    repository.
14. **The power-off confirmation** (owner, 2026-09-29), in
    `context/octowhere-poweroff-implementation-handoff-2026-09-29/`, approved and built. It
    takes the settings cap and the CLEAR slide's gesture, in orange. A cancel returns the
    screen to the rest the key woke it from, and the 10 s restarts on every touch.
15. **The design's reply to responses 2 and 3** (design, 2026-09-29), in
    `handoffs/IMPLEMENTATION-REPLY-2026-09-29-2-AND-3.md`. The pin stays at (437, 175). The
    battery's slices keep the 0.1.0 barcode's widths whatever version runs. The settings hint
    style stays 14 px everywhere. A cancel onto a dimming screen must not brighten it or flash
    the page; it now turns the fade back from the level that shows. The reply asks for the
    charging and power-off recordings, and a panel check, before it signs off the motion.
