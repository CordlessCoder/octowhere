# /// script
# requires-python = ">=3.11"
# dependencies = ["pyelftools"]
# ///
"""Drives a board flashed with `crowded-screens-bench` through the crowded screens.

It takes the member face and the Events and Messages drawer, each crowded, a phase at a time,
for tools/crowded-screens-summary.py to read from the board's log.

    uv run tools/crowded-screens-bench.py <elf> <probe> <log> [--turn 30] [--shots DIR]

    uv run tools/crowded-screens-bench.py \
        firmware/target/xtensa-esp32s3-none-elf/release/octowhere \
        303a:1001:44:1B:F6:86:1A:38 crowded.log

Start the board's log capture into `<log>` first, a reset read, and run this while it goes: it
waits for the start-up to end, stands a fix in at the members' centre through
tools/fix-inject.py, then runs each phase, printing it as it starts with where the screens were.
Gestures go through tools/touch-inject.py, and the phase and the turning heading through the
firmware's `OCTOWHERE_BENCH_PHASE` and `OCTOWHERE_BENCH_TURN`, over the USB JTAG. It stops when a
phase finds the screens somewhere its gestures would not do what they are for.
host-tests/src/crowded_screens.rs steps the same gestures through the stage on the host, and
checks each phase reaches what it is named for.

The member face places its members only with a fix and the RTC's time, and turns only with a
true heading, which needs both: the line for phase 4 should end `TRUE / FORWARD`. A board whose
RTC has no time takes one from `uv run tools/rtc-inject.py now --elf <elf> --probe <probe>` first.

With `--shots DIR` it reads the screen back after each phase into DIR, in a phase of its own
numbered 100 more, since a read halts the board for about 11 s.
"""

import argparse
import math
import re
import subprocess
import sys
import time
from pathlib import Path

from elftools.elf.elffile import ELFFile

TOOLS = Path(__file__).resolve().parent
PHASE = "OCTOWHERE_BENCH_PHASE"
TURN = "OCTOWHERE_BENCH_TURN"
# The fix the firmware places its members around, `CENTRE` in firmware/src/crowded_bench/fixture.rs.
CENTRE = ("53.3498", "-6.2603")

# Gestures, on the panel as the touch controller reports them, each from the UI code or a test or
# render that makes the same move:
# - from the clock face to the member face, a swipe right that wraps the pager round: the
#   reverse of the last swipe of `the_member_face_follows_the_compass_in_the_ring` in
#   crates/octowhere-ui/tests/members.rs, which wraps from the member face to the clock;
TO_MEMBERS = (80, 233, 400, 233, 250)
TO_CLOCK = (400, 233, 80, 233, 250)
# - a tap in the member face's middle selects the next member: `select` in
#   crates/octowhere-ui/examples/render/members.rs, within `MIDDLE_RADIUS` of ui/members.rs;
MIDDLE = (233, 233)
# - the drawer's upward drag: `open_drawer` in crates/octowhere-ui/examples/render/events.rs;
OPEN_DRAWER = (233, 430, 233, 120, 250)
# - a root list's scroll: "events-drawer-history" in examples/render/events.rs, and its reverse;
SCROLL_DOWN = (233, 380, 233, 133, 400)
SCROLL_UP = (233, 133, 233, 380, 400)
# - from Events to Messages: `inbox` in examples/render/messages.rs;
TO_MESSAGES = (380, 260, 80, 260, 300)
# - the inbox's first row: "messages-02-group" in examples/render/messages.rs;
FIRST_ROW = (233, 170)
# - the title, which goes back from a conversation and closes a root: within `TOP_HIT` in
#   ui/drawer/parts.rs, between the (233, 20) of examples/render/events.rs and the (233, 26) of
#   "messages-11-recipient-removed-draft" in examples/render/messages.rs.
TITLE = (233, 25)

# The drawer's rows, viewport and the first row's gap: `ROW` in ui/drawer/rows.rs, `INBOX_ROW` in
# ui/drawer/messages.rs, and `VIEWPORT_HEIGHT` and `FIRST` in ui/drawer/mod.rs.
EVENT_ROW, INBOX_ROW, VIEWPORT, FIRST = 178, 95, 287, 4
# Taps that step the selection from the face's own choice, the freshest member, through every
# other to Kestrel at 043° inside the 035-061° run: the hand-off's
# "members-05-crowded-selected-exact". One more is the freshest again.
TO_KESTREL = 30

ANSI = re.compile(r"\x1b\[[0-9;]*m")
READY = re.compile(r"\[BENCH\] ready")
PHASE_LINE = re.compile(r"\[BENCH\] phase (\d+) (.*?)(?:\s+\([^()]*:\d+\))?\s*$")


def symbols(elf):
    with open(elf, "rb") as file:
        table = ELFFile(file).get_section_by_name(".symtab")
        found = {s.name: s["st_value"] for s in table.iter_symbols() if s.name in (PHASE, TURN)}
    if set(found) != {PHASE, TURN}:
        sys.exit(f"{elf} has no {PHASE} or {TURN}: build it with --features crowded-screens-bench")
    return found


class Board:
    def __init__(self, elf, probe, log):
        self.elf, self.probe, self.log = str(Path(elf).resolve()), probe, Path(log)
        self.address = symbols(self.elf)
        self.read_to = 0
        self.pending = []

    def write(self, name, value):
        subprocess.run(
            ["probe-rs", "write", "--chip", "esp32s3", "--probe", self.probe, "b32",
             hex(self.address[name]), str(value & 0xFFFF_FFFF)],
            check=True, capture_output=True,
        )

    def tool(self, script, *arguments):
        subprocess.run(
            ["uv", "run", "--quiet", str(TOOLS / script), *map(str, arguments),
             "--probe", self.probe, "--elf", self.elf],
            check=True, capture_output=True,
        )

    def swipe(self, gesture):
        x0, y0, x1, y1, ms = gesture
        self.tool("touch-inject.py", "swipe", x0, y0, x1, y1, "--ms", ms)

    def tap(self, point):
        self.tool("touch-inject.py", "tap", *point)

    def wait_for(self, pattern, timeout):
        """The next line of the log to match `pattern`, read as the capture writes it."""
        deadline = time.monotonic() + timeout
        while True:
            while self.pending:
                found = pattern.search(self.pending.pop(0))
                if found:
                    return found
            if time.monotonic() > deadline:
                return None
            with open(self.log, "rb") as file:
                file.seek(self.read_to)
                data = file.read()
            end = data.rfind(b"\n") + 1
            self.read_to += end
            self.pending = ANSI.sub("", data[:end].decode(errors="replace")).splitlines()
            if not self.pending:
                time.sleep(0.2)


def fields(text):
    """A phase line's `key=value` pairs, and its heading's caption, which is last and has spaces
    in it."""
    pairs, _, heading = text.partition(" heading=")
    found = dict(pair.split("=", 1) for pair in pairs.split() if "=" in pair)
    found["heading"] = heading
    return found


def scrolls(rows, row):
    """The swipes that take a list of `rows` rows from its top to its end, the last stopping
    there. As many take it back to the top, none starting there, where a downward drag would
    close the drawer instead (`Exit::Pull` in ui/drawer/mod.rs)."""
    travel = SCROLL_DOWN[1] - SCROLL_DOWN[3]
    return max(0, math.ceil((FIRST + rows * row - VIEWPORT) / travel))


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("elf")
    parser.add_argument("probe", help="the probe selector, e.g. 303a:1001:44:1B:F6:86:1A:38")
    parser.add_argument("log", help="the board's log, as its capture writes it")
    parser.add_argument("--turn", type=int, default=30, help="degrees a second the heading turns")
    parser.add_argument("--shots", type=Path, help="read the screen back after each phase")
    parser.add_argument(
        "--drawer-over",
        choices=("members", "clock"),
        default="members",
        help="the face the drawer opens over: the member face builds its list under it every step",
    )
    args = parser.parse_args()
    board = Board(args.elf, args.probe, args.log)

    def turn(rate):
        board.write(TURN, rate)

    def taps(count):
        for _ in range(count):
            board.tap(MIDDLE)
            time.sleep(0.3)

    def scroll(count):
        for gesture in (SCROLL_DOWN, SCROLL_UP):
            for _ in range(count):
                board.swipe(gesture)
                time.sleep(1)

    def clock(state):
        time.sleep(10)

    def members(state):
        board.swipe(TO_MEMBERS)
        time.sleep(10)

    def turning(state):
        turn(args.turn)
        time.sleep(30)

    def select(state):
        turn(0)
        taps(TO_KESTREL)
        time.sleep(3)

    def turning_selected(state):
        turn(args.turn)
        time.sleep(20)

    def deselect(state):
        turn(0)
        taps(1)
        time.sleep(8)
        if args.drawer_over == "clock":
            board.swipe(TO_CLOCK)
            time.sleep(3)

    def drawer_open(state):
        board.swipe(OPEN_DRAWER)
        time.sleep(10)

    def events_scroll(state):
        scroll(scrolls(int(state["events"]), EVENT_ROW))

    def messages_switch(state):
        board.swipe(TO_MESSAGES)
        time.sleep(6)

    def messages_scroll(state):
        scroll(scrolls(int(state["conversations"]), INBOX_ROW))

    def conversation(state):
        board.tap(FIRST_ROW)
        time.sleep(6)

    def back(state):
        board.tap(TITLE)
        time.sleep(5)

    def close(state):
        board.tap(TITLE)
        time.sleep(8)

    def end(state):
        time.sleep(2)

    # Each phase's number, its name, where its start must find the screens, and what it does.
    phases = [
        (1, "clock", {"screen": "clock", "drawer": "closed"}, clock),
        (2, "members", {"screen": "clock", "drawer": "closed"}, members),
        (3, "turn", {"screen": "members", "drawer": "closed"}, turning),
        (4, "select", {"screen": "members", "drawer": "closed"}, select),
        (5, "turn-selected", {"screen": "members", "drawer": "closed"}, turning_selected),
        (6, "deselect", {"screen": "members", "drawer": "closed"}, deselect),
        (7, "drawer-open", {"screen": args.drawer_over, "drawer": "closed"}, drawer_open),
        (8, "events-scroll", {"drawer": "events"}, events_scroll),
        (9, "messages-switch", {"drawer": "events"}, messages_switch),
        (10, "messages-scroll", {"drawer": "messages"}, messages_scroll),
        (11, "conversation", {"drawer": "messages"}, conversation),
        (12, "back", {"drawer": "child"}, back),
        (13, "close", {"drawer": "messages"}, close),
        (14, "end", {"screen": args.drawer_over, "drawer": "closed"}, end),
    ]

    print("waiting for the start-up to end", flush=True)
    if not board.wait_for(READY, 120):
        sys.exit(f"no `[BENCH] ready` in {args.log}: is its capture running on this board?")
    turn(0)
    board.tool("fix-inject.py", *CENTRE)
    time.sleep(3)
    try:
        for number, name, expected, act in phases:
            board.write(PHASE, number)
            found = board.wait_for(PHASE_LINE, 10)
            while found and int(found.group(1)) != number:
                found = board.wait_for(PHASE_LINE, 10)
            if not found:
                sys.exit(f"phase {number} {name}: the board never logged it")
            print(f"phase {number} {name}: {found.group(2)}", flush=True)
            state = fields(found.group(2))
            wrong = {key: state.get(key) for key, value in expected.items() if state.get(key) != value}
            if wrong:
                sys.exit(f"phase {number} {name} needs {expected}, and found {wrong}; stopping")
            if number == 4 and not state["heading"].endswith("FORWARD"):
                print("  the face had no true heading as it turned: it needs a fix and the RTC's time")
            act(state)
            if args.shots and number < phases[-1][0]:
                args.shots.mkdir(parents=True, exist_ok=True)
                board.write(PHASE, 100 + number)
                time.sleep(1)
                board.tool("touch-inject.py", "shot", (args.shots / f"{number:02}-{name}.png").resolve())
                time.sleep(6)
    finally:
        turn(0)
        board.tool("fix-inject.py", "off")
    print("done")


if __name__ == "__main__":
    main()
