# /// script
# requires-python = ">=3.11"
# ///
"""Summarises a `touch-latency-bench` log: each stage's spread, per condition.

    uv run tools/touch-latency-summary.py <log>...

The log is the decoded defmt output, as `espflash monitor --log-format defmt` prints it. A read
is at rest when no contact was held before it and the frame loop was waiting for it, and during a
drag when a contact was held; either can happen with the compass sampling fast. Stages run from the report's edge on INT, or from the task
waking when no new report came, to the end of the flush that put the frame on the panel. Only
frames that changed the panel count towards the flush stages and the total.
"""

import re
import statistics
import sys

STAGES = [
    ("edge_wake", "edge → task wakes"),
    ("wake_read", "wake → read done"),
    ("read_recv", "read → frame loop takes it"),
    ("recv_step", "step"),
    ("step_draw", "draw"),
    ("draw_take", "draw → core 1 takes frame"),
    ("take_te", "take → TE"),
    ("te_flush", "flush"),
    ("total", "total"),
]
TAP_SLOP = 16
FIELD = re.compile(r"(\w+)=(\S+)")
ANSI = re.compile(r"\x1b\[[0-9;]*m")


def parse(paths):
    for path in paths:
        with open(path, "rb") as log:
            for raw in log:
                line = ANSI.sub("", raw.decode("utf-8", "replace"))
                if "[TOUCH-LATENCY] " not in line or "kind=" not in line:
                    continue
                fields = dict(FIELD.findall(line.split("[TOUCH-LATENCY]", 1)[1]))
                for key, value in fields.items():
                    if re.fullmatch(r"-?\d+", value):
                        fields[key] = int(value)
                yield fields


def spread(values):
    values = sorted(values)
    if not values:
        return "—"
    def at(q):
        return values[min(len(values) - 1, int(q * len(values)))]
    return (
        f"n={len(values):4d} min={values[0] / 1000:6.2f} p10={at(0.1) / 1000:6.2f} "
        f"med={statistics.median(values) / 1000:6.2f} p90={at(0.9) / 1000:6.2f} "
        f"max={values[-1] / 1000:6.2f}"
    )


def report(title, reads):
    if not reads:
        return
    changed = [read for read in reads if read["damage"] in ("full", "part")]
    sources = {}
    for read in reads:
        key = f"{read['src']}/{read['kind']}"
        sources[key] = sources.get(key, 0) + 1
    print(f"\n{title}: {len(reads)} reads, {len(changed)} changed the panel")
    print("  " + ", ".join(f"{key} {count}" for key, count in sorted(sources.items())))
    missed = sum(read["reports"] - 1 for read in reads if read["reports"] > 1)
    print(f"  reports overwritten before a read: {missed}")
    print("  ms, over frames that changed the panel:")
    for key, label in STAGES:
        values = [read[key] for read in changed if read[key] >= 0]
        print(f"    {label:28s} {spread(values)}")
    to_take = [
        read["total"] - read["take_te"] - read["te_flush"]
        for read in reads
        if read["damage"] not in ("full", "part")
    ]
    if to_take:
        print(f"    {'unchanged: → drawn or taken':28s} {spread(to_take)}")


def main():
    reads = list(parse(sys.argv[1:]))
    if not reads:
        sys.exit("no [TOUCH-LATENCY] lines")
    sequences = sorted(read["seq"] for read in reads if "seq" in read)
    if sequences:
        never = sequences[-1] - sequences[0] + 1 - len(sequences)
        print(f"reads the frame loop never took: {never} of {sequences[-1] - sequences[0] + 1}")
    lows = [read["low"] for read in reads if read["low"] > 0]
    print(f"INT low, the last time before each read, ms: {spread(lows)}")
    def condition(read):
        if read["first"] == "false":
            return "during a drag"
        if read.get("idle", "true") == "true":
            return "at rest"
        return "no contact, frame loop busy"

    # A contact that began at rest is a tap until it has moved TAP_SLOP, and nothing moves on the
    # panel before then. Its response runs from the report that first moved it that far to the
    # first frame that changed the panel after it.
    responses = {"false": [], "true": []}
    start = crossed = None
    for read in sorted((r for r in reads if "x" in r), key=lambda r: r["seq"]):
        if read["first"] == "true" and read["idle"] == "true" and read["kind"] == "contact":
            start, crossed = read, None
        elif start is None:
            continue
        elif crossed is None and read["kind"] == "contact":
            if max(abs(read["x"] - start["x"]), abs(read["y"] - start["y"])) > TAP_SLOP:
                crossed = read
        if crossed is not None and read["damage"] in ("full", "part"):
            responses[start["compass"]].append(read["at"] + read["total"] - crossed["at"])
            start = crossed = None
    for compass, values in responses.items():
        label = "drag from rest, slop crossed → first change" + (", compass" if compass == "true" else "")
        print(f"{label}, ms: {spread(values)}")

    for compass in ("false", "true"):
        for name in ("at rest", "no contact, frame loop busy", "during a drag"):
            report(
                name + (", compass sampling fast" if compass == "true" else ""),
                [r for r in reads if condition(r) == name and r["compass"] == compass],
            )


if __name__ == "__main__":
    main()
