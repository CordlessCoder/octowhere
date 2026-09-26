# Display and motion specification

This round answers the three questions in `SCREEN-DESIGN-BRIEF.md` ("This round") and adds
two things the owner asked for: a start-up sequence and an always-on face. Each part changes
approved behaviour and is an explicit proposal against the documents the brief lists.

0. Frame rate: the start-up's non-functional animations run at 30 fps; the rest keeps ms timings.
1. Compass state changes: firmware holds, then motion for every change.
2. Start-up: a self-test, then an identity sequence that hands over to the clock, or a fault
   screen if a part fails.
3. Screen timeout, dimming and the always-on face.
4. Pixel shift against burn-in.
5. Two new panel cells, TIMEOUT and ALWAYS ON, and the timeout screen.

The motion primitives (cell reveal, icon row build, rule draw-out, dial sweep) are those of
the clock face spec. Functional motion is timed in ms, with frames assumed every 20 ms; the
start-up's identity, logo card and fault screen are timed in frames at 30 fps. The images
were drawn by a concept renderer (Pillow, 4× oversampled), not the firmware. The fixture is the
brief's.

| File | Shows |
| --- | --- |
| `compass-trans-frames.png`, `compass-trans.gif`, `compass-trans-slow4x.gif` | Every compass change, a frame every 20 ms |
| `startup-overview.png` | Self-test stages, a failure, the fault screen, the clock after it |
| `startup.gif`, `startup-slow2x.gif` | Self-test, identity, logo card, the clock, rendered at 30 fps |
| `startup-failed.gif` | Self-test with the magnetometer failing, the fault screen, the clock |
| `identity-G2-hatch.png`, `identity-G2-hatch.gif`, `identity-G2-hatch-hero.png` | The identity and the logo card frame by frame, with notes; the GIF; the identity's resting frame |
| `logo-mark.png` | The mark through the card's five stages |
| `marks-hatched.png` | The mark's row form and card stages, beside the octagon variant it was chosen over |
| `identity-K-hero.png` | The fault screen at rest |
| `aod-overview.png` | The always-on face in each state, and pixel shift on the always-on face, the clock and the compass |
| `aod-flow.gif` | Rest, dim, the always-on face over three minutes, a touch, the clock's entry. The waits are compressed, and brightness is simulated by scaling colours |
| `display-settings.png` | The panel with eight cells, and the timeout screen |

## 0. Frame rate

- **The start-up's non-functional animations run at 30 fps**: the identity, the logo card and
  the fault screen. The owner set this target. They are timed in frames: frame n shows at
  n × 33.3 ms. Times in ms beside them are for reading only.
- For these, frames are counted by the clock, not by frames drawn. If a frame is late, the next
  frame drawn shows the step due at its own time, so the animation never runs long. This is a
  proposal.
- **Everything else keeps its ms timings**, as built or as specified here: the compass
  transitions, the self-test grid, the clock face's entry, wake and the panel. The renders of
  these assume a frame every 20 ms. Nothing here asks those screens to run at 30 fps.
- "Frames 0–3" means the step draws on each of frames 0, 1, 2 and 3.
- The 30 fps GIFs alternate 30, 30 and 40 ms delays, because GIF delays come in 10 ms steps.

## 1. Compass state changes

### Firmware holds

Motion only reads as smooth if the states stop flickering. So the conditions gain hysteresis
and hold times. These replace the "decided afresh on every reading" rules in the brief.

| State | Enters when | Leaves when |
| --- | --- | --- |
| INTERFERENCE | The field strength has been more than 35 % off the calibrated strength for 200 ms without a break | The field strength has been within 30 % for 1 s without a break |
| TOP EDGE UP | The top edge is within 11.5° of vertical, at once, because the heading is no longer valid | The top edge is more than 15° from vertical |

- Interference errs toward warning too long rather than too briefly. While it holds, the heading
  continues from the gyroscope as today.
- TOP EDGE UP keeps the built 750 ms rule for the dial on its way back.
- With these holds, two changes of the same kind are at least 200 ms apart. Every transition
  below ends within 180 ms. If a change does arrive while a transition runs, the running one
  completes in that frame and the new one starts from there.

### Primitives

- **Slab wipe.** When the slab changes colour, the new colour covers it from the top in four
  steps, on the frames at 0, 20, 40 and 60 ms after the change: rows 204–226, 204–249, 204–271,
  then 204–294. The readout is redrawn in `BLACK` over both colours on every step. Its value
  changes, if it changes, in the first frame. Digits never animate.
- **Icon rebuild.** The frame recolours in the first frame. The new glyph's modules build one
  row every 30 ms, rows at 0, 30, 60, 90 and 120 ms.
- **State line and caption.** A new text types in by cell reveal. Text that goes untypes, right
  to left. When one state line replaces another, the old one untypes over 0–60 ms and the new
  one types over 60–180 ms. A state line with nothing before it types over 0–120 ms. A caption
  retypes over 0–120 ms. The addendum's rule that the state line is never animated is replaced.

### Changes

| From → to | What happens |
| --- | --- |
| HEADING → INTERFERENCE | Icon rebuild (interference, `ORANGE`). Slab wipe `WHITE` to `ORANGE`. `INTERFERENCE` types in, 0–120 ms. The dial, ring, caption, readout and tilt stay |
| INTERFERENCE → HEADING | Icon rebuild (arrow, `BLUE`). Slab wipe `ORANGE` to `WHITE`. `INTERFERENCE` untypes, 0–60 ms |
| CALIBRATING → TOP EDGE UP | The readout becomes `---` in the first frame. Icon rebuild (top bar, `WHITE`). Slab wipe `ORANGE` to `WHITE`. `CALIBRATION` retypes as `MAGNETIC` (`GRAY`). `TURN ALL WAYS` untypes, then `TOP EDGE UP` types |
| CALIBRATING → HEADING or INTERFERENCE | As built: the dial sweeps over 170 ms, the icon rebuilds, and the caption retypes. Added: the slab wipes to `WHITE` for HEADING (INTERFERENCE stays `ORANGE`), `TURN ALL WAYS` untypes, and `INTERFERENCE` types in if it applies |
| TOP EDGE UP → HEADING or INTERFERENCE | As built for the dial (instant within 750 ms, otherwise sweep and rebuild). Added: `TOP EDGE UP` untypes, and for INTERFERENCE the slab wipes to `ORANGE` and `INTERFERENCE` types in |
| HEADING or INTERFERENCE → TOP EDGE UP | The dial and the readout go in the first frame, so no stale bearing shows. Icon rebuild. The slab wipes to `WHITE` from INTERFERENCE. The state line changes as above |
| NO DATA → any state | The ring turns `GRAY`, the slab takes its new colour, and the readout and tilt appear, all in the first frame, so the fault's `RED` never sits behind a live value. Then icon rebuild, the caption retypes, and the state line types in. The dial sweeps if there is a heading |
| Any state → NO DATA | Everything in one frame, as built |

### What changes, and how often

A transition touches only the slab (196 × 91), the icon (66 × 66) and the state line's band,
one step at a time. The ring changes only on leaving NO DATA, and the dial only when it sweeps.
With the holds, the compass makes at most one transition per 200 ms. None of this has been
measured.

## 2. Start-up

> Identity layout in this section documents Round 3 / G2 and is superseded by the G17 entrance,
> G18 centered title and G19 matched scatter study named in `../PROJECT-MAP.md`. Retain the
> functional self-test/failure behavior and other unaffected timing until explicitly revised.

### When

The sequence starts at the first frame the panel can show after power-on. The panel brightness
steps from 0 to the stored level over the first 200 ms, by the panel's brightness command, at
no draw cost. The self-test lasts exactly as long as boot does. Nothing waits for the animation, and
nothing waits longer than the tests need.

### Self-test

| Element | Position | Treatment |
| --- | --- | --- |
| Ring | radius 230–232 | `GRAY` |
| Title | centred by ink, ink top 92 | `SELF TEST`, Shapiro 16 px, `WHITE` |
| Rules | rows 129, 233 and 337, x 59–407; columns at x 59, 175, 291 and 407, rows 129–337 | 1 px `GRAY` |
| Cells | 116 × 104, three columns and two rows, index order across then down | |
| Index | pen at the cell's left + 9, ink top at its top + 9 | `01`–`06`, Mono Regular 14 px, `GRAY` |
| Icon, 48 × 48 | x at the cell's left + 59, y at its top + 8 | Outlined, 8 px modules, 4 px padding, 2 px frame |
| Name | ink left at the cell's left + 9, ink top at its top + 62 | Mono Bold 14 px, `GRAY` |
| Status | pen at the cell's left + 9, baseline at its top + 90 | Mono Regular 16 px: `--` `GRAY`, `OK` `WHITE`, `FAIL` `RED` |
| Counter | centred by ink, ink top 353 | `n/6`, Mono Regular 14 px, `GRAY` |
| Version | centred by ink, ink top 373 | `0.1.0`, Mono Regular 14 px, `GRAY` |

| Cell | Glyph | Passes when |
| --- | --- | --- |
| 01 POWER | battery `01110 11111 10001 11111 11111` | the power controller answers |
| 02 CLOCK | the clock's RTC glyph `11111 10001 10101 10001 11111` | the real-time clock answers and reads |
| 03 TOUCH | crosshair `00100 00100 11011 00100 00100` | the touch controller answers |
| 04 MOTION | axes `10000 10000 10000 10000 11111` | the IMU answers |
| 05 MAGNET | horseshoe `11011 11011 11011 11111 01110` | the magnetometer answers |
| 06 GNSS | the GNSS glyph `00100 01010 10101 01010 00100` | the receiver answers. This is not a fix |

- `OK` means only that the part answered its driver. It claims nothing more.
- A cell starts with a `GRAY` empty frame and `--`.
- When its part answers, the frame turns `WHITE`, the glyph builds by rows (30 ms a row), the
  status reads `OK`, and the counter steps.
- A part that has not answered by its driver's deadline fails. On one frame the cell turns
  `RED`: a `RED` frame, the no-data glyph and `FAIL`. The firmware sets the deadlines.
- The counter counts decided cells, passed or failed.
- Once every cell is decided and the last glyph is built:
  - all passed: hold 200 ms, then the identity;
  - any failed: hold 300 ms, then the fault screen. There is no identity after a failure.

### Identity

Black field, no ring. It uses the two tokens nothing else uses, `LIME` and `PURPLE`, so they mean
OCTOWHERE and nothing else. Frame 0 is the first frame after the self-test's hold.

**Resting layout** (`identity-G2-hatch-hero.png`)

| Element | Position | Treatment |
| --- | --- | --- |
| Scatter | the whole circle to radius 228, except the band rows | `PURPLE` marks on an 8 px grid; see below |
| Band | rows 198–317 | Black. No rules: the band is where the scatter stops |
| Microtext row | top 211, from x 34 | Five elements, left to right, gaps as listed |
| Word | ink left 34, ink bottom 307 | `OCTOWHERE`, Shapiro 40 px drawn 1.8× tall (325 × 54 px ink), `LIME` |
| Hatch column | x 372–436, rows 206–311 | `LIME` stripes at 45°, rising to the right, 5 px wide on a 10 px pitch |

Microtext row, all `LIME`, top at row 211:

| # | Element | x | Treatment |
| --- | --- | --- | --- |
| 1 | Version barcode | 34–101 | For each character of the version string (`0.1.0`), its four low bits, least first: a 2 px bar for 1, a 1 px bar for 0, 2 px gaps. 15 px tall. Gap 10 |
| 2 | The mark | 111–125 | Its 15 × 15 px row form, 1 px strokes, rows top down: `111000111000111 100000000000001 100000000000001 000111111111000 000100000001000 000100000001000 100100000001001 100100000001001 100100000001001 000100000001000 000100000001000 000111111111000 100000000000001 100000000000001 111000111000111`. Gap 11 |
| 3 | UTC hours and minutes | 137–191 | Pixel digits, 3 × 5 modules of 3 px, 12 px advance, a 6 px space between hours and minutes. From the RTC. Gap 10 |
| 4 | GNSS glyph | 201–216 | `00100 01010 10101 01010 00100`, 3 px modules. Gap 11 |
| 5 | Two text lines | ink left 227, ink tops 209 and 225 | `VERSION 0.1.0` and `SELF TEST 6/6 OK`, Mono Regular 14 px |

Pixel digits:

| 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `111 101 101 101 111` | `010 110 010 010 111` | `111 001 111 100 111` | `111 001 111 001 111` | `101 101 111 001 001` | `111 100 111 001 111` | `111 100 111 101 111` | `111 001 010 010 010` | `111 101 111 101 111` | `111 101 111 001 111` |

The version appears twice, as the barcode and as the first text line. The row's text is real
data only.

**Scatter**

- Grid points every 8 px: x from 12, y from −2. A point's mark is either hollow (6 × 6, with a
  2 × 2 hole in the middle) or solid (4 × 4, inset 1 px). Marks on the rows that would touch
  rows 197–317 are not drawn, and marks below the band are drawn 2 px lower, so 2 px of black
  separate the scatter from the band on both sides.
- Each point draws two numbers from a seeded generator, in raster order, once: v and kind. The
  pattern is the same every boot. Any generator will do; mine is Python's with seed 4.
- A point shows when v < radial × turn × 1.15 × appear, and only within radius 228 of the
  centre (measured to the mark's centre):
  - radial = 0.45 + 0.55 × clamp((r − 40) / 180), with r the distance from the centre
  - turn = 0.45 + 0.55 × cos(θ − 0.8 − 0.055 n), with θ the angle from the centre and n the
    frame. The dense side turns slowly all through
  - appear = 0.5 on frame 0, then 0.5 → 1 over frames 2–8
- kind < 0.6 draws the hollow mark, otherwise the solid one.
- The scatter changes every frame, so every frame is a full redraw.

**Timeline**

| Frame | ms | What |
| --- | --- | --- |
| 0 | 0 | The scatter cuts on at half density |
| 1 | 33 | Dark: one frame off, by brightness, at no draw cost |
| 2–8 | 67–267 | The scatter fills to full density |
| 6–10 | 200–333 | The microtext row types in, one element a frame. Both text lines come together on frame 10 |
| 6–9 | 200–300 | The hatch column draws down from row 206, a quarter a frame |
| 8–17 | 267–567 | The word by cell reveal, one letter a frame. The block for a letter shows a frame before the letter. Whole on frame 17 |
| 20, 22 | 667, 733 | The word is off for one frame each: the flicker |
| 23–57 | 767–1900 | Holds, steady. About 1.2 s |

### Handover to the clock: the logo card

After the hold, a short card with the OCTOWHERE mark, after the owner's reference (a trailer
card by Worship Studio). The band no longer carries over: the clock arrives by its own entry.

| Frame | ms | What |
| --- | --- | --- |
| 58–60 | 1933–2000 | The page (radius 232) fills `LIME`. The mark's tile, with its hatch, is knocked out alone in `BLACK` |
| 61–64 | 2033–2133 | The whole mark: the corners and edge lines join the tile. `BLACK` on `LIME` |
| 65–68 | 2167–2267 | Inverted: `BLACK` field, the mark alone in `LIME` |
| 69 | 2300 | The mark scaled 1.9× about the centre, `LIME` on black, for one frame. The corners reach the panel edge |
| 70 | 2333 | Impact: the scaled mark inverted, `BLACK` on a `LIME` page, for one frame |
| 71– | 2367– | The clock face, cut in. From here its own motion runs in ms as built: the time types in over 0–180 ms and the date over 120–280 ms, while the face's entry runs, ending at 460 ms |

**The mark** (`logo-mark.png`): an icon tile's outline with a hatched centre, and four
corners and four edge lines around it. It is built on the icons' 5 × 5 grid, with the icons'
frame rule for its stroke. On the card: module m = 42 px, stroke 11 px (frame_for(42)), the mark
210 × 210 with its top-left at (128, 128).

| Part | Where, on the card at 1× | |
| --- | --- | --- |
| Tile | the square x 170–296, y 170–296 (modules 1–4) | An 11 px outline |
| Hatch | inside the tile, the square x 186–280, y 186–280 (inset 5 px from the outline) | 45° stripes rising to the right, the same direction as the identity's hatch column: lit where ((x − 233) + (y − 233)) / √2 mod 12 < 5, so 5 px stripes on a 12 px pitch |
| Corners | the four corner modules | L brackets on the outer edge: arms 42 px long, 11 px thick. Top left: x 128–170 × y 128–139, and x 128–139 × y 128–170; the others mirrored |
| Edge lines | the four middle modules of the outer ring | 42 × 11 px, along the outer edge: top x 212–254 × y 128–139; the others rotated |

- At 1.9× every coordinate scales about (233, 233), strokes, hatch pitch and stripes included.
- The mark is rectangles and a stripe test, so the firmware draws it at any scale with no
  bitmap.
- In the identity's microtext row it is element 2, in its 15 × 15 px form above, without the
  hatch: at 1 px the hatch is noise.
- The first stage shows the tile with its hatch alone. That is the reference's partial
  knock-out, done with the mark's own hatch. It replaces vertical stripes over the whole mark,
  which break thin strokes up unevenly.

From the last glyph built to the finished clock face takes 200 ms + 71 frames + 460 ms, about
3.0 s.

A touch during the identity or the card goes straight to the settled clock face. This is a
proposal. Touches during the self-test do nothing.

### Replay

The implementation adds REPLAY START-UP under the panel's DEVICE cell. What it plays:

- The identity from frame 0 and the logo card, then the clock face, exactly as after boot. It
  does not replay the self-test or its brightness step: the panel stays at the set level.
- The microtext row shows live data: UTC now, and the self-test result stored from the last
  boot. If a part failed at that boot, the row says so (`SELF TEST 5/6`), because the row's text
  is real data only.
- It lands on the clock face, like the end of any start-up, not back on DEVICE. Leaving the
  panel for the clock is what cover does, so this is the same grammar.
- A touch skips to the clock face, as at boot. The timeout timer does not run during it; it
  restarts when the clock face settles.
- The DEVICE screen's own design for the entry is the implementation's. I have not seen it,
  so this spec covers only what plays.
- Everything in this list is a proposal except the entry itself, which the implementation
  already has.

### Fault screen

It replaces the old 2 s hold on the self-test grid. It is timed as a fault: it cuts on in one
frame. The failed part is the example here: 05 MAGNET (`identity-K-hero.png`,
`startup-failed.gif`).

| Element | Position | Treatment |
| --- | --- | --- |
| Field | the circle to radius 229, no ring | `RED` |
| Dashed rules | rows 86–87, 150–151, 366–367 and 430–431 | `BLACK` dashes 9 px long, one every 46 px from x 20 |
| Part glyph, four times | centred on (110, 118), (356, 118), (110, 348) and (356, 348) | The failed part's self-test glyph, 9 px modules (45 × 45), `BLACK`. A `BLACK` bar 64 × 8 above and below each: rows centre − 38 to − 30 and centre + 30 to + 38 |
| Band | rows 198–317, clipped | `BLACK` |
| Giant name | ink-centred on row 258, clipped to the band | The part's name, Shapiro 200 px, `RED`. Its ink starts 60 px right of centred and drifts left 1 px a frame |
| Strip | rows 234–281 | `BLACK`, over the giant name |
| Running line | ink-centred on row 258 | `MAGNET FAIL_` repeated without a seam, Shapiro 32 px drawn 1.3× tall, `WHITE`, moving left 4 px a frame |
| Microtext, above | ink left 162; ink tops 164 and 178 | `SELF TEST 5/6 OK` Mono Bold 12 px, `05 MAGNET FAIL` Mono Regular 12 px, `BLACK` |
| Hatch | x 290–318, rows 164–190 | `BLACK` stripes at 45°, 4 px on an 8 px pitch, moving 2 px a frame |
| Microtext, below | ink left 162; ink top 328; barcode and version at top 345 | `NO REPLY BY DEADLINE` Mono Regular 12 px; the version barcode 12 px tall, then `0.1.0` Mono Bold 12 px 8 px after it; `BLACK` |

- `RED` is the fault colour, so the fault screen may use it at full field. `WHITE` carries the
  running line; `BLUE` stays for a live valid reading.
- **Several failures:** the running line lists each, `MAGNET FAIL_GNSS FAIL_`. The glyphs and
  the giant name are the first failure's. The count is of parts that passed.
- **The reason line** says what the self-test knows: the part did not answer by its deadline.
- **Timing:** it holds 120 frames (4 s), set by the owner, then cuts to the clock face, which
  runs its entry as built and shows its own fault states for whatever failed. Over the 4 s the
  giant name drifts 120 px and the running line 480 px. A touch goes to the clock at once. This
  is a proposal.

### Drawing cost

- **The word** is Shapiro stretched 1.8× tall, which the glyph renderer does not do today. It is stored
  as a 1-bit bitmap, 325 × 55 px, about 2.2 KB. The cell reveal clips it at each letter's edges,
  which are stored with it.
- **The fault screen's running lines**, one per part, are 1-bit bitmaps of about 309 × 38 px,
  about 1.5 KB each. The giant names are about 1092 × 152 px, about 21 KB each, or 124 KB for
  all six. The giant name could instead be drawn by the glyph renderer at 200 px, if the heap
  allows it.
- **The scatter** is up to about 1,600 small rectangles a frame, every frame.
- None of this has been measured.

## 3. Timeout, dimming and the always-on face

### Flow

```
in use ──timeout──▶ DIM (5 s) ──▶ ALWAYS-ON face  (ALWAYS ON is on)
   ▲                  │           or DISPLAY OFF    (ALWAYS ON is off)
   │                  │touch            │touch
   └──────────────────┴─────────────────┘   wake: the page runs its entry
```

- **Timer.** It restarts on any touch contact, a cover, and any page or sheet movement. On the
  compass page it also restarts on a heading change of more than 10° since the last restart, so
  the compass stays on while it is in use.
- **Timeout.** The stored setting: 15 s, 30 s, 1 min (the default), 5 min, or never. It is the
  same on every screen.
- **Dim.** At the timeout, the panel's brightness drops to 30 % of the set level (at least 3 %),
  in one step. Nothing redraws. A touch during the dim restores the level at once and does
  nothing else.
- **After 5 s dimmed**, either the always-on face shows, cut in one frame at the always-on
  level, or the panel is switched off by its display-off and sleep commands.
- **Wake.** A touch contact wakes the screen, and that touch does nothing else. A cover does
  nothing while the screen is dimmed, off or showing the always-on face.
- **Where a wake lands.** On the clock or the compass, back on that page, which runs its entry
  (accents build) while the brightness steps back to the set level over 100 ms. From the panel
  or any second-level screen, it lands on the clock face, as cover does, with any edit discarded
  and nothing stored.
- **Never.** No dim, no always-on face, no switching off. Pixel shift still runs.

### The always-on face

The clock's layout, drawn hollow. It always shows the clock, whichever page was left.

| Element | Position | Treatment |
| --- | --- | --- |
| Ring | none | |
| Hours | pens x 62 and 144, baseline 184 | Fraktion Mono Regular 136 px, `WHITE` |
| Band | two rules, rows 198 and 317, clipped to radius 232 | 1 px `WHITE`, the outlined form of the band |
| Minutes | pens x 62 and 144, baseline 305 | Mono Regular 136 px, `WHITE` |
| Date | centred by ink, ink top 334 | `THU 24 SEP`, Mono Regular 16 px, `GRAY` |

| Clock state | The always-on face shows |
| --- | --- |
| LOCAL | The time and date |
| STOPPED | `--` `--`, and `STOPPED` in Mono Regular 16 px `ORANGE`, ink left 262, ink bottom 305 |
| NO ZONE | `--` `--`, and `NO ZONE` in Mono Regular 16 px `GRAY` at the same place |
| NO DATA | The band rules in `RED`, and `NO DATA` in Shapiro 28 px `RED`, ink left 71, centred on row 257.5 |

- There are no seconds, icon, plate or wordmark.
- It redraws in full once a minute, at the minute's change, and on a change of state. Each
  redraw also moves it one step of the pixel shift.
- It is lit at the always-on level, 10 % of full (26 of 255), or the set level if that is
  lower. This is a starting value, to be judged on the panel.
- Lit pixels: 5.7 % of the visible circle (1.5–1.7 % in the other states), against 38 % for the clock face, measured from the renders.

## 4. Pixel shift

- **What moves:** everything inside the ring, on every screen and on the always-on face, by
  an offset (dx, dy). The ring stays put.
- **Clipping:** bands, field rules and the panel's cropped column move with the content, but are
  clipped to the fixed circle of radius 232, so a band's ends always meet the ring. Everything
  else is already inside radius 226, so it stays inside 229, clear of the ring's inner edge at
  230. The compass's ticks, the outermost moving ink, reach 229 at most.
- **Positions:** P0 is (0, 0). Then eight points round a 3 px circle: P1 (3, 0), P2 (2, 2),
  P3 (0, 3), P4 (−2, 2), P5 (−3, 0), P6 (−2, −2), P7 (0, −3), P8 (2, −2). Each move goes one
  step along P0, P1, …, P8, P0, … and so is never more than 3 px.
- **When it moves on the faces, panel and screens:** only at moments that hide it. That means
  a page change settling, the panel opening or closing, a wake, or, failing all of those, a
  minute change once 10 minutes have passed since the last move. The move is a full redraw of
  that frame.
- **When it moves on the always-on face:** at every minute's redraw.
- **Touch** follows the picture: a touch's coordinates are taken minus the offset.
- This is the brief's third option, "shifting only what is inside them". The first two would
  clip the ring, or move it inward, and the owner judged a 2 px inset too far in the first
  round.

## 5. Display settings

### Panel

The grid grows to four columns (s from 0 to 2). The indices follow the new order, and four
column markers show:

| Column | Top | Bottom |
| --- | --- | --- |
| 1 | 01 ZONE | 02 BRIGHTNESS |
| 2 | 03 TIMEOUT | 04 ALWAYS ON |
| 3 | 05 COMPASS | 06 GNSS |
| 4 | 07 BATTERY | 08 DEVICE |

| Cell | Glyph | Icon | Value |
| --- | --- | --- | --- |
| TIMEOUT | hourglass `11111 01110 00100 01110 11111` | `WHITE` | `15 S`, `30 S`, `1 MIN`, `5 MIN` or `NEVER`, `WHITE` |
| ALWAYS ON | open eye `00000 01110 11011 01110 00000` | `WHITE` | an `ON` tag (filled `GRAY`, a mode in force), or `OFF` in `GRAY` |

- A tap on TIMEOUT opens the timeout screen.
- A tap on ALWAYS ON toggles it and stores it. The toggled cell shows before the save starts.
- Both are new stored keys: `timeout` (default 1 min) and `always_on` (default off).
- Clearing settings erases them too. The clear warning's text gains them.

### Timeout screen

The picker's grammar, one step:

| Element | Position | Treatment |
| --- | --- | --- |
| Hint | centred by ink, ink top 48 | `DRAG TO SET TIMEOUT`, Regular 14 px `GRAY` |
| `CANCEL` | box x 72–181, rows 104–142 | As the picker's |
| Icon, 96 × 96 at x 262 | rows 90–185 | Hourglass, `WHITE`, outlined |
| Field | as the picker's | |
| Selected | ink left 62, centred on row 257.5 | Mono Bold 56 px, `WHITE` |
| Neighbours | ink left 62, ink centred on rows 174 and 341 | Mono Regular 23 px, `GRAY`. Empty past either end |
| Position | centred by ink, ink top 381 | `n/5`, Regular 14 px `GRAY` |
| Hint 2 | centred by ink, ink top 401 | `TAP TO KEEP`, Regular 14 px `GRAY` |

Stepping, keeping, `CANCEL` and cover all behave as in the picker.

## Firmware work

0. A 30 fps frame clock for the identity, the logo card and the fault screen (section 0).
1. The interference hold and hysteresis, and the TOP EDGE UP hysteresis.
2. The compass transitions: the slab wipe in steps, the state line and caption reveals, and
   their redraw regions.
3. Start-up:
   - the panel up as early in boot as it can be
   - per-part answers and deadlines passed to the screens
   - the boot brightness step
   - the self-test, identity, logo card and fault screens
   - the identity's scatter generator, the word bitmap with its letter edges, and the fault
     screen's bitmaps per part
   - the logo card: the mark as rectangles and its hatch, the scaled frame and the impact frame
   - replaying the identity from DEVICE (section 2, Replay)
   - a touch to skip the identity or the fault screen
4. The timeout timer and its restart rules, including the compass heading rule.
5. Dimming and waking by brightness command. Display off and sleep, and waking from them.
   Whether the touch controller reports while the panel is off is untested. If it doesn't,
   "off" means the panel on at brightness 0 with a black frame.
6. Wake routing: the page's entry, or the clock face, with edits discarded.
7. The always-on face: its minute redraw, its brightness, and its state variants.
8. Pixel shift: a layout offset on every screen, the fixed ring, bands clipped to the fixed
   circle, touch offset, and the step schedule.
9. Two stored keys, two panel cells, the four-column grid and the timeout screen.
10. A proposal: the power button as a second wake source, once its pin is verified.

## Decisions

The owner settled these on 25 Sep 2026.

1. The interference and TOP EDGE UP holds and hysteresis, as above.
2. The compass motion as proposed in discussion: slab wipe, icon rebuild, state line reveal,
   and cuts where a bearing would go stale.
3. Start-up is a self-test, then an identity sequence that plays only after the self-test
   succeeds.
4. Pixel shift, timeout and the always-on face are designed together in this round.
5. The identity is direction G2 of the prototypes, layout B: a black field with `PURPLE`
   halftone scatter running up to the band, no band rules and no checker squares, F's microtext
   row inside the band with the version in it, a larger word left of centre, and a hatch column
   on the right.
6. The identity holds on its end state for about 1.2 s.
7. A failed self-test shows the fault screen (prototype K) for 4 s (120 frames), then the clock.
8. The start-up's non-functional animations (identity, logo card, fault screen) target 30 fps
   and are timed in frames. Functional motion keeps its ms timings.
9. The identity hands over through a logo card (the hatched tile, the whole mark, inverted,
   one frame scaled up hard, one inverted impact frame), not through the band.
10. The mark is L1 with a hatched centre: an icon tile's outline, hatched, with four corners
    and four edge lines. It replaces the octagon glyph in the identity's microtext row.

## Open choices

1. The always-on level (10 %) and the dim level (30 % of set) are starting values, to be judged
   on the panel.
2. The default timeout, 1 min. A navigation device read outdoors may want longer.
3. Skipping the identity, or the fault screen, with a touch.
4. The self-test's deadlines per part. `OK` must mean only that the part answered.
5. Motion as a wake source is left out: wake-on-motion would fire constantly while walking. A
   raise gesture would be firmware work on the accelerometer.
6. Whether the always-on face should show the compass when the compass was the page left. This
   design always shows the clock.
7. The start-up now takes about 3.0 s from the last check passing to the finished clock face,
   against 1.66 s before, because of the longer hold and the card.

## Verification record

What I checked:

- Every render in this round, looked at frame by frame for the transitions, start-up, the fault
  path and the timeout flow: the start-up at 30 fps, the rest at 20 ms frames.
- The identity's positions and sizes above are measured from the renderer's own layout.
- Pixel shift: the clock and compass drawn at offsets, with the ring fixed and the band ends
  meeting it. The compass's ticks stay clear of the ring at 3 px.
- The lit-pixel counts above, measured from the renders.

What I did not check:

- Drawing by the firmware, or the panel. The dimmed frames are simulated by scaling colours.
- Any draw cost, whether 30 fps holds with the scatter redrawn every frame, the bitmap sizes
  (estimated, not built), or how the holds feel in use.
