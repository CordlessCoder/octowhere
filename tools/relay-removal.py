# /// script
# requires-python = ">=3.11"
# ///
"""Sets a removal's cost beside its timing, per variant and shape, from the bench's results with
RELAY_COST_KIND=removal: the packets that carried messages, how long until every other member held
its key message (median and slowest of the seeds), and how many members took theirs only after the
first member switched, summed over the seeds.

    uv run tools/relay-removal.py results/lines-removal results/removal-*.jsonl
"""

import json
import statistics
import sys
from collections import defaultdict
from pathlib import Path


def late(path):
    """Members whose key message came after the first switch."""
    switched = None
    keyed = {}
    for line in path.read_text().splitlines():
        at, node, text = line.split(" ", 2)
        at = int(at)
        if "switched to generation" in text and (switched is None or at < switched):
            switched = at
        if " asks to remove " in text:
            keyed.setdefault(node, at)
    if switched is None:
        return 0
    return sum(at > switched for at in keyed.values())


def main():
    lines = Path(sys.argv[1])
    runs = defaultdict(list)
    variants, shapes = [], []
    for name in sys.argv[2:]:
        for line in open(name):
            run = json.loads(line)
            if run["variant"] not in variants:
                variants.append(run["variant"])
            if run["shape"] not in shapes:
                shapes.append(run["shape"])
            run["late"] = late(lines / f"{run['variant']}-{run['shape']}-{run['seed']}.log")
            runs[(run["variant"], run["shape"])].append(run)
    print("packets carrying messages (median) | s until every key held (median/slowest) | late members (sum)")
    print(f"{'shape':<16}" + "".join(f"{v:>28}" for v in variants))
    for shape in shapes:
        cells = []
        for variant in variants:
            group = runs.get((variant, shape))
            if not group:
                cells.append("-")
                continue
            got = [r["reached_s"] for r in group if r["reached_s"] is not None]
            never = len(group) - len(got)
            carrying = statistics.median(r["carrying"] for r in group)
            text = f"{carrying:.0f} | {statistics.median(got):.0f}/{max(got):.0f} | {sum(r['late'] for r in group)}" if got else "never"
            if never:
                text += f" !{never}"
            cells.append(text)
        print(f"{shape:<16}" + "".join(f"{c:>28}" for c in cells))


if __name__ == "__main__":
    main()
