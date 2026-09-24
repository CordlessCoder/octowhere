# Compass design answers

1. Keep the `N` at radius 172. The N–icon–caption stack at 0° reads intentionally. Check the actual font ink on the panel; the elements should remain visibly separate.

2. Keep the cardinal letters in Marathon Shapiro at 40 px.

3. Keep `NO DATA` in Marathon Shapiro at 28 px, centred by its ink within the slab.

4. Use PP Fraktion Mono Bold at 86 px for `---`. Centre the ink of all three dashes as one group in the slab. Do not add a degree symbol.

5. Confirm the stated weights: Bold for the numeric readout, suffix, captions, direction abbreviation and `MAG INTERFERENCE`; Regular for `TURN ALL WAYS`, `TOP EDGE UP`, tilt and hint.

6. Keep `MAGNETIC` and `COMPASS` captions gray and `CALIBRATION` orange. The red slab, ring and icon already carry the NO DATA fault colour.

7. Keep the icon at y 87, caption baseline at y 138 and direction abbreviation at 36 px. Centre the smaller `MAG INTERFERENCE` ink optically in the space between the slab and tilt; its baseline may differ from the direction abbreviation's.

8. Change heading/interference colours and content in one frame. On loss of `live` or heading, remove the dial marks immediately and show NO DATA or TOP EDGE UP. Do not fade stale bearings away.

9. If a heading returns from TOP EDGE UP within 750 ms, restore the indices and letters immediately. After a longer absence, use the 170 ms index fade followed 70 ms later by the 170 ms letter fade. The first heading after calibration always uses the reveal.

10. Use the compass page's actual offset for swipe opacity, including its settling movement after finger release. This keeps the fade continuous with the page position and clipping.

11. Six to eight frames in a 110 ms fade are acceptable if the transition looks clean on the physical panel. Do not lengthen it solely to add frames; adjust if the actual fade looks stepped or sluggish.

12. A second cover must work after the hand is lifted, without another screen touch. One continuous cover may trigger only once. Use release/idle reporting if available; otherwise test a new-event re-arm policy that distinguishes a fresh cover from a held-cover repeat. Do not change the hint to instruct an extra tap.
