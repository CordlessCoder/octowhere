# /// script
# requires-python = ">=3.11"
# ///
"""Summarises tools/startup-timing.sh's captures: each milestone's median time since the timer
started, per board, in ms, and each part's check from start to end.

Usage: uv run tools/startup-timing-summary.py <out-dir> [<out-dir>...]
"""
import re
import statistics
import sys
from collections import defaultdict
from pathlib import Path

MARK = re.compile(r"\[STARTUP\] (\S+) (\S+) (\d+)")
MAC = re.compile(r"\[MESH\] id=.* mac=\[([0-9a-f, ]+)\]")
RESET = re.compile(r"\[STARTUP\] reset power_on=(\w+)")


def read(path):
    text = path.read_text(errors="replace")
    marks = [(m[1], m[2], int(m[3])) for m in MARK.finditer(text)]
    mac = MAC.search(text)
    board = "".join(mac[1].split(", ")[-2:]) if mac else path.stem.split("-")[0] + "?"
    reset = RESET.search(text)
    return board, marks, reset and reset[1]


def main():
    for out in sys.argv[1:]:
        print(f"== {out}")
        boots = defaultdict(list)
        for path in sorted(Path(out).glob("*.log")):
            board, marks, power_on = read(path)
            if not marks:
                print(f"  {path.name}: no marks")
                continue
            boots[board].append((path.name, marks, power_on))
        for board, runs in sorted(boots.items()):
            print(f"  board {board}: {len(runs)} boots")
            at = defaultdict(list)
            took = defaultdict(list)
            outcomes = defaultdict(set)
            for _, marks, _ in runs:
                started = {}
                for what, how, us in marks:
                    if how == "start":
                        started[what] = us
                        at[f"{what} start"].append(us)
                    elif what in started:
                        at[f"{what} end"].append(us)
                        took[what].append(us - started[what])
                        outcomes[what].add(how)
                    else:
                        at[f"{what} {how}"].append(us)
            order = sorted(at, key=lambda k: statistics.median(at[k]))
            for key in order:
                values = at[key]
                print(
                    f"    {key:22} {statistics.median(values) / 1000:8.1f} ms"
                    f"  ({min(values) / 1000:.1f}..{max(values) / 1000:.1f}, n={len(values)})"
                )
            print("    checks, start to end:")
            for part, values in took.items():
                print(
                    f"      {part:8} {statistics.median(values) / 1000:7.1f} ms"
                    f"  ({min(values) / 1000:.1f}..{max(values) / 1000:.1f})"
                    f"  {','.join(sorted(outcomes[part]))}"
                )
            resets = {power_on for _, _, power_on in runs}
            print(f"    power-on resets: {resets}")


main()
