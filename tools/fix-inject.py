# /// script
# requires-python = ">=3.11"
# dependencies = ["pyelftools"]
# ///
"""Stands a position in for a GNSS fix over the USB JTAG, on firmware built with `fix-inject`.

    uv run tools/fix-inject.py 53.3498 -6.2603
    uv run tools/fix-inject.py 53.3498 -6.2603 --toward 25 420 --mesh --probe 303a:1001:44:1B:F6:86:1C:1C
    uv run tools/fix-inject.py off

The firmware takes the position as this device's fix every 250 ms while the RTC has a time, and
the screens show it, until `off`. With `--mesh` the mesh takes it too, stamped with the RTC's
time, which also stands in for GPS time. Two boards' RTCs disagree by more than the mesh's slots
allow, so give `--mesh` to one board of a group: the others take their timebase from it. Its
position then reaches the others' screens, and theirs never reaches it.

`--toward BEARING METRES` moves the position that far from the coordinates given, along a great
circle. With two boards attached, `--probe` picks one by its MAC, which `probe-rs list` shows.
The ELF must be the one flashed, since the addresses come from it.
"""

import argparse
import math
import subprocess
import sys

from elftools.elf.elffile import ELFFile

NAMES = [
    "OCTOWHERE_FIX_INJECT_LATITUDE",
    "OCTOWHERE_FIX_INJECT_LONGITUDE",
    "OCTOWHERE_FIX_INJECT_ON",
]
EARTH_M = 6_371_008.8


def symbols(elf):
    with open(elf, "rb") as file:
        table = ELFFile(file).get_section_by_name(".symtab")
        found = {s.name: s["st_value"] for s in table.iter_symbols() if s.name in NAMES}
    missing = set(NAMES) - found.keys()
    if missing:
        sys.exit(f"{elf} has no {', '.join(sorted(missing))}: build it with --features fix-inject")
    return found


def toward(latitude, longitude, bearing, metres):
    phi1, lambda1 = math.radians(latitude), math.radians(longitude)
    d, theta = metres / EARTH_M, math.radians(bearing)
    phi2 = math.asin(math.sin(phi1) * math.cos(d) + math.cos(phi1) * math.sin(d) * math.cos(theta))
    lambda2 = lambda1 + math.atan2(
        math.sin(theta) * math.sin(d) * math.cos(phi1), math.cos(d) - math.sin(phi1) * math.sin(phi2)
    )
    return math.degrees(phi2), (math.degrees(lambda2) + 540) % 360 - 180


def write(probe, address, value):
    subprocess.run(
        ["probe-rs", "write", "--chip", "esp32s3", "--probe", probe, "b32", hex(address),
         str(value & 0xFFFF_FFFF)],
        check=True,
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("latitude", help="degrees north, or `off`")
    parser.add_argument("longitude", nargs="?", type=float, help="degrees east")
    parser.add_argument("--toward", nargs=2, type=float, metavar=("BEARING", "METRES"))
    parser.add_argument("--mesh", action="store_true", help="the mesh takes it too")
    parser.add_argument("--elf", default="target/xtensa-esp32s3-none-elf/release/octowhere")
    parser.add_argument("--probe", default="303a:1001")
    args = parser.parse_args()
    address = symbols(args.elf)
    if args.latitude == "off":
        write(args.probe, address["OCTOWHERE_FIX_INJECT_ON"], 0)
        print("fix ended")
        return
    if args.longitude is None:
        sys.exit("give a longitude, or `off`")
    latitude, longitude = float(args.latitude), args.longitude
    if args.toward:
        latitude, longitude = toward(latitude, longitude, *args.toward)
    # The switch goes last: the firmware reads the position once it sees it on.
    write(args.probe, address["OCTOWHERE_FIX_INJECT_LATITUDE"], round(latitude * 1e7))
    write(args.probe, address["OCTOWHERE_FIX_INJECT_LONGITUDE"], round(longitude * 1e7))
    write(args.probe, address["OCTOWHERE_FIX_INJECT_ON"], 2 if args.mesh else 1)
    print(f"fix at {latitude:.7f} {longitude:.7f}{' for the mesh too' if args.mesh else ''}")


if __name__ == "__main__":
    main()
