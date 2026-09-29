# OCTOWHERE implementation handoff — 29 September 2026

This is an **incremental design handoff** for the agent that already implemented the 26 September complete backup and received the 27 September fault/charging update. Apply [IMPLEMENTATION-DELTA.md](IMPLEMENTATION-DELTA.md) to the current firmware. Use [VALIDATION.md](VALIDATION.md) for exact native renders and animation timing. This archive is not a firmware checkout and does not replace the earlier handoffs for untouched screens.

## Authority and scope

The owner settled the V11 identity composition with the naturally tall Maratype title, full-width subtitle, octagonal-hole pin logo, balanced plus marks, square/dot microtext and centered horizon HOLD LEVEL icon. The owner also selected the wordmark-free K1 clock with the wide lower battery well, no colored outline, title-barcode-proportioned charging slices and the **reversed 45° gray 15 px hatch** for `BAT --`. The previous clock wordmark placements, outlined battery, 13-slice charging version, 20 px hatch and two-dash unknown mark are superseded.

The separate exploration applying Maratype to SELF TEST, SETTINGS, zone values, clock NO DATA and compass NO DATA was **rejected by the owner**. Do not implement those trials. Maratype remains selected for the identity title only. Keep the current functional typography on all the other screens and states.

The current firmware's validated wider clock/settings scatter, compass field fade on exit, and center-opening hour rail are retained. The concept clock studies in this archive use an earlier K1 compositing helper, so compare their **changed clock geometry and battery treatment** against current captures without rolling back those established details. The 27 September accepted fault screen, hold/exit timing, color exception and functional semantics remain in force. See [BASELINE-AND-DECISIONS.md](BASELINE-AND-DECISIONS.md).

## Read in order

1. [Implementation delta](IMPLEMENTATION-DELTA.md): exact identity and clock changes, timings, data rules and boundaries.
2. [Validation map](VALIDATION.md): assets to compare, including native stills and 30 fps motion.
3. [Baseline and decisions](BASELINE-AND-DECISIONS.md): supersession and rejected paths.
4. Earlier implementation archive's `AGENTS.md`, `IMPLEMENTATION-DELTA.md`, and the complete backup's `AGENTS.md` for behavior not amended here.

## Folder map and reproduction

`SELECTED-REVIEW-BOARD.png` shows the four main visual targets together. `studies/octowhere-identity-maratype-study-v11/` is the selected identity source and render set. `studies/octowhere-clock-battery-placement-v2/` holds the selected outline-free solid/charging geometry and barcode proportions. `studies/octowhere-clock-battery-placement-v4/` holds the selected 15 px absent hatch. `studies/octowhere-clock-battery-fill-v2/render_fill.py` supplies the earlier charging rhythm dependency. Some study folders retain old options in comparison images; only the named targets in `VALIDATION.md` are implementation references. All values in these renders are fixtures, not live telemetry.

The PNGs and MP4s are viewable as packaged. To rerender, copy those four study folders **beside** the `octowhere-design-project` folder from the original backup, preserving their names, then run each `render_study.py` with Pillow and ffmpeg installed. The original source videos are **not** in this archive; **the user will provide them if needed**. The included MP4s are generated design review animations, not source reference footage. Python output is a geometry/timing reference; draw native UI primitives in firmware instead of blitting these frames.

Verify extracted files with `sha256sum -c MANIFEST.sha256` from this directory. `MANIFEST.sha256` lists every packaged file except itself.
