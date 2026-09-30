# /// script
# requires-python = ">=3.11"
# ///
"""Summarises how the touch controller reports lifts, from a `touch-read-log` capture.

    uv run tools/touch-lift-summary.py <log>

The log is the decoded defmt output, or its `[TOUCH-READ]` and `[TOUCH-LIFT]` lines. It prints
how long the stage took to count each lift after its report, each lift report a contact came back
from and how soon, how often a contact went quiet without one, and the longest gap between
reports while a finger was held still. With the raw bytes the log carries since
`finger-reads-raw.txt`, it also compares where each lift report puts the finger with the last
contact read before it.
"""

import re
import statistics
import sys

FIELD = re.compile(r"(\w+)=(\[[^\]]*\]|\S+)")
ANSI = re.compile(r"\x1b\[[0-9;]*m")
# A contact after a lift report sooner than this counts as the same finger coming back.
RETURN = 60_000


def spread(values):
    values = sorted(values)
    if not values:
        return "—"
    def at(q):
        return values[min(len(values) - 1, int(q * len(values)))]
    return (
        f"n={len(values)} min={values[0] / 1000:.1f} p10={at(0.1) / 1000:.1f} "
        f"med={statistics.median(values) / 1000:.1f} p90={at(0.9) / 1000:.1f} "
        f"max={values[-1] / 1000:.1f}"
    )


def main():
    reads, lifts, silent = [], [], 0
    for line in open(sys.argv[1], errors="replace"):
        line = ANSI.sub("", line)
        if "[TOUCH-LIFT]" in line:
            fields = dict(FIELD.findall(line))
            if "report_to_lift" in fields:
                lifts.append(int(fields["report_to_lift"]))
            else:
                silent += 1
        elif "[TOUCH-READ]" in line:
            fields = dict(FIELD.findall(line.split("[TOUCH-READ]", 1)[1]))
            for key in ("seq", "at", "x", "y"):
                fields[key] = int(fields[key])
            fields["raw"] = [int(byte, 16) for byte in fields.get("raw", "[]").strip("[]").split(", ") if byte]
            reads.append(fields)
    print(f"lift report → stage counts the lift, ms: {spread(lifts)}")
    print(f"lifts counted without a report: {silent}")

    fresh = [read for read in reads if read["kind"] != "stale"]
    reported, returns = 0, []
    for i, read in enumerate(fresh):
        after = fresh[i + 1] if i + 1 < len(fresh) else None
        if read["kind"] == "lift" and i and fresh[i - 1]["kind"] == "contact":
            reported += 1
            if after and after["kind"] in ("contact", "cover") and after["at"] - read["at"] < RETURN:
                returns.append(after["at"] - read["at"])
    print(f"lift reports after a contact: {reported}, of which a finger came back from: {len(returns)}")
    print(f"  back after, ms: {spread(returns)}")

    def position(raw):
        return (raw[1] << 4) | (raw[3] >> 4), (raw[2] << 4) | (raw[3] & 0x0F)

    same, newer = 0, []
    for before, read in zip(fresh, fresh[1:]):
        if read["kind"] == "lift" and before["kind"] == "contact" and read["raw"] and before["raw"]:
            (x, y), (last_x, last_y) = position(read["raw"]), position(before["raw"])
            if (x, y) == (last_x, last_y):
                same += 1
            else:
                newer.append((read["at"] - before["at"], max(abs(x - last_x), abs(y - last_y))))
    if same or newer:
        print(f"lift reports at the last contact read: {same}; elsewhere: {len(newer)}")
        for since, moved in sorted(newer):
            print(f"  {moved} px away, {since / 1000:.1f} ms after that contact")

    quiet = [
        b["at"] - a["at"]
        for a, b in zip(fresh, fresh[1:])
        if a["kind"] == b["kind"] == "contact" and b["at"] - a["at"] > 100_000
    ]
    print(f"contacts that went quiet with no lift report, gap to the next contact, ms: {spread(quiet)}")

    held, start, longest = [], None, 0
    for a, b in zip(fresh, fresh[1:]):
        still = (
            a["kind"] == b["kind"] == "contact"
            and abs(a["x"] - b["x"]) <= 2
            and abs(a["y"] - b["y"]) <= 2
            and b["at"] - a["at"] < 100_000
        )
        if still:
            start = a["at"] if start is None else start
            longest = max(longest, b["at"] - a["at"])
        elif start is not None:
            held.append((a["at"] - start, longest))
            start, longest = None, 0
    held.sort()
    print("longest still holds (ms held, longest gap between reports):")
    for duration, gap in held[-5:]:
        print(f"  {duration / 1000:.0f}, {gap / 1000:.1f}")


if __name__ == "__main__":
    main()
