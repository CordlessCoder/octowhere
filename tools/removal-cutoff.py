# /// script
# requires-python = ">=3.11"
# ///
"""How a removal's switch lead plays out, per shape, from `RELAY_COST_KIND=removal` results and
their node logs: seconds until every member held its key (median and slowest), how far the
switch was from the start, the members that switched after the group (they learned within three
rounds of the switch or after it), and how long those were cut off, a run's sum and the worst.

    uv run tools/removal-cutoff.py results/built8.jsonl,results/lines-built8,steps32-again1 ...
"""
import json, re, statistics, sys
from collections import defaultdict
from pathlib import Path

ASK = re.compile(r"asks to remove \d+: generation \d+ from round (\d+) \(now (\d+)\)")
ROUND_S = 45

def run_cutoff(path):
    # Each member names its own switch: the group's, or three rounds after it learned.
    own = {}
    for line in path.read_text().splitlines():
        at, node, text = line.split(" ", 2)
        m = ASK.search(text)
        if m and node not in own:
            own[node] = (int(m.group(1)), int(m.group(2)))
    if not own:
        return None
    switch = min(s for s, _ in own.values())
    start = min(now for _, now in own.values())
    delays = [s - switch for s, _ in own.values()]
    return switch - start, delays

for label in sys.argv[1:]:
    jsonl, lines, variant = label.split(",")
    by = defaultdict(list)
    for l in open(jsonl):
        r = json.loads(l)
        if r["variant"] != variant: continue
        by[r["shape"]].append((r, run_cutoff(Path(lines) / f"{r['variant']}-{r['shape']}-{r['seed']}.log")))
    print(f"== {jsonl.split('/')[-1]}")
    for shape, runs in by.items():
        t = [r["reached_s"] if r["reached_s"] is not None else 1800 for r, _ in runs]
        lead = statistics.median(c[0] for _, c in runs if c) * ROUND_S
        delays = [d for _, c in runs if c for d in c[1]]
        late = sum(d > 0 for d in delays)
        cut = sum(delays) * ROUND_S
        worst = max(delays) * ROUND_S
        print(f"  {shape:<16} key {statistics.median(t):5.0f}/{max(t):5.0f}s  switch at {lead:4.0f}s  late {late:3d}  cut off {cut/len(runs):6.0f}s a run, worst {worst:4.0f}s")
