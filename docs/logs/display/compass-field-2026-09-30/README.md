# Compass field draw, 30 September 2026

`bench/compass-field`, feature `compass-field-bench`: the compass held on its settled fields,
10 s each of heading, calibration and interference, the heading turning a degree every 100 ms.
Each line of `[BENCH] draw` is one draw's time and the pixels it repainted. Draws within a
second of a change of state are left out of the summary.

    cargo build --release --offline --features compass-field-bench
    uv run tools/compass-field-summary.py <log>

`after.txt` is master at 04dda8d, with the design's fixtures; `before.txt` is 93af9ff, with the
earlier generated field, built with the same bench.

| Draw of a one-degree turn | Before, median | p90 | After, median | p90 |
| --- | ---: | ---: | ---: | ---: |
| Heading | 10.8 ms | 14.3 ms | 14.0 ms | 16.7 ms |
| Interference | 10.8 ms | 14.2 ms | 13.0 ms | 15.6 ms |

Calibration has no dial, so its settled field draws nothing after the entry. A full compass draw
during the entry's first steps took 15.7 to 19.2 ms after.
