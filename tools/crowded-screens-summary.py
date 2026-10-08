# /// script
# requires-python = ">=3.11"
# ///
"""Summarises a `crowded-screens-bench` run from the board's log: for each phase, how many
frames the frame loop made and what a step cost, how many drew and how many of those drew the
whole panel, what a draw and the flush that sent it cost, and the internal heap at its fullest.
Then, for the same phases, the step's cost apart for the frames that changed the panel and for
those that changed nothing. The two differ by a lot in the drawer, so the median over all frames
moves with how many of each a run made. Phases from 100 hold a screenshot, which halts the
board, and are left out.

    uv run tools/crowded-screens-summary.py <board log>

A step runs from the frame loop's wake to the end of acting on the stage's update, less what the
bench spent standing its group and messages in; a draw from there to the end of drawing into the
framebuffer. A flush is the display core's transfer of the frame, without its wait for TE. Times
are in milliseconds, the heap in bytes: the most in use after a step or a draw.
"""

import re
import statistics
import sys

# As tools/crowded-screens-bench.py numbers them.
NAMES = {
    0: "before",
    1: "clock",
    2: "members",
    3: "turn",
    4: "select",
    5: "turn-selected",
    6: "deselect",
    7: "drawer-open",
    8: "events-scroll",
    9: "messages-switch",
    10: "messages-scroll",
    11: "conversation",
    12: "back",
    13: "close",
    14: "end",
}
FULL = 466 * 466

ANSI = re.compile(r"\x1b\[[0-9;]*m")
STAMP = re.compile(r"(\d+\.\d+) \[\s*\w+\s*\]")
FRAME = re.compile(r"\[BENCH\] frame ((?:\w+=\d+(?:us)? ?)+)")
PHASE = re.compile(r"\[BENCH\] phase (\d+)")


def spread(values):
    """Median, 95th percentile and largest, in milliseconds, of microseconds."""
    if not values:
        return "     -      -      -"
    values = sorted(values)
    p95 = values[min(len(values) - 1, int(len(values) * 0.95))]
    return f"{statistics.median(values) / 1000:6.1f} {p95 / 1000:6.1f} {values[-1] / 1000:6.1f}"


def main():
    frames, phases = [], []
    with open(sys.argv[1], errors="replace") as log:
        for line in log:
            line = ANSI.sub("", line)
            stamp = STAMP.search(line)
            if found := FRAME.search(line):
                frames.append({
                    key: int(value.removesuffix("us"))
                    for key, value in (pair.split("=") for pair in found.group(1).split())
                })
            elif (found := PHASE.search(line)) and stamp:
                phases.append((int(found.group(1)), float(stamp.group(1))))

    if not frames:
        sys.exit("no `[BENCH] frame` lines")
    numbers = sorted(frame["n"] for frame in frames)
    missing = numbers[-1] - numbers[0] + 1 - len(set(numbers))
    started = {}
    for number, at in phases:
        started.setdefault(number, at)

    print(f"{'phase':19} {'s':>5} {'frames':>6} {'step med':>8} {'p95':>6} {'max':>6} "
          f"{'drawn':>5} {'full':>4} {'draw med':>8} {'p95':>6} {'max':>6} "
          f"{'flush med':>9} {'p95':>6} {'max':>6} {'heap peak':>9}")
    for number in sorted({frame["phase"] for frame in frames}):
        if number >= 100:
            continue
        own = [frame for frame in frames if frame["phase"] == number]
        drawn = [frame for frame in own if frame["repaint"] > 0]
        sent = [frame for frame in own if frame["pixels"] > 0]
        later = [at for phase, at in phases if at > started.get(number, float("inf"))]
        seconds = f"{min(later) - started[number]:5.1f}" if number in started and later else "    -"
        name = f"{number} {NAMES.get(number, '')}"
        print(f"{name:19} {seconds} {len(own):6} {spread([f['step'] for f in own])} "
              f"{len(drawn):5} {sum(f['repaint'] == FULL for f in drawn):4} "
              f"{spread([f['draw'] for f in drawn])} "
              f"{spread([f['flush'] for f in sent]):>23} {max(f['peak'] for f in own):9}")

    print()
    print(f"{'phase':19} {'changed':>7} {'step med':>8} {'p95':>6} {'max':>6} "
          f"{'unchanged':>9} {'step med':>8} {'p95':>6} {'max':>6}")
    for number in sorted({frame["phase"] for frame in frames}):
        if number >= 100:
            continue
        own = [frame for frame in frames if frame["phase"] == number]
        changed = [frame["step"] for frame in own if frame["pixels"] > 0]
        unchanged = [frame["step"] for frame in own if frame["pixels"] == 0]
        name = f"{number} {NAMES.get(number, '')}"
        print(f"{name:19} {len(changed):7} {spread(changed)} {len(unchanged):9} {spread(unchanged)}")

    print(f"{len(frames)} frames, {missing} missing from the log by their numbers")
    if phases:
        (first, first_at), (last, last_at) = phases[0], phases[-1]
        print(f"phases from {first} at {first_at:.3f} s to {last} at {last_at:.3f} s, "
              f"{last_at - first_at:.1f} s")


if __name__ == "__main__":
    main()
