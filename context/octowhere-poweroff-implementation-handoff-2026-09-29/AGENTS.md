# OCTOWHERE power-off confirmation — implementation handoff

This is an **incremental handoff** for the agent that already built the 26 September project backup, the charging/fault update and the 29 September identity/clock update. The owner reviewed and accepted the power-off confirmation design shown in `references/power-off-state-board.png`. Implement [IMPLEMENTATION-DELTA.md](IMPLEMENTATION-DELTA.md) over the current firmware power-key placeholder. Use [VALIDATION.md](VALIDATION.md) to compare captures. No other screen is being redesigned by this archive.

## Read in order

1. [Implementation delta](IMPLEMENTATION-DELTA.md): geometry, colors, entry, gesture, cancellation and shutdown fade.
2. [Validation](VALIDATION.md): exact native frames and state checks.
3. The existing firmware's power-controller flow and previous design handoffs for all unaffected behavior.

`references/` contains the selected 466 × 466 rest, drag, confirmed and fade stills, plus boards for context and the CLEAR SETTINGS family relationship. `source/render_study.py` is a deterministic Pillow concept renderer, with a clock still fixture for its entry board. To reproduce, extract this folder **beside** the original `octowhere-design-project` backup and run `python3 source/render_study.py`; it writes into `references/`. The current firmware's wider settings scatter, source fonts, colors and draw primitives remain the implementation authority. The Python scatter is only a visual surrogate.

The source reference videos are intentionally omitted; **the user will provide them if needed**. This archive includes no firmware repository and no source video. Fixture clock time, battery level, icon and scatter are not live telemetry. Do not blit a PNG into the embedded UI.

Verify the extracted archive with `sha256sum -c MANIFEST.sha256` from this directory. The manifest covers every packaged file except itself.
