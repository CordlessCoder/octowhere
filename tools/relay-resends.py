# /// script
# requires-python = ">=3.11"
# ///
"""Counts, in the logs `tests/relay_cost.rs` dumps (RELAY_COST_LINES), how many of the packets a
node sent again for a neighbour not heard passing a message on brought the message to a node that
lacked it.

    uv run tools/relay-resends.py results/lines
"""

import re
import sys
from collections import defaultdict
from pathlib import Path

SENT = re.compile(r"^(\d+) (\d+) \[MESH\] sent .* messages=(\d+) ")
HEARD = re.compile(r"^(\d+) (\d+) \[MESH\] heard id=(\d+) .* messages=(\d+)/(\d+) ")
AGAIN = re.compile(r"^(\d+) (\d+) \[MSG\] .* goes again")


def main():
    shapes = defaultdict(lambda: [0, 0, 0, 0])
    for path in sorted(Path(sys.argv[1]).glob("*.log")):
        shape = path.stem.rsplit("-", 1)[0]
        again_at = defaultdict(list)
        sends = []
        new_from = defaultdict(int)
        for line in path.read_text().splitlines():
            if m := AGAIN.match(line):
                again_at[int(m[2])].append(int(m[1]))
            elif m := SENT.match(line):
                if int(m[3]) > 0:
                    sends.append((int(m[1]), int(m[2])))
            elif m := HEARD.match(line):
                if int(m[4]) > 0:
                    new_from[(int(m[1]), int(m[3]))] += 1
        totals = shapes[shape]
        for at, node in sends:
            # A send within a second of the node's "goes again" is that send.
            again = any(0 <= at - t < 2_000_000 for t in again_at[node])
            # Receptions are logged at the packet's end; a send is logged at its end too.
            delivered = new_from.get((at, node), 0)
            totals[0 if not again else 1] += 1
            if again and delivered:
                totals[2] += 1
                totals[3] += delivered
    print(f"{'shape':<16}{'first':>7}{'again':>7}{'useful':>8}{'nodes reached':>15}")
    for shape, (first, again, useful, reached) in shapes.items():
        print(f"{shape:<16}{first:>7}{again:>7}{useful:>8}{reached:>15}")


if __name__ == "__main__":
    main()
