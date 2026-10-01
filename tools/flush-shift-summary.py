# /// script
# requires-python = ">=3.11"
# ///
"""Summarises a `flush-shift-bench` log: each case's median and p90 across rounds, in µs.

    uv run tools/flush-shift-summary.py <log>

The log is a reset read of the bench, `espflash monitor --non-interactive -L defmt --elf <elf>`.
"""

import re
import statistics
import sys
from collections import defaultdict

LINE = re.compile(
    r"\[FLUSHBENCH\] case=(\w+) region=(\w+) shift=(-?\d+),(-?\d+) n=\d+ "
    r"min=(\d+) median=(\d+) p90=(\d+) max=(\d+)"
)

cases = defaultdict(list)
for line in open(sys.argv[1], errors="replace"):
    if m := LINE.search(line):
        case, region, x, y, low, median, p90, high = m.groups()
        cases[(region, case, int(x), int(y))].append((int(low), int(median), int(p90), int(high)))

print(f"{'region':8} {'path':7} {'shift':>6} {'rounds':>6} {'min':>6} {'median':>7} {'p90':>6} {'max':>6}")
order = {"frame": 0, "band": 1, "compass": 2, "gauge": 3, "small": 4, "rows": 5}
for (region, case, x, y), rows in sorted(cases.items(), key=lambda kv: (order.get(kv[0][0], 9), kv[0][1] != "before", kv[0][2:])):
    low = min(r[0] for r in rows)
    median = statistics.median(r[1] for r in rows)
    p90 = statistics.median(r[2] for r in rows)
    high = max(r[3] for r in rows)
    print(f"{region:8} {case:7} {x:>3},{y:<2} {len(rows):>6} {low:>6} {median:>7.0f} {p90:>6.0f} {high:>6}")
