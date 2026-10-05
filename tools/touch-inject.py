# /// script
# requires-python = ">=3.11"
# dependencies = ["pyelftools", "pillow"]
# ///
"""Touches a board's screen, covers it or presses its power key over the USB JTAG, and reads the
screen back, on firmware built with `touch-inject`.

    uv run tools/touch-inject.py tap 233 190 --probe 303a:1001:44:1B:F6:86:1A:38
    uv run tools/touch-inject.py swipe 233 80 233 420 [--ms 300]
    uv run tools/touch-inject.py cover | short | long
    uv run tools/touch-inject.py shot screen.png

Points are on the panel, before the pixel shift is taken off, as the touch controller reports
them. `short` and `long` press the power key. `shot` reads the framebuffer drawn last, so let
the screen settle first: the two buffers match once nothing has changed for a frame. With two
boards attached, `--probe` picks one by its MAC, which `probe-rs list` shows. The ELF must be
the one flashed, since the addresses come from it.
"""

import argparse
import os
import subprocess
import sys
import tempfile

from elftools.elf.elffile import ELFFile
from PIL import Image

PATH = "OCTOWHERE_TOUCH_PATH"
GO = "OCTOWHERE_TOUCH_GO"
FRAMEBUFFERS = "OCTOWHERE_FRAMEBUFFERS"
KINDS = {"stroke": 1, "cover": 2, "short": 3, "long": 4}
SIDE = 466


def symbols(elf, names):
    with open(elf, "rb") as file:
        table = ELFFile(file).get_section_by_name(".symtab")
        found = {s.name: s["st_value"] for s in table.iter_symbols() if s.name in names}
    missing = set(names) - found.keys()
    if missing:
        sys.exit(f"{elf} has no {', '.join(sorted(missing))}: build it with --features touch-inject")
    return found


def probe_rs(probe, *args, **kwargs):
    return subprocess.run(["probe-rs", *args, "--chip", "esp32s3", "--probe", probe], check=True,
                          **kwargs)


def write(probe, address, value):
    probe_rs(probe, "write", "b32", hex(address), str(value))


def read_word(probe, address):
    out = probe_rs(probe, "read", "b32", hex(address), "1", capture_output=True, text=True)
    # Each line opens with its address.
    return int(out.stdout.split()[-1], 16)


def stroke(probe, address, start, end, ms):
    write(probe, address[PATH], start[0] | start[1] << 16)
    write(probe, address[PATH] + 4, end[0] | end[1] << 16)
    write(probe, address[GO], KINDS["stroke"] | ms << 8)


def shot(probe, address, out):
    buffer = read_word(probe, address[FRAMEBUFFERS] + 8)
    if buffer == 0:
        sys.exit("nothing drawn yet")
    with tempfile.TemporaryDirectory() as directory:
        raw = os.path.join(directory, "fb.bin")
        probe_rs(probe, "read", "b32", hex(buffer), str(SIDE * SIDE // 2), "-o", raw, "-f", "binary")
        data = open(raw, "rb").read()
    pixels = bytearray()
    for i in range(0, SIDE * SIDE * 2, 2):
        value = data[i] << 8 | data[i + 1]
        r, g, b = value >> 11, value >> 5 & 63, value & 31
        pixels += bytes((r << 3 | r >> 2, g << 2 | g >> 4, b << 3 | b >> 2))
    Image.frombytes("RGB", (SIDE, SIDE), bytes(pixels)).save(out)


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("command", choices=["tap", "swipe", "cover", "short", "long", "shot"])
    parser.add_argument("arguments", nargs="*")
    parser.add_argument("--ms", type=int, default=300, help="a swipe's length")
    parser.add_argument("--probe", default="303a:1001")
    parser.add_argument("--elf", default="firmware/target/xtensa-esp32s3-none-elf/release/octowhere")
    args = parser.parse_args()

    address = symbols(args.elf, [PATH, GO, FRAMEBUFFERS])
    values = args.arguments
    if args.command == "tap":
        x, y = map(int, values)
        stroke(args.probe, address, (x, y), (x, y), 0)
    elif args.command == "swipe":
        x0, y0, x1, y1 = map(int, values)
        stroke(args.probe, address, (x0, y0), (x1, y1), args.ms)
    elif args.command == "shot":
        (out,) = values
        shot(args.probe, address, out)
    else:
        write(args.probe, address[GO], KINDS[args.command])


if __name__ == "__main__":
    main()
