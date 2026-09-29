# OCTOWHERE clock — reversed 15 px absent battery hatch

This revises the V3 `BAT --` hatch in the selected unoutlined horizontal battery well. The diagonal is flipped: each band moves 24 px left between the top and bottom of the 24 px inner field, for a 45° slope. The gray `#888E98` bands are 15 px wide at a horizontal cross-section with 20 px black gaps, for a 35 px repeat. They remain static and are clipped to x265–437, y284–308 inside the black well x262–440, y281–311. The phase is selected to avoid tiny isolated hatch fragments at either end.

`battery-hatch-direction-width-comparison.png` compares V3 and V4 on GNSS, NO ZONE, and NO DATA clocks. `battery-hatch-15px-detail.png` enlarges both hatches 4×. `battery-hatch-15px-all-states.png` shows the new treatment on all six clock states, and `battery-hatch-15px-neighbor-states.png` shows it beside known low, normal, and charging levels. `clock-absent-hatch-15px.png` is a native 466 × 466 still.

The `BAT --` text still means absent or unavailable battery reading. A known 0% remains an empty black well. Charging barcode and solid fill are unchanged from V2. This is a design render, not a firmware change.

Run `python3 render_study.py` with Pillow installed and sibling `octowhere-design-project/renderer` and `octowhere-clock-battery-fill-v2` directories available. The source videos are omitted; the user can provide them.
