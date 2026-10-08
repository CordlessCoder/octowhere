# /// script
# requires-python = ">=3.11"
# ///
"""Frame loop steps during the self-test's checks, from [FRAMES] lines: per build, the steps
that woke before the last check ended, their step time (wake to hand-over) and the gap between
wakes, in ms. Each build's logs are one directory, as tools/startup-timing.sh writes them.

Usage: uv run tools/startup-frames-summary.py <out-dir> [<out-dir>...]
"""
import re, statistics, sys
from pathlib import Path

FRAME = re.compile(r"\[FRAMES\] (\d+) woke (\d+) drawn (\d+)")
MARK = re.compile(r"\[STARTUP\] (\S+) (\S+) (\d+)")

def pct(values, p):
    values = sorted(values)
    return values[min(len(values) - 1, int(p * len(values)))]

for d in sys.argv[1:]:
    steps, gaps, counts, longest = [], [], [], []
    for path in sorted(Path(d).glob("*.log")):
        text = path.read_text(errors="replace")
        marks = {(m[1], m[2]): int(m[3]) for m in MARK.finditer(text)}
        end = marks.get(("RADIO", "answered")) or marks.get(("bring_up", "exit"))
        frames = [(int(m[2]), int(m[3])) for m in FRAME.finditer(text) if int(m[2])]
        during = [(w, dr) for w, dr in frames if w < end]
        counts.append(len(during))
        steps += [(dr - w) / 1000 for w, dr in during]
        g = [(b[0] - a[0]) / 1000 for a, b in zip(during, during[1:])]
        gaps += g
        longest.append(max(g) if g else 0)
        first = frames[0][0] / 1000
        print(f"  {path.name}: checks end {end/1000:.0f} ms, {len(during)} steps from {first:.0f} ms, longest gap {max(g):.1f} ms")
    print(f"== {d}: steps during checks {statistics.median(counts)} per boot; step median {statistics.median(steps):.1f} p95 {pct(steps, .95):.1f} max {max(steps):.1f}; gap median {statistics.median(gaps):.1f} p95 {pct(gaps, .95):.1f} max {max(gaps):.1f}")
