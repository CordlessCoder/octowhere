# /// script
# requires-python = ">=3.11"
# ///
"""Sets the bench's variants side by side, per shape: the packets that carried the message and
how long every node took to hold it, as medians over the seeds, with the slowest seed.

    uv run tools/relay-variants.py results/variants.jsonl
"""

import json
import statistics
import sys
from collections import defaultdict


def main():
    runs = defaultdict(list)
    variants = []
    shapes = []
    for line in open(sys.argv[1]):
        run = json.loads(line)
        if run["variant"] not in variants:
            variants.append(run["variant"])
        if run["shape"] not in shapes:
            shapes.append(run["shape"])
        runs[(run["variant"], run["shape"])].append(run)
    print("packets that carried it (median), and seconds until all held it (median/slowest)")
    print(f"{'shape':<22}" + "".join(f"{v:>22}" for v in variants))
    for shape in shapes:
        cells = []
        for variant in variants:
            group = runs[(variant, shape)]
            carrying = statistics.median(run["carrying"] for run in group)
            reached = [run["reached_s"] for run in group]
            never = sum(r is None for r in reached)
            got = [r for r in reached if r is not None]
            text = f"{carrying:.0f} {statistics.median(got):.1f}/{max(got):.0f}" if got else "never"
            if never:
                text += f" !{never}"
            cells.append(text)
        print(f"{shape:<22}" + "".join(f"{c:>22}" for c in cells))


if __name__ == "__main__":
    main()
