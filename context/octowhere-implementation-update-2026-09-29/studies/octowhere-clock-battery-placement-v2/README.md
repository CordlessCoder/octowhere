# OCTOWHERE clock battery placement V2 — outline and barcode study

This is a follow-up to `octowhere-clock-battery-placement-v1`, using its option 5 (wide lower battery bar, seconds high). The clock wordmark remains absent; K1's status icon, hours, minutes, labels, scatter, rail, date, and zone placement remain as shown. These are design renders, not firmware captures or implementation changes.

## Proposed battery treatment

- The gauge is a black well at x262–440, y281–311 on the 466 × 466 clock. The 1px colored outline is removed entirely. An inset x265–437, y284–308 carries the level fill. The black well makes a lime level distinguishable from the lime clock band.
- A known, idle level is a continuous solid fill from left to right. Charging replaces that fill with upright, full-height slices occupying the same measured length, so its endpoint continues to indicate the level. `BAT 87%` or the appropriate value remains visible above it.
- The identity title's subtitle barcode uses twenty marks whose width is one or two units, with two-unit spaces. Across the twenty marks, it has 27 units of marks and 38 units of internal gaps: about 41.5% illuminated width. At the 87% clock example, the new gauge has twenty marks and nineteen spaces, 62 illuminated pixels and 88 gap pixels: about 41.3% illuminated width. This matches the title's mark-to-gap proportion while scaling to the longer battery slot.
- The mark count follows measured length: three marks at 12%, twenty at 87%, and twenty-three at 100%. Mark widths retain the title barcode's narrow/wide data pattern. The charging beat perturbs the otherwise even gaps by a few pixels over the established 72-frame, 30 fps (2.4 s) cycle. The level endpoint does not move with the beat.
- Low level remains orange; the independent battery on the red NO DATA face remains white and quiet. Unknown level is two gray dashes with `BAT --`, and zero level leaves the well empty. These cases have no invented level fill.

The previous solid-to-slices reveal and interruption transition were designed for the older vertical gauge. A horizontal version is still to be specified if this placement and density are adopted; the included loop only shows the charging hold state.

## Review renders

| File | Use |
| --- | --- |
| `battery-outline-density-comparison.png` | Old outline and 13-slice charging beside the new unoutlined solid, unknown, and title-derived 20-slice treatments. |
| `battery-title-barcode-proportions.png` | Enlarged pixel comparison between the accepted identity subtitle barcode and the proposed clock gauge. |
| `battery-title-barcode-rhythm.png` | Six charging phases across the 2.4 s cycle. |
| `clock-unoutlined-barcode-charge-loop.mp4` | Native 466 × 466, 30 fps, 72-frame charging hold loop. |
| `battery-unoutlined-all-states.png` | Solid and charging examples across the six clock states. |
| `battery-unoutlined-edge-states.png` | 12%, unknown, zero, and full cases. |
| `clock-unoutlined-solid.png`, `clock-unoutlined-charging.png` | Native example stills for direct comparison. |
| `identity-barcode-reference.png` | Accepted identity render used for the barcode proportion study. |

Run `python3 render_study.py` with Pillow and ffmpeg installed, with the sibling `octowhere-design-project/renderer` and `octowhere-clock-battery-fill-v2` directories available. The bundled `placement_base.py` and `clock_study_base.py` reproduce the selected placement and K1 composition; `Maratype.otf` is accompanied by its redistribution note. The original reference videos are not included; the user can provide them.
