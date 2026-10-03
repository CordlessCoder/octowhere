# /// script
# requires-python = ">=3.11"
# dependencies = ["pyelftools"]
# ///
"""Gives a board's mesh a pairing command over the USB JTAG, on firmware built with `pair-inject`.

    uv run tools/pair-inject.py join --probe 303a:1001:44:1B:F6:86:1A:38
    uv run tools/pair-inject.py add
    uv run tools/pair-inject.py choose 0
    uv run tools/pair-inject.py accept | decline | mismatch | cancel | leave
    uv run tools/pair-inject.py name "Ana's watch"
    uv run tools/pair-inject.py deaf 40
    uv run tools/pair-inject.py refresh
    uv run tools/pair-inject.py send 1 "meet at the car"
    uv run tools/pair-inject.py send group "back in 10"
    uv run tools/pair-inject.py remove 2
    uv run tools/pair-inject.py keep
    uv run tools/pair-inject.py phantom

The commands stand in for the screens until they are built: `add` and `join` start a pairing,
`choose` picks a device the adding side found, `accept`, `decline` and `mismatch` answer the code,
and `cancel` ends a pairing. `leave` tells the group and forgets it, and `name` renames this
device. `deaf` makes a pairing, or the mesh, drop every frame it hears for that many seconds, up to
255, to lose an acknowledgement or a switch on purpose. `refresh` listens throughout for three rounds, as REFRESH DEVICES
does. `send` sends text, up to 160 printable ASCII characters, to the member with that id,
privately, or to the whole group. `remove` removes the member with that id from the group, and
`keep` declines a removal another member asked for. `phantom` enrols a member no device stands
behind, so that two boards can try a removal with a member left to tell. With two boards attached, `--probe` picks one by its MAC, which
`probe-rs list` shows. The ELF must be the one flashed, since the addresses come from it. The
firmware logs `[MESH] command …` when it takes one.
"""

import argparse
import subprocess
import sys

from elftools.elf.elffile import ELFFile

CODES = {"add": 1, "join": 2, "choose": 3, "accept": 4, "decline": 5, "mismatch": 6,
         "cancel": 7, "leave": 8, "name": 9, "deaf": 10, "refresh": 11, "send": 12, "remove": 13, "keep": 14, "phantom": 15}
COMMAND = "OCTOWHERE_PAIR_COMMAND"
NAME = "OCTOWHERE_PAIR_NAME"
TEXT = "OCTOWHERE_PAIR_TEXT"


def symbols(elf, names):
    with open(elf, "rb") as file:
        table = ELFFile(file).get_section_by_name(".symtab")
        found = {s.name: s["st_value"] for s in table.iter_symbols() if s.name in names}
    missing = set(names) - found.keys()
    if missing:
        sys.exit(f"{elf} has no {', '.join(sorted(missing))}: build it with --features pair-inject")
    return found


def write(probe, address, value):
    subprocess.run(
        ["probe-rs", "write", "--chip", "esp32s3", "--probe", probe, "b32", hex(address),
         str(value)],
        check=True,
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("command", choices=CODES)
    parser.add_argument("argument", nargs="?",
                        help="the index for `choose`, the name for `name`, seconds for `deaf`, "
                             "the id or `group` for `send`, the id for `remove`")
    parser.add_argument("text", nargs="?", help="the text for `send`")
    parser.add_argument("--probe", default="303a:1001")
    parser.add_argument("--elf", default="target/xtensa-esp32s3-none-elf/release/octowhere")
    args = parser.parse_args()

    address = symbols(args.elf, [COMMAND, NAME, TEXT])
    argument = 0
    length = 0
    if args.command in ("choose", "remove"):
        argument = int(args.argument or 0)
    elif args.command == "deaf":
        argument = int(args.argument or 0)
        if not 0 <= argument <= 255:
            sys.exit("`deaf` takes 0 to 255 seconds")
    elif args.command == "name":
        name = (args.argument or "").encode("ascii")
        if not 1 <= len(name) <= 16 or not all(0x20 <= c < 0x7F for c in name):
            sys.exit("a name is 1 to 16 printable ASCII characters")
        padded = name.ljust(16, b"\0")
        for i in range(4):
            write(args.probe, address[NAME] + 4 * i, int.from_bytes(padded[4 * i:4 * i + 4], "little"))
        argument = len(name)
    elif args.command == "send":
        argument = 255 if args.argument == "group" else int(args.argument or 0)
        text = (args.text or "").encode("ascii")
        if not 1 <= len(text) <= 160 or not all(0x20 <= c < 0x7F for c in text):
            sys.exit("text is 1 to 160 printable ASCII characters")
        padded = text.ljust(160, b"\0")
        for i in range((len(text) + 3) // 4):
            write(args.probe, address[TEXT] + 4 * i, int.from_bytes(padded[4 * i:4 * i + 4], "little"))
        length = len(text)
    elif args.argument is not None:
        sys.exit(f"`{args.command}` takes no argument")
    # The command goes last: the firmware takes the name or text once it sees one.
    write(args.probe, address[COMMAND], CODES[args.command] | argument << 8 | length << 16)
    print(f"sent {args.command}" + (f" {args.argument}" if args.argument is not None else ""))


if __name__ == "__main__":
    main()
