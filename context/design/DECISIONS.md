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
    style stays 14 px everywhere. Its review of the recordings approves `power-off.mp4` and the
    vertical charging wipe. It asks that a cancel onto a dimming or darkening screen bring it
    back at the level it had reached and carry on with the time it had left, which is built.
16. **The charging wipe runs along the fill** (owner, 2026-09-29), overriding the vertical wipe
    that entry 15's review approved. Plugged in, the solid layer's end draws back from the
    level to the fill's start; unplugged, it grows back from the start.

17. **A double tap wakes a resting screen** (owner, 2026-09-30), on the always-on face and
    dark alike, instead of any contact. A single tap, a swipe or a hand does not. The touch
    controller watches for the taps in its gesture mode, which the owner took as the power
    saving; its low-power scan mode is not used.18. **The 2026-09-30 typography update** (owner, 2026-09-30), in
    `handoffs/IMPLEMENTATION-HANDOFF-2026-09-30/`, approved and built. KH Interference Bold
    sets the clock's digits and label, the compass readout and caption, the settings' row names
    and selected values; Fraktion Sans Light the clock's band lines and the offset's lower
    neighbour. The compass's settled fields are the design's own fixtures in its blue noise
    palette, and the settings halftone takes the S1 prototype's lobes with three purples that
    brighten with the density. The owner checked the clock in the simulator. Where the build
    departs from the reference renders, `SCREEN-DESIGN-BRIEF.md` says so: the compass fields
    follow `compass_noise.py` where the reference render differs from it, `---` centres in the
    slab, the fields keep off the foreground's boxes rather than its ink, HOLD LEVEL keeps its
    earlier field, the halftone's solid marks stay 4 px and it still breathes (4a).
19. **A zone line too long for the picker's slab scrolls** (owner, 2026-09-30), rather than
    stepping its size down: it holds at its start, runs to its end, holds and runs back.
20. **The clock face's and the identity's scatters vary their brightness too** (owner,
    2026-09-30), with the settings halftone's three purples: dense marks bright, sparse marks
    dark.
21. **The 2026-10-01 start-up and always-on update** (owner, 2026-10-01), in
    `handoffs/octowhere-startup-aod-handoff-2026-10-01/`, approved and built: KH Bold names on
    the self-test, a KH Regular subtitle on a 74 px barcode, and the clock face's KH digits and
    compact labels on the always-on face. BOOT and the fault screen are unchanged. After a
    failed boot the subtitle keeps `SELF TEST n/6 OK` (owner). Its pixel shift is not built:
    the owner will take it up separately, now that no ring stands in its way.
22. **The design accepts both updates as built** (design, 2026-10-01), in
    `handoffs/DESIGN-RESPONSE-2026-10-01.md`: the compass field from `compass_noise.py` over its
    screenshot, `---` centred, rectangular halos, the entry's palette strengths, the halftone's
    lobes, tones, 4 px marks and breathing, the scrolling zone lines, the toned clock and
    identity scatters, the ticker's centring, `SELF TEST n/6 OK`, and the always-on face's
    columns and blocks. The captures become the visual baseline once the design has seen them;
    it has not yet. Its pixel-shift recommendations are for that separate pass, not settled.
23. **Pixel shift's scope and cadence** (owner, 2026-10-01), settling what entry 22 left open.
    It applies to every screen, the always-on face included, with the 2026-10-01 handoff's
    nine positions. The active screens move as round 3 §4 says: at a page change settling,
    the panel opening or closing, or a wake, and failing those at a minute change once 10
    minutes have passed since the last move. The always-on face moves at every minute's
    redraw. The start-up holds at (0, 0) and shifting starts once it hands over. Bands cut at
    radius 232 are painted out to 236, and core 1 repeats the framebuffer's edge pixel where a
    shifted source falls outside it, so a shifted band always meets the glass.
24. **The charging bars dock and split as the logo's do** (owner, 2026-10-01), from
    `handoffs/BARCODE-MOTION-REVIEW-2026-10-01.md` and the Marathon logo animation itself, which
    the owner supplied with `handoffs/reference-joining-splitting.png`. The design's recipe kept
    every gap at 4 px or more, so the slices only re-spaced. Now the joined poses repeat the
    logo's gaps, docks and splits take its measured frames, and charging starts with the
    logo's build (without its hairlines) and stops with the build run backwards. The solid
    wipes in to the fill's middle before the build and back out from it after, on the curve the
    logo's end bars fly on. `SCREEN-DESIGN-BRIEF.md` has the timings. Placement, envelope,
    percentage and the low and unknown treatment are unchanged. The design has not seen it.
25. **The identity's upper scatter field turns on every frame** (owner, 2026-10-01). G19
    turned it by 0.09 rad on frames 66, 77, 88, 99 and 110, and each step flipped a batch of
    marks at once, which read as large jumps. It now turns by the same 0.45 rad in equal steps
    on every frame from 66 to 110, ending where it did. The lower field still holds still. The
    design has not seen it.
26. **The battery gauge builds in after the start-up** (owner, 2026-10-01). When the start-up
    hands over to the clock face, the gauge no longer rises from the left. While charging,
    the slices run their build from an empty fill, as when charging starts. Otherwise the
    solid grows out from the fill's middle, as when charging stops. Both start at the rise's
    160 ms. Other entries to the clock face keep the rise. The design has not seen it.
