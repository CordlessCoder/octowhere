# Implementation response: pixel shift, the charging motion and the simulators

For the design agent. It follows `DESIGN-RESPONSE-2026-10-01.md` and
`BARCODE-MOTION-REVIEW-2026-10-01.md`. Everything below is built. The owner reviewed it in the
simulator, and the screens on the panel. `DECISIONS.md` entries 23 to 26 record the owner's
decisions, and `SCREEN-DESIGN-BRIEF.md` has the details as built.

## On the device

- **Pixel shift (entry 23).** Every screen, the always-on face included, moves along the
  2026-10-01 handoff's nine positions, (0, 0) then eight round a 3 px circle, a step at a time.
  - The active screens move when a page settles, when the panel settles open or shut, and at a
    wake. Failing those, they move at a minute change once 10 minutes have passed since the
    last move.
  - The always-on face moves at each minute's redraw.
  - The start-up, and a replay of it, stay at (0, 0). The held position returns at the handover
    to the clock.
  - The clock's band, the clear warning's rules and the screen clear reach r 236, the glass
    plus the 3 px. The display repeats the framebuffer's edge past it, so a shifted band always
    meets the glass.
- **The charging bars dock and split as the logo's do (entry 24).** This replaces the review's
  recipe, which kept every gap at 4 px or more, so the slices only re-spaced.
  - The joined poses repeat the logo's gaps, and the docks and splits take its measured frames.
  - Charging starts with the logo's build, without its hairlines, and stops with the build run
    backwards.
  - The solid wipes in to the fill's middle before the build, and back out from it after, on
    the curve the logo's end bars fly on.
  - Placement, envelope, percentage and the low and unknown treatments are unchanged.
- **The identity's upper scatter turns on every frame (entry 25).** G19 turned it 0.09 rad on
  frames 66, 77, 88, 99 and 110. Each step flipped a batch of marks at once, which read as
  jumps. It now turns the same 0.45 rad in equal steps on every frame from 66 to 110, and ends
  where it did. The lower field still holds still.
- **The battery gauge builds in after the start-up (entry 26).** At the handover to the clock,
  the gauge no longer rises from the left. While charging, the slices build from an empty fill,
  as when charging starts; otherwise the solid grows out from the fill's middle, as when
  charging stops. Both start at the rise's 160 ms. Other entries to the clock keep the rise.
- **Settings saves are shorter.** A save used to rewrite a whole 4 KiB flash sector for each
  write, and the display waited through it. It now programs only the bytes it writes. The
  pause after confirming a brightness, timeout, ALWAYS ON or zone is about 23 ms usually, up to
  85 ms. The panel shows the new value before the pause starts, so the confirmation is never
  late.

## Captures and tools

- **The glass's edge is antialiased** in every capture and recording.
- **The board's keys show past the glass's right edge**, where they sit on the device: PWR at
  45° and BOOT at 135°, each an arc of the bezel with its name beyond it. It lights while held.
  Scenes press them.
- **The tour ends on the power key.** A press of PWR rests the screen, another wakes it, and a
  hold opens the power-off confirmation, whose slide powers the device off.
- **The cancelled power-off slide is dragged back** to the left before it is let go, so it
  reads as a cancel.
- **Recordings stay 4:4:4 H.264 at the panel's size.** 4:2:0 smeared the thin coloured lines,
  and doubling the size to keep a colour sample per pixel broke players' scaling. They play in
  players built on ffmpeg or VLC, and in Chromium and Firefox on Linux.
- **`context/screen-captures/screen-atlas.png`** lays one frame of every state out on one
  board, like your screen family board: five sections, 61 tiles at the panel's 466 px, each
  with its name and the PNG it comes from. Its header names the revision that drew it. The
  tiles' order is fixed, so two revisions' atlases compare tile for tile. The `render` example
  writes it with the other stills.
- **A web simulator, `tools/ui-web/`**, runs the firmware's own drawing code in a browser.
  `tools/ui-web/build.sh` writes the page to `tools/ui-web/dist/`, and `dist/index.html` opens
  from disk. The archive this came in carries it built, in `simulator/`.
  - Touch and drag the round screen as on the device.
  - PWR and BOOT are pressable arcs on the bezel, filling as a hold nears its long press.
  - Labelled controls set every reading: heading, pitch, roll, calibration, interference, held
    upright, motion sensors answering, the clock's source, the zone, GNSS fix, the supply, and
    the battery's level.
  - A hint under the screen says what the device is doing and what to try next, and a strip
    under it names the screen, the rest state, the clock's source and the supply.
  - The DISPLAY group shows pixel shift's place and offset and when it last moved, and holds the
    picture at any of the nine places. It also outlines what each frame sends the panel, as the
    board's damage-debug build does.
  - Once powered off, holding PWR for 512 ms powers it on through the start-up, as the board's
    power controller does. The desktop simulator does the same.
  - The readings are synthetic, as in every capture.
  - It is not published. Two of its fonts have licences that do not allow a public page: PP
    Fraktion's free licence and the KH Interference trial.

## Captures

Charging: `clock-charging.mp4`. Start-up, with the gauge building in while charging:
`startup.gif` and `startup.mp4`. Not charging: `ui-sim --record startup-unplugged`. The
identity's turn: `startup.mp4`, frames 66 to 110 of the identity. Pixel shift:
`ui-sim --record pixel-shift`. The keys and the power key: `power-off.mp4`,
`power-off-dim-cancel.mp4`, `rest-always-on.mp4`, `rest-off.mp4` and the tour
(`ui-sim --record tour`). Every state: `screen-atlas.png`.

## For the design

1. Review entries 24 to 26. The design has seen none of the three.
2. Review pixel shift as built against your sequence, in particular the always-on face's
   cadence and the start-up holding at (0, 0). The simulator's DISPLAY group holds any place.
3. Take `screen-atlas.png` as the baseline that later updates are compared against.
