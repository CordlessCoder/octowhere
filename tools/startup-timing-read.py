# /// script
# requires-python = ">=3.11"
# dependencies = ["pyelftools"]
# ///
"""Reads a startup-timing-bench build's marks over the USB JTAG, without a reset.

    uv run tools/startup-timing-read.py --probe 303a:1001:44:1B:F6:86:1A:38 > cold/1A38-1.log

For a cold start, which a serial capture would turn into a reset: power the board off and on,
wait until its start-up is over, then read. It prints the lines the firmware logs 9 s in, so
`tools/startup-timing-summary.py` reads its output as it reads a capture. The ELF must be the one
flashed, since the address comes from it.
"""

import argparse
import struct
import subprocess
import sys
import tempfile

from elftools.elf.elffile import ELFFile

SYMBOL = "OCTOWHERE_STARTUP_MARKS"
# The firmware's `STEPS`, in its order.
STEPS = [
    ("bring_up", "enter"),
    ("i2c", "powered"),
    ("exio", "reset"),
    ("gnss-settle", "end"),
    ("GNSS-config", "end"),
    ("frame_loop", "first"),
    ("startup", "over"),
]
PARTS = ["POWER", "CLOCK", "TOUCH", "MOTION", "MAGNET", "GNSS", "RADIO"]
OUTCOMES = {0: "ended", 1: "answered", 2: "no-reply", 3: "bad-reply"}
WORDS = len(STEPS) + 3 * len(PARTS) + 1


def address(elf):
    with open(elf, "rb") as file:
        table = ELFFile(file).get_section_by_name(".symtab")
        for symbol in table.iter_symbols():
            if symbol.name == SYMBOL:
                return symbol["st_value"]
    sys.exit(f"{elf} has no {SYMBOL}: build it with --features startup-timing-bench")


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--elf", default="firmware/target/xtensa-esp32s3-none-elf/release/octowhere")
    parser.add_argument("--probe", default="303a:1001")
    args = parser.parse_args()

    with tempfile.NamedTemporaryFile(suffix=".bin") as raw:
        subprocess.run(
            ["probe-rs", "read", "--chip", "esp32s3", "--probe", args.probe, "b32",
             hex(address(args.elf)), str(WORDS), "-o", raw.name, "-f", "binary"],
            check=True,
        )
        words = struct.unpack(f"<{WORDS}I", raw.read())

    board = args.probe.replace(":", "")[-4:].lower()
    marks = [(at, what, how) for (what, how), at in zip(STEPS, words) if at]
    parts = words[len(STEPS):len(STEPS) + 2 * len(PARTS)]
    outcomes = words[len(STEPS) + 2 * len(PARTS):-1]
    for index, part in enumerate(PARTS):
        start, end = parts[2 * index], parts[2 * index + 1]
        if start:
            marks.append((start, part, "start"))
        if end:
            marks.append((end, part, OUTCOMES.get(outcomes[index], "ended")))
    print(f"[STARTUP] board {board}")
    if words[-1]:
        print(f"[STARTUP] reset power_on={str(words[-1] == 2).lower()}")
    for at, what, how in sorted(marks):
        print(f"[STARTUP] {what} {how} {at}")


if __name__ == "__main__":
    main()
