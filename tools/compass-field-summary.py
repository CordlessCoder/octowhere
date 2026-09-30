# /// script
# requires-python = ">=3.11"
# ///
"""Summarises a `compass-field-bench` capture: each state's draw time, of the draws that repaint
anything, and the area each step flushes.

    uv run tools/compass-field-summary.py <log>

The log is the decoded defmt output. Draws within a second of a change of state are left out,
so the figures are for the settled field under a turning dial.
"""

import re
import statistics
import sys

STAMP = re.compile(r"^(\d+\.\d+) ")
ANSI = re.compile(r"\x1b\[[0-9;]*m")
NAMES = {"0": "heading", "1": "calibration", "2": "interference"}


def spread(values):
    values = sorted(values)
    if not values:
        return "—"
    at = lambda q: values[min(len(values) - 1, int(q * len(values)))]
    return (
        f"n={len(values)} med={statistics.median(values):.0f} p90={at(0.9):.0f} "
        f"max={values[-1]:.0f}"
    )


def main():
    phase, since = None, 0.0
    draws, pixels = {}, {}
    for line in open(sys.argv[1], errors="replace"):
        line = ANSI.sub("", line)
        stamp = STAMP.match(line)
        if not stamp:
            continue
        t = float(stamp.group(1))
        if "[BENCH] phase" in line:
            phase, since = NAMES[line.split("[BENCH] phase ")[1].split()[0]], t
        elif phase and t - since >= 1.0:
            if "[BENCH] draw" in line:
                draws.setdefault(phase, []).append(int(re.search(r"us=(\d+)", line).group(1)))
            elif "timing spi:" in line:
                pixels.setdefault(phase, []).append(int(re.search(r"pixels=(\d+)", line).group(1)))
    for name in NAMES.values():
        print(f"{name:13} draw µs: {spread(draws.get(name, []))}")
        print(f"{'':13} flushed px: {spread(pixels.get(name, []))}")


if __name__ == "__main__":
    main()
