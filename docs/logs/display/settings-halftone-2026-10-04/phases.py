"""Summarises drive.sh's runs: each phase's frame times, step and draw together, from the
`timing-log` lines between the drive's marks. The phases are windows of seconds from the
drive's start, set from these runs' timelines.

    uv run docs/logs/display/settings-halftone-2026-10-04/phases.py <dir> before after
"""

import lzma
import re
import sys
from pathlib import Path
from statistics import median, quantiles

PHASES = {
    "opening, frames over 20 ms": lambda t, f: (0.5 <= t < 1.5 or 15.5 <= t < 16.5) and f > 20,
    "page drags, frames over 20 ms": lambda t, f: 3.5 <= t < 13.5 and f > 20,
    "settled, frames of 2 to 20 ms": lambda t, f: (2 <= t < 3.5 or 16.5 <= t < 18.5) and 2 < f <= 20,
}


def frames(directory, name):
    with lzma.open(directory / f"{name}-1a38.log.xz", "rt", errors="replace") as log:
        lines = [line for line in log if "timing draw" in line]
    start, end = (int(line.split()[-1]) for line in open(directory / f"{name}-1a38.marks"))
    out = []
    for line in lines[start:end]:
        if m := re.search(r"^\s*([\d.]+) .*frame=(\d+)us", line):
            out.append((float(m[1]), int(m[2]) / 1000))
    return [(t - out[0][0], f) for t, f in out]


directory = Path(sys.argv[1])
for phase, keep in PHASES.items():
    print(phase)
    for name in sys.argv[2:]:
        v = [f for t, f in frames(directory, name) if keep(t, f)]
        over = sum(f > 33.3 for f in v) / len(v) * 100
        print(
            f"  {name:6s} n={len(v):3d} median={median(v):5.1f} "
            f"p90={quantiles(v, n=10)[-1]:5.1f} over 33.3 ms={over:3.0f} %"
        )
