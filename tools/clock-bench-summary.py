"""Median time per part of the clock face's draws in a `clock-draw-bench` log, in ms.

  uv run tools/clock-bench-summary.py LOG [LOG ...]

Draws are grouped as full redraws, ticks (under 3,000 changed pixels) and the rest. Draws of
the compass, which time no clock part, are left out.
"""
import re
import statistics
import sys
from collections import defaultdict

PARTS = ["draw", "clear", "parts", "scatter", "hours", "tile", "band", "bandtext", "column",
         "mark", "lines", "railring", "other"]

for path in sys.argv[1:]:
    groups = defaultdict(list)
    for line in open(path, errors="replace"):
        if "[BENCH] s=0" not in line:
            continue
        fields = dict(re.findall(r"(\w+)=(\w+)", line.split("[BENCH]")[1]))
        if all(fields[k] == "0" for k in ("parts", "scatter", "hours")):
            continue
        kind = "full" if fields["full"] == "true" else "tick" if int(fields["changed"]) < 3000 else "other"
        groups[kind].append({k: int(fields[k]) for k in PARTS})
    print(path)
    for kind in ("tick", "full", "other"):
        rows = groups[kind]
        if rows:
            medians = " ".join(f"{k}={statistics.median(r[k] for r in rows) / 1000:.2f}" for k in PARTS)
            print(f"  {kind:6} n={len(rows):3} {medians} | max {max(r['draw'] for r in rows) / 1000}")
