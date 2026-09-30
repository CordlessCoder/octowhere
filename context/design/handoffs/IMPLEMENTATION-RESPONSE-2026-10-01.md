# Implementation response: the 30 September typography and the 1 October start-up and AOD

For the design agent. It answers `IMPLEMENTATION-HANDOFF-2026-09-30/` and
`octowhere-startup-aod-handoff-2026-10-01/`. Both are built and on the device. The owner
reviewed every screen in the simulator as it was finished. `DECISIONS.md` entries 18 to 21
record the decisions, and `SCREEN-DESIGN-BRIEF.md` has the details as built. Native captures
are in `context/screen-captures/` (list at the end).

## 30 September: typography, compass field and halftone

**Clock.** KH Bold 136 hours and minutes, KH Bold 40 seconds, KH Bold 18 label and Fraktion Sans
Light 16 UTC and battery lines, all within a pixel of `clock-band-selected-*.png` in every
state, including `BAT 100% CHG` and the low and unknown batteries. The withheld `--` stays Mono
Bold on the digits' columns. The handoff puts the band's ink at x 262; your renders put it at
263 to 264, and the build matches the renders.

**Compass.**
- The readout and `---` are KH Bold 86, and the captions KH Bold 18.
- The suffix stays Mono Bold 40. It is raised to the digits' top and centred on one column, so
  `°` and `%` share a place and finishing a calibration moves nothing.
- `TURN ALL WAYS` is Mono Regular 20 rather than 19, as in `compass-kh-d-selected.png`.
- Against that reference, the text lands within a pixel.

**Compass field.**
- The settled fields are your fixtures (heading blocks seed 4, calibration triangles seed 8,
  interference blocks seed 5).
- They are recorded from `compass_noise.py`'s own generator by `tools/compass-texture.py`,
  which checks the recording repaints Pillow's output exactly.
- The firmware applies the palette, the highlight rows, 0.62 / 0.47 / 0.26, the ×0.27 core and
  the r 229 aperture.
- After RGB565, fewer than 1,000 of about 170,000 field pixels differ from the source, all
  beside the foreground.

**Settings.**
- Row names are KH Bold 20, exactly on column B.
- The inner screens follow column D's mapping: KH at the existing sizes for the selected
  values, Sans Light 18 for the offset's lower neighbour, and Mono everywhere else.

**Halftone.**
- The firmware's shared radial law had the lobes sparse (8 %) and the quiet arcs dense (20 %).
- The panel now uses the S1 prototype's law: 43 % in the two lobes, 9 % elsewhere, the same
  exclusions, and an 8 px grid inside r 219.
- Marks take `#180A36`, `#250C54` or `#371374` by the density where they lie, spread a little
  per mark. Quiet marks mix the darker two, lobe marks the brighter two.

### Where the build departs from the references

1. **`compass-kh-d-selected.png` differs from `compass_noise.py`.** Some stripe rows are one
   pixel off, and a few plates have different sizes, so it came from another copy of the
   generator. The build follows the source, as §3 names it.
2. **HOLD LEVEL's `---` is centred in the slab,** as §2's text says. The reference draws it
   12 px above centre.
3. **The field keeps 2 px off rectangles around the still foreground,** not off its ink. Inside
   the ×0.27 core this does not show. The turning dial is drawn over the field with no halo.
4. **HOLD LEVEL keeps its previous stepped-tile field,** per "retain the approved HOLD LEVEL
   appearance".
5. **The entry steps use the palette.** The heading's is at full strength, as in your timeline;
   calibration's and interference's are scaled by their settled ratio.
6. **The halftone's solid marks stay 4 × 4 inset,** shared with the clock and identity. The
   prototype's are 6 × 6.
7. **The halftone still breathes (owner, entry 4a).** §6's "static at rest" did not override
   the owner's earlier decision, and the owner confirmed it.

## Owner changes beyond the handoffs

- **Long zone lines scroll (entry 19).** In the picker, a name or identifier too long for the
  slab holds 1.2 s, runs to its end at 40 px/s, holds and runs back. It is clipped to the slab
  less its 13 px padding. The owner chose this over stepping the size down.
  `BAHIA BANDERAS`, `PORT-AU-PRINCE` and `DUMONTDURVILLE` overhang by about 36 px.
- **The clock and identity scatters take the halftone's tones (entry 20).** Dense marks are
  bright and sparse marks dark, with the brightest from each scatter's own dense point (the
  clock's densest tenth at chance 0.8, the identity's at 0.71). The identity now reads brighter
  than its old 27 % `PURPLE`, and the clock's scatter slightly dimmer, since its old single
  colour is now the brightest of the three.
- **The fault ticker is centred on the ink of A to Z and `_`** in the 234–282 strip, whatever
  its line holds (baseline 268, one pixel lower than before). Its capitals sat high because the
  line was placed by its own ink, underscore included. The giant name behind it is unchanged,
  still centred on its own ink at row 258.

## 1 October: start-up and AOD

- **Self-test.** Names are KH Bold 18, their visible ink exactly at (left + 82, row + 12).
  Statuses and everything else are unchanged.
- **Identity.**
  - Both subtitle lines are KH Regular 18 from x 261, at tops 309 and 325.
  - The barcode is 74 px and the square starts at x 115, matching `identity-subtitle.png` column
    for column apart from antialiasing on the thinnest bars.
  - After a failed boot the second line keeps `SELF TEST n/6 OK`, as the owner chose, since
    `n/6 FAIL` is 194 px and does not fit the 172 px copy width.
  - BOOT, title, logo, marks, dot period and timings are unchanged.
- **AOD.** It matches `aod-*.png` in every state.
  - The digits are KH Bold on the active clock's columns. The unknown dashes are Mono Bold,
    following the active clock's reference, one column left of the AOD reference.
  - `BAT` is KH Bold 14 with its value in Sans Light 14 a KH space on, the group ending at x 365
    as your renders place it.
  - `UTC / NO ZONE`, `STOPPED` and `CLOCK` are KH Bold. The date stays Mono Regular 16.
  - The bridge blocks, rail, marker and battery cells are unchanged. The references' corner
    blocks differ in shape from the firmware's; the firmware keeps its own, per "retain".
- **Fault.** Unchanged. Compared with `fault-single-A.mp4`, the dropout frames, surviving
  fragments, lower-edge knockouts, slide and black final frame match.
- **Pixel shift is not built yet.** It was deferred while a perimeter ring would have moved
  with it. The ring is gone, and the owner will take up pixel shift separately, using your
  sequence.

## Measured on the device

- **A one-degree heading turn** redraws in 14.0 ms median (p90 16.7) over the new field, against
  10.8 ms (p90 14.3) over the old one. Interference takes 13.0 ms. This is recorded, not yet
  optimised (`bench/compass-field`).
- **Not yet measured:** the AOD's panel luminance and power with the heavier digits, and the
  identity's frame time with the toned scatter.

## Captures

Clock: `clock-*.png`, `clock-charging.mp4`. Compass: `compass-*.png`, `compass-states.gif`.
Settings: `panel-*.png`, `settings-*.png`, `picker-*.png`, `replay-chooser*.png`,
`settings.gif`. Start-up: `startup-selftest*.png`, `startup-identity.png`, `startup-fault*.png`,
`startup.gif`/`.mp4`, `startup-failed.gif`/`.mp4`. AOD: `always-on-*.png`,
`rest-always-on.mp4`. The zone scroll is `ui-sim --record zone-scroll`.

## For the design

1. Review the deviations above, in particular items 1 and 2.
2. Review the toned scatter on the clock face and the identity, which no design round has
   drawn.
3. Pixel shift is next on the owner's list, if you have anything to add to the sequence
   beyond the always-on face.
