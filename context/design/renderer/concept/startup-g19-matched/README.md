# OCTOWHERE matched scatter study

Four-second identity render at 466 × 466, 30 fps. The G18 title, gray marks, lime marks, BOOT unlock and flicker are retained. Its concept plume is replaced by a two-field approximation of the supplied `ui::scatter` primitive.

| Field | Centre | Radius | Facing | Density | Motion |
|---|---:|---:|---:|---:|---|
| Upper-right | (270, 145) | 150 px | −0.65 rad initially | 0.65 | +0.09 rad on frames 66, 77, 88, 99, 110 |
| Lower-left | (190, 334) | 125 px | 2.55 rad | 0.40 | Static |

Both fields share an 8 px grid with origin (12, −2), a fixed seed per field, and a dim purple (24, 7, 62). Marks below row 317 move down 2 px. A mark is omitted if any part would touch rows 197–317. Grid locations are clipped to the round display. Each selected point is a fixed 60% hollow 6 × 6 rectangle with 2 × 2 knockout, or 40% solid 4 × 4 rectangle inset 1 px. The sample uses the radial and facing laws supplied for the firmware: radial 0.45 at 40 px to 1 at 220 px; turn = 0.45 + 0.55 cos(angle difference); compare hash to radial × turn × density.

The settled render has approximately 195–207 unique marks; the upper field changes by 3–10 points at a scheduled step and is still otherwise. There are no scatter changes while the gray marks animate. The title and microtext remain clear of the scatter.

This is a design match, not a pixel-identical firmware capture. The point hash, exact threshold quantization, edge clipping, seed and hardware colour value must be checked against `ui::scatter`; the Python mixer is a deterministic surrogate. Implement with rectangle operations through the existing primitive, retaining its incremental damage handling. Do not redraw the static lower field after initial presentation. If a later overlay overlaps an active mark, include its affected area in damage.

Run `python3 matched_scatter.py` in this directory to reproduce the render using the included `identity-G18-plume.gif` for the accepted title and marks timing. The GIF and three PNGs are provided for review. G18's dot plume is an input to be removed, not the selected background; see `../startup-g18-out/README.md` as historical exploration. The script saves the four-second GIF and stills here.
