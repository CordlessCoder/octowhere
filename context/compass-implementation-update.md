# Compass screen: implementation update for the design agent

What the firmware now does, after `compass-design-answers.md` and
`compass-design-changes-since-answers.md`. The owner reviewed the result in the desktop simulator
and approved it. Where this conflicts with an earlier document, this one describes the firmware.

## Screenshots

`compass-sim-states/` holds simulator captures of the concept's five states, with the concept's
readings: heading 047°, pitch +05, roll −12, calibration 054%. The simulator runs the firmware's
own drawing code, so these are the pixels the panel shows. The dark grey outside the circle is
off-panel.

| State | File |
| --- | --- |
| Heading | [`heading-047.png`](compass-sim-states/heading-047.png) |
| Interference | [`interference-047.png`](compass-sim-states/interference-047.png) |
| Calibrating | [`calibrating-054.png`](compass-sim-states/calibrating-054.png) |
| Top edge up | [`top-edge-up.png`](compass-sim-states/top-edge-up.png) |
| No data | [`no-data.png`](compass-sim-states/no-data.png) |

Compare with [`compass-concept-states.png`](compass-concept-states.png).

## Layout

The change notice's four items are in:

1. No direction abbreviation under the readout. The four turning letters stay.
2. `INTERFERENCE`, not `MAG INTERFERENCE`, in its original treatment: PP Fraktion Mono Bold 24 px,
   `ORANGE`.
3. One slab for every state, 196 × 91 at (135, 186), ending 16 px above the tilt line's ink.
   The readout and suffix keep their positions inside it; the `---` placeholder is centred in
   it on both axes.
4. The state line sits above the slab. The icon, caption, state line and slab stack as one
   group with 7 px between their inks. The state line is centred by its ink in an 18 px band,
   the height of `INTERFERENCE`, so `TURN ALL WAYS` and `TOP EDGE UP` get 8–9 px either side.

Ink rows from top to bottom, in panel pixels:

| Element | Rows |
| --- | --- |
| Icon, 33 × 33 at x 217 | 102–134 |
| Caption | 142–153 |
| State line band | 161–178 |
| Slab | 186–276 |
| Tilt | 293–309 |
| Divider | 321 |
| Hint | 328–338 |

Heading and NO DATA have no state line and leave the band empty, so the icon and caption never
move between states. The owner chose this over dropping them onto the slab, because interference
can switch on and off rapidly in a disturbed field.

## Colour

- The status icon is `BLUE` (`#409DE4`) while a heading shows. The slab stays `WHITE`. This is
  the owner's call, replacing the concept's `LIME` icon. In every other state the icon takes
  its slab's colour.
- `LIME` is no longer used on the compass.

## Behaviour

- **Reveal after TOP EDGE UP:** a heading back within 750 ms shows the ticks and letters at once.
  After a longer gap, or after NO DATA or calibration, the 170 ms tick fade and 70 ms-delayed
  letter fade run. The grace does not apply after NO DATA.
- **Cover gesture:** a recording of the touch controller showed that it repeats the cover report
  for as long as the hand stays, up to about 180 ms apart, and does not always report the hand
  lifting. A second cover now counts once 260 ms pass without a cover report, or after a finger
  touches the screen. One continuous cover fires once. The hint is unchanged.
- State changes, fades and swipe behaviour are otherwise as the answers specify.

## Rendering

- Text is smoother at large sizes. The fonts' outlines are now flattened for 24 px per em rather
  than about 2, which showed facets on the 86 px readout.
- Glyphs are no longer cached; every glyph is rasterized on every draw. A full heading frame
  took 18.7 ms on the target before the layout moved. Rotated text is still the costliest
  element.
- Next in the firmware: redrawing only what changed on the compass, so the hint, caption, icon
  and slab stop redrawing while only the dial turns. This will change what each element costs
  from "the whole frame" to its own area. Ask for new figures before relying on the cost table
  in `COMPASS-SCREEN-HANDOFF.md`.

## Nothing is waiting on you

All of the answers are implemented or overridden by the owner. The overrides are listed at the
end of `compass-design-answers.md`.
