# Clock face, round 4: colour, background and microtext

The owner asked for more colour in the clock face's normal state and a more interesting
background, taking the identity and fault screens as the direction for the visual language (high
chromatic contrast, huge text with bright microtext, complex patterns, living motion, blocky
elements) while each screen keeps its own identity. This spec changes the built clock face
(`CLOCK-FACE-SPEC.md`, the wordmark addendum, and the brief's "as built" notes). Everything not
named here stays as built: the hours, minutes and seconds, the icon, the band's rows and label,
the wordmark's shape and place, the exit, the gestures, the always-on face.

The images were drawn by the concept renderer (Pillow, 4× oversampled), not the firmware. Fixture:
13:07:42 Thu 24 Sep 2026, Europe/Dublin, UTC 12:07, battery 87 %.

| File | Shows |
| --- | --- |
| `clock4-states.png`, `clock4-state-*.png` | Every state: GNSS, RTC, MANUAL, STOPPED, NO ZONE, NO DATA, and NO DATA with the battery in RED for comparison |
| `clock4-state-longest.png` | The longest zone name, manual, the widest date: the name takes a third line |
| `clock4-battery.png`, `clock4-charging.gif` | The battery column at 87 %, charging, 12 % and unknown; the charging crawl (close-up, 20 ms frames) |
| `clock4-entry.gif`, `clock4-entry-slow3x.gif`, `clock4-entry-strip.png` | The entry with the chosen curves |

## Decisions

The owner settled these on 25 Sep 2026.

1. The built layout stays. The face takes the identity's language, not its composition.
2. The band is `LIME` in the normal states (LOCAL by GNSS, by RTC, manual). `ORANGE` in STOPPED,
   `WHITE` in NO ZONE and `RED` in NO DATA stay.
3. The wordmark's letters off the band, and the battery column's hatch, take the band's colour.
4. The `PURPLE` scatter is drawn under everything, thinner toward the centre, with a 2 px `BLACK`
   halo round every element on the field. Fills are ground: knockout text stays `BLACK`.
5. Microtext lives in one place: the band's two free rows carry UTC and the battery. The top cap
   carries none.
6. The lower block (date, plate, zone name) becomes two lines of code-style tokens: emphasis by
   `[brackets]`, the zone name as a `// comment`, one colour per line (`LIME`, then `GRAY`).
7. The band's right end is a battery column: `LIME` hatch is the charge, `BLACK` the rest.
   `ORANGE` at 15 % or under, `GRAY` hatch at full height while unknown, and the hatch crawls up
   while charging.
8. The entry is eased: out-cubic for what is read, out-back overshoot for the ring and the battery
   hatch, in-quad for the scatter (it blooms late), in-expo for the wordmark (it holds, then
   snaps).
9. No motion while the face rests, except the charging crawl on USB power. A moving background
   distracts from the time.

## Owner rules this changes

- **`LIME` and `PURPLE`**, which the brief lists as unused, now carry the clock face's normal
  state and its background. They no longer mean OCTOWHERE alone.
- **"The bands carry the colour where a state must be loud."** The band now carries colour in
  every state; `LIME` means normal.
- **"A mode in force is filled `GRAY`."** In the lower block's tokens, a mode in force is in
  `[brackets]` instead of a filled tag. Filled tags stay wherever boxes remain (the panel).
- **"Every screen draws on a known flat colour."** Field elements draw over the scatter, as the
  brief allows as a deliberate choice, with a halo so no edge meets the pattern.

## Layers

Bottom up, every frame the face draws in full:

1. The scatter (below), over the whole circle to radius 228.
2. A 2 px `BLACK` halo round every element that sits on the field: the hours, the icon tile, the
   wordmark's letters off the band, and the lower block's text. Text takes the renderer's
   outlined-text ring, 2 px, in `BLACK`, drawn first. Rectangles and the icon take their box grown
   by 2 px in `BLACK`.
3. Ground: the band (rows 198–317, clipped to radius 232), the icon tile's `BLACK` inside, the
   battery window. Nothing of the scatter shows through a ground.
4. The elements.
5. The ring (radius 230–232), `GRAY`; `RED` in NO DATA.

### The scatter

The identity's scatter (`ui::scatter`) with these parameters:

| Parameter | Value |
| --- | --- |
| Colour | `PURPLE` |
| Circle | radius 228 about the panel centre, measured to each mark's centre |
| Grid | every 8 px, x from 12, y from −2. No band gap: it runs under the band, which covers it |
| Marks | as the identity's: hollow 6 × 6 with a 2 × 2 hole (kind < 0.6), else solid 4 × 4 inset 1 px |
| Density | a point shows when v < radial × turn × 0.8 × k |
| radial | 0.15 + 0.85 × clamp((r − 40) / 180) |
| turn | 0.6 + 0.4 × cos(θ − 0.8), fixed: the dense side does not move |
| k | 1 at rest; the entry's scatter curve while the page enters |

It is static at rest. The seed and generator are the identity's (the build's hash of the point's
index), so the pattern is the same every time.

## Band colours and the wordmark

| State | Band | Ring | Wordmark off the band | Battery hatch |
| --- | --- | --- | --- | --- |
| LOCAL, GNSS / RTC / MANUAL | `LIME` | `GRAY` | `LIME` | `LIME` |
| STOPPED | `ORANGE` | `GRAY` | `ORANGE` | `ORANGE` |
| NO ZONE | `WHITE` | `GRAY` | `WHITE` | `WHITE` |
| NO DATA | `RED` | `RED` | `RED` | `WHITE`: the battery is not faulted |

- The wordmark's letters on the band stay `BLACK` in every state.
- The battery hatch is `ORANGE` at 15 % or under and `GRAY` while unknown, whatever the band.
- In STOPPED a low battery looks like a normal one; the number says it.
- The icon, the hours, the minutes, the seconds and the band label are as built.

## The band's lines

Two lines in the band's free rows, between the label and the seconds. Mono Regular 14 px `BLACK`,
pen x 262.

| Line | Cap top | Text |
| --- | --- | --- |
| UTC | 238 | `UTC 12:07`. `UTC --:--` in STOPPED. None in NO DATA |
| Battery | 256 | `BAT 87%`; `BAT 87% CHG` while charging; `BAT --` while unknown. None in NO DATA |

- `UTC HH:MM` from the RTC, as the clock face's NO ZONE row already has it.
- The battery's percentage, charging flag and presence reach the screens today (brief, data
  grade 1). With no battery and USB present the line reads `USB`. This is a proposal.
- The widest, `BAT 100% CHG`, ends at x 363, clear of the wordmark at 366.

## The battery column

| Part | Where | Treatment |
| --- | --- | --- |
| Window | x 394–440, rows 206–311 | `BLACK`, a ground |
| Edge | the window inset 1 px: x 395–439, rows 207–310 | 1 px line in the hatch's colour |
| Hatch | the window inset 3 px: x 397–437, rows 209–308 (99 rows) | 45° stripes rising to the right, 4 px on an 8 px pitch, from the bottom up to round(99 × percent / 100) rows |

- **Colour:** the band's; `ORANGE` at 15 % or under; `WHITE` in NO DATA.
- **Unknown** (until the power controller first answers): `GRAY`, the hatch at full height.
- **Charging:** the stripes move up 1 px a frame, every frame, for as long as the charging flag
  is set. It redraws the hatch area only (41 × 99 px). It is the only motion at rest, and only on
  USB power.
- The window's corner (440, 311) is at radius 221.

## The lower block

The built date row, plate and zone name are replaced by token lines. Each line is centred as a
group on x 233 by its advances. Runs of text sit on one baseline.

| Line | Cap top | Size | Colour | Normal state |
| --- | --- | --- | --- | --- |
| 1 | 340 | 19 px | `LIME` | `THU` Bold, then `24 SEP 2026` Regular, then two spaces and `[AUTO]`: the brackets Regular, `AUTO` Bold |
| 2 | 368 | 14 px | `GRAY` | `IST +01:00 // EUROPE/DUBLIN`, Regular |
| 3 | 386 | 14 px | `GRAY` | Only when line 2 would pass 320 px: line 2 keeps the abbreviation and offset, and line 3 takes `// ` and the name |

| State | Line 1 | Line 2 |
| --- | --- | --- |
| LOCAL, GNSS or RTC | `THU 24 SEP 2026  [AUTO]` | `IST +01:00 // EUROPE/DUBLIN` |
| MANUAL | `… [MANUAL]` | as above |
| STOPPED | `WAITING FOR GNSS`, `GRAY` | as above |
| NO ZONE | `NO FIX YET  [AUTO]`, `GRAY` | none (no zone, as built) |
| NO DATA | none | as above (the zone does not come from the clock) |

- The date keeps 15 characters, so line 1 keeps one width for a given mode.
- Worst cases: line 1 `WED 30 SEP 2026  [MANUAL]` is 285 px, x 91–376, inside radius 226 at
  its rows. The longest line 2, `ART -03:00 // AMERICA/ARGENTINA/BUENOS_AIRES`, is 370 px and
  splits; its line 3 is 277 px at rows 386–396 (`clock4-state-longest.png`).
- The token grammar, for any screen that takes it later: `[X]` marks a mode or a selection;
  `// X` is a remark or a name; one colour per line.

## Entry

The built entry (option B) with the new parts, and a curve per part. Times in ms from the page
settling; a frame every 20 ms, as the face's functional motion is timed (round 3, section 0).

| Part | Window | Curve | What it drives |
| --- | --- | --- | --- |
| Scatter | 0–120 | in-quad, u² | k in the density law: it blooms late |
| Ring | 0–110 | out-back | the ring's fade; it passes `GRAY` by up to 10 % (toward `WHITE`) and settles |
| Icon | 60–210 | out-cubic | rows shown: round(5 × curve) |
| Band label, band lines | 100–220 | out-cubic | the cell reveal |
| Battery hatch | 160–240 | out-back | the hatch's height: it rises up to 10 % past its level, clipped to the window, and settles |
| Lower line 1 | 160–280 | out-cubic | the cell reveal (the plate's old slot) |
| Lower lines 2 and 3 | 240–400 | out-cubic | the cell reveal (the zone name's old slot) |
| Wordmark | 300–460 | in-expo, 2^(10u − 10) | the reveal: it holds, then snaps in over the last frames |

- out-cubic: 1 − (1 − u)³. out-back: 1 + 2.70158 (u − 1)³ + 1.70158 (u − 1)². u is the part's
  progress through its window, 0 to 1.
- The durations are the built ones. The curves change when steps land, not how long anything
  takes.
- The battery window's `BLACK` shows from the first frame, as the icon's frame does.
- **Exit:** as built. The band lines leave with the label's step, the battery hatch with the
  plate's step, the lower lines with the plate's and the zone name's steps. The scatter moves
  with the page and does not fade.

## What changes, and how often

| What | When | Region | Redraws |
| --- | --- | --- | --- |
| Seconds | every second | as built | on the band's ground: no scatter under it |
| Minutes, UTC line | every minute | the band's rows it touches | on the band's ground |
| Battery line and hatch | when the percentage, charging or presence changes | the line; the hatch area | on ground |
| Charging crawl | every frame while charging | 41 × 99 px | on ground |
| Hours | every hour | the hours' ink box grown by 2 px | the scatter under it, the halo, the digits |
| Lower block | at midnight, or on a date or zone change | each line's box grown by 2 px | the scatter under it, the halo, the text |
| Band colour | on a state change | the band's rows | as built |
| Everything | the entry, a swipe, a state change that swaps the lower block | full | full |

Everything on the band draws on known ground, so the per-second and per-minute costs stay close
to the built ones. The hours and the lower block now redraw the scatter under themselves;
`ui::scatter` draws any region on its own, since the pattern is a function of the grid point.

## Firmware work

1. `ui::scatter` on the clock page, with the parameters above, full circle, no band gap.
2. The halo: the outlined-text ring in `BLACK` under the hours, the lower block and the
   wordmark's letters off the band. The wordmark is quarter-turned text; the brief lists outlines
   for upright text only, so its halo is new work (a ring round the transposed raster).
3. Drawing field elements over a pattern, with the scatter redrawn under a changed region.
4. The band colours per state, the wordmark's off-band colour and the battery hatch colour.
5. The band lines, with the battery's percentage, charging flag and presence.
6. The battery column, and its charging crawl (a per-frame redraw of 41 × 99 px while charging).
7. Token lines: runs of Regular and Bold, in one colour, on one baseline, centred as a group;
   the third line for long names.
8. The entry's curves: one function per curve, applied to each part's progress.

## Open choices

1. Whether the curves (the chosen mix) become the rule for accent builds across the UI, or stay
   on this face.
2. The low-battery threshold, 15 %.
3. The battery hatch in NO DATA: `WHITE` (as specified) or `RED`.
4. `[brackets]` and `// comments` as the UI's microtext grammar beyond this face.
5. The always-on face is unchanged (hollow, `WHITE`). It could take `LIME` band rules.
6. A scatter whose dense side moves once an hour as a 24-hour hand was offered and not chosen; the
   dense side is fixed.
7. `USB` in the battery line when no battery is present.

## Verification record

What I checked:

- Every state and the entry, frame by frame, from the renders.
- The layout against the firmware's own capture `clock-gnss.png`: the built elements match.
- Widths: the band lines' widest ends at x 363, before the wordmark (366). Line 1's widest and
  the split line 3 stay inside radius 226 at their rows. The battery window's far corner is at
  radius 221.

What I did not check:

- Any draw cost on the device: the halo, the scatter under a region, the charging crawl.
- How the scatter and halo read on the panel at the owner's viewing distance, and at low
  brightness.
