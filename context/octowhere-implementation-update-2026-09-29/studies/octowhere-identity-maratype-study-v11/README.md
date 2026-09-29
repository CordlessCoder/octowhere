# OCTOWHERE identity: full-width subtitle (V11)

This revision follows the natural-proportion Maratype V10 title. It widens the
subtitle row to the title's visible width by expanding the barcode element
horizontally. All barcode bits, bar-width distinctions, and spaces remain in
the same order; their horizontal coordinates are uniformly scaled. Symbol
heights, copy, title, upper pin, registration marks, dot pulse, and impact
cadence are carried forward.

## Native 466 × 466 geometry

| Element | Visible ink bounds (right/bottom exclusive) | Treatment |
| --- | --- | --- |
| Title | `(33,177)..(433,289)` | Maratype at 112 px with no x/y stretching, centered at `(233,233)`. |
| Complete subtitle | `(33,309)..(433,338)` | Same visible left and right edges as the title. |
| Barcode | Starts x33; 67 px source advance scaled to 113.625 px | About 1.696× wider, still 25 px high. The extra width resides in the bars and their intervals. |
| Square / digits / GNSS / copy | Shift right with the barcode; heights and source proportions unchanged | Keeps the two-line subtitle band coherent. |

The barcode width includes a 2 px correction for the mono copy's advance being
larger than its visible ink. `layout-before-after.png` compares V10 and V11 at
native size and enlarges the subtitle band. `identity-frame-097.png` is the
settled render. `maratype-motion-checkpoints.png` shows the type/flicker phases.

## Animation and timings

`identity-and-impact.mp4` is 139 frames at 30 fps (120 identity frames plus
19 impact frames). `startup-complete.mp4` is 203 frames at 30 fps, or 6.767 s:
30 existing concept self-test frames, 120 identity frames, 19 impact frames,
16 clock-entry frames, and an 18-frame final hold. The self-test and clock
segments provide context, not current firmware captures. Identity title type-on
begins 430 ms into its phase; the subtitle starts at local frame 21; the
outline/fill flicker starts at frame 42; the upper pin appears from frame 56.
The dot in the lower square pulses every 30 frames (1 second). The full-screen
map pin retains its centered octagonal opening through the impact frames.

## Reproduction

Place this folder next to `octowhere-design-project` and run
`python3 render_study.py` with Pillow and ffmpeg installed. The source code,
font, its supplied licence note, prior V10 frame for comparison, and reference
renders are included. Source videos are not packaged; the user can provide
them separately. This is a design study and does not edit the previous backup
or firmware.
