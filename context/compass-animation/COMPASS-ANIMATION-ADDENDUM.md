# Compass animation addendum

This is a change to the compass's motion only. It replaces the entry fades, the swipe-out fade
and the heading reveal described in `SCREEN-DESIGN-BRIEF.md` (compass, Behaviour). Layout,
states, precedence and gestures are unchanged.

It uses the same two primitives as the clock face, the icon row build and the cell reveal, and
adds two of its own for the compass, a rule draw-out and a dial sweep. All four are defined
below, so this document stands alone. It works with the compass's icon at any size. The
renders use the proposed 66 px outlined icon.

Later change, by the owner's decision of 2026-09-24: the hint `COVER SCREEN TO RECAL` and the
divider above it are gone, with their entry and exit steps. A cover on the compass now goes to
the clock face, and calibration restarts from the settings panel's COMPASS cell. The tables
below still list the two, as a record of the approved motion.

The brief's compass Behaviour section this replaces no longer exists. `SCREEN-DESIGN-BRIEF.md`
now records the compass's states and every change between them as built.

Later change, round 3 (owner, 2026-09-25): section 1 of
`../octowhere-round3-display-motion-v5/DISPLAY-AND-MOTION-SPEC.md` replaces "Changes while the
page shows" below. The state line and the slab now animate, the icon rebuilds on every change
of state, and interference and top edge up are held against flicker.

Files, all at 466 × 466 with a frame every 20 ms:

| File | Shows |
| --- | --- |
| `images/compass-anim-build.gif`, `images/compass-anim-build-slow4x.gif` | Swipe in, settle, entry, hold, swipe out |
| `images/compass-anim-build-entry-frames.png` | The entry, one frame every 20 ms |
| `images/compass-anim-heading-arrives.gif`, `images/compass-anim-heading-arrives-frames.png` | Calibration completing, with the page settled |
| `images/compass-anim-current.gif` | The current fades, for comparison |

The 20 ms frame is an assumption. The compass draws a full frame in 13–23 ms. The timings are
given in ms, and each frame shows the state at its own time, so a slower frame rate drops
steps rather than stretching the animation.

## Primitives

- Icon row build. The icon's frame and its black inside are drawn whole. The modules in the
  state colour are drawn one row at a time, top down, one row every 30 ms. Every glyph takes
  five steps, 150 ms. For a filled icon, as the compass has today, the tile is drawn whole and
  its black modules build the same way.
- Cell reveal, for Mono text. Over a fixed duration, a string of n characters is revealed left
  to right. At progress k (0 to 1), the character at index i has three possible states:
  - i < k(n + 1) − 1: its glyph shows.
  - The next index: a solid block in the text colour, one advance wide less 1 px each side,
    from the baseline up to cap height.
  - Later indices: nothing.

  A block stands only where a character will be, so no frame shows a wrong character.
- Rule draw-out. The divider grows from its centre to both ends: at progress k it spans
  x 233.5 ± 95.5k.
- Dial sweep. The ticks and letters appear in bearing order, clockwise from N. At progress k,
  every tick and letter at a bearing below 360k is drawn. N shows as soon as k > 0. Bearings
  are the dial's own, so if the heading changes during the sweep, the swept arc turns with the
  dial. Every mark drawn is at its true bearing, and no stale bearing is ever shown.

## What moves with the page

As today, the centre content arrives with the page during a swipe. It must read when a
vertical edge cuts through it. The division changes in two places:

- The icon's frame (or tile) moves into the centre content. Only its modules are an accent.
- The caption, the divider and the hint become accents.

| Centre content | Accents |
| --- | --- |
| Icon frame, state line, slab and readout, tilt line | Ring, icon modules, caption, dial ticks and letters, divider, hint |

The state line is never animated, so `INTERFERENCE` and the rest appear the moment their
condition holds. Digits are never animated: the readout and the tilt change by hard
replacement, as today.

## Entry

Once the page settles, times in ms from settling:

| Accent | Now | Proposed |
| --- | --- | --- |
| Ring | fade 0–110 | fade 0–110, unchanged |
| Icon | fade 95–205 | row build from 95: rows at 95, 125, 155, 185, 215 |
| Caption | with the page | cell reveal 120–240 |
| Ticks | fade 190–300 | dial sweep 190–360, ticks and letters together |
| Letters | fade 285–395 | with the sweep, as it passes each one |
| Divider | with the page | draw-out 240–340 |
| Hint | with the page | cell reveal 280–440 |

The sequence ends at 440 ms, against 395 ms now. In states without a dial (calibrating, top
edge up, no data), the dial rows simply do not apply. In NO DATA, the ring, icon and caption
appear at once with no build or reveal, because a fault stays deterministic.

## Swipe out

As today, the accents follow the page's offset, not time. p runs from 0 at rest to 1 at 35 % of
the width. Reversing a drag restores them continuously.

| Accent | Over p | How |
| --- | --- | --- |
| Hint | 0–0.2 | Reveal run backwards, right to left |
| Divider | 0.1–0.3 | Draw-out run backwards, ends to centre |
| Caption | 0.2–0.5 | Reveal run backwards |
| Dial | 0.2–0.6 | Sweep run backwards, the last bearings first, N last |
| Icon modules | 0.3–0.8 | Rows removed, bottom first, leaving the frame |
| Ring | 0.6–1.0 | Fade to `BLACK`, unchanged |

If a swipe starts before the entry has finished, each accent shows the lesser of its entry
progress and its exit progress, so nothing jumps forward.

## Changes while the page shows

- Heading arrives after NO DATA or calibration. This replaces "ticks fade in over 170 ms,
  letters start 70 ms later". The dial sweeps over 170 ms. The icon rebuilds its new glyph by
  rows over 150 ms, from the same frame, and the caption re-reveals over 120 ms if its text
  changed (`CALIBRATION` to `MAGNETIC`). The readout and slab change at once, as today.
- Heading back from TOP EDGE UP within 750 ms shows the dial, icon and caption at once, as
  today. Tilting through vertical does not replay anything.
- Between heading and interference, the icon swaps in one frame and nothing replays. The rule
  is that the icon rebuilds only when the dial sweeps (or on entry). Interference can switch on
  and off quickly, and a rebuild on every change would be noise.
- Any change into NO DATA is instant, as today. Leaving NO DATA for a heading runs the heading
  arrival above.
- The dial still never animates out on a state change. When the heading goes, its marks go in
  the same frame.

## Firmware work

1. The compass's accent timeline, generalised from colour fades to stepped builds. It needs
   one timeline driven by time after settling, and one driven by the swipe offset, with the
   lesser of the two taken per accent.
2. Drawing the dial by bearing range, so a sweep step draws only the ticks and letters it adds.
   Each step adds about five ticks and at most one letter. That is likely cheaper than today's
   fade, which redraws the whole dial on every step. Neither has been measured.
3. Nothing else. The steps are rectangles, the existing tick quadrilaterals, rotated letters
   and upright glyphs.

## Verification record

What I checked:

- The frames every 20 ms, for the entry, a swipe out and the heading arrival.
- That a partly swept dial draws each mark at its true bearing, by construction in the
  renderer: marks are filtered by bearing, not moved.

What I did not check:

- Drawing by the firmware's own code, or viewing on the panel. The renders come from a concept
  renderer (Pillow, 4× oversampled), with fixture data: heading 047°, pitch +05, roll −12,
  calibration 54 %.
- Any draw cost, or the 20 ms frame rate.
