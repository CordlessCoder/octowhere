# /// script
# requires-python = ">=3.11"
# dependencies = ["pyelftools"]
# ///
"""Sets the board's RTC over the USB JTAG, without a reset, on firmware built with `rtc-inject`.

    uv run tools/rtc-inject.py now
    uv run tools/rtc-inject.py 2026-12-31T23:59:50Z --as gnss
    uv run tools/rtc-inject.py 2026-03-29T00:59:30 --elf <elf>
    uv run tools/rtc-inject.py now --probe 303a:1001:44:1B:F6:86:1A:38

A time without an offset is UTC. `--as rtc`, the default, sets it as though the RTC had kept it,
so the clock shows it unconfirmed; `--as gnss` as though GNSS had just set it. After either, GNSS
no longer sets the clock until the firmware restarts. With two boards attached, `--probe` picks
one by its MAC, which `probe-rs list` shows. The ELF must be the one flashed, since the addresses
come from it. The firmware logs `[RTC] injected …` when it takes the time.
"""

import argparse
import datetime
import subprocess
import sys

from elftools.elf.elffile import ELFFile

MODES = {"rtc": 1, "gnss": 2}


def symbols(elf, names):
    with open(elf, "rb") as file:
        table = ELFFile(file).get_section_by_name(".symtab")
        found = {s.name: s["st_value"] for s in table.iter_symbols() if s.name in names}
    missing = set(names) - found.keys()
    if missing:
        sys.exit(f"{elf} has no {', '.join(sorted(missing))}: build it with --features rtc-inject")
    return found


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("time", help="`now`, or an ISO 8601 time; UTC unless it has an offset")
    parser.add_argument("--as", dest="mode", choices=MODES, default="rtc")
    parser.add_argument("--elf", default="firmware/target/xtensa-esp32s3-none-elf/release/octowhere")
    parser.add_argument("--probe", default="303a:1001")
    args = parser.parse_args()

    if args.time == "now":
        when = datetime.datetime.now(datetime.UTC)
    else:
        when = datetime.datetime.fromisoformat(args.time)
        if when.tzinfo is None:
            when = when.replace(tzinfo=datetime.UTC)
    seconds = int(when.timestamp())

    names = ["OCTOWHERE_RTC_INJECT_TIME", "OCTOWHERE_RTC_INJECT_MODE"]
    address = symbols(args.elf, names)
    # The mode goes second: the firmware takes the time once it sees a mode.
    for name, value in zip(names, [seconds, MODES[args.mode]]):
        subprocess.run(
            ["probe-rs", "write", "--chip", "esp32s3", "--probe", args.probe, "b32",
             hex(address[name]), str(value)],
            check=True,
        )
    print(f"injected {when.astimezone(datetime.UTC):%Y-%m-%d %H:%M:%S} UTC as {args.mode}")


if __name__ == "__main__":
    main()
