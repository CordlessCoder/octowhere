# /// script
# dependencies = ["pillow"]
# ///
"""Side-by-side sheets for checking a screen against the design's renders.

  uv run tools/design-compare.py pair OUT.png OURS DESIGN [OURS DESIGN ...]
  uv run tools/design-compare.py frames OUT.png ANIM.gif 0,12,42,56

`pair` puts each of our frames (a PNG from the `render` example or a `ui-sim` recording) beside a
design render, one pair per row. Either side may name a GIF frame as `anim.gif:N`. `frames` lays
out the chosen frames of a GIF, four to a row.

GIF frames are expanded to 30 fps by their durations first, so N is the start-up's frame number
whether the GIF stores every frame or holds some for longer.
"""
import sys

from PIL import Image, ImageDraw, ImageSequence

SIZE = 466
LABEL = 20


def frames_at_30fps(path):
    frames = []
    for frame in ImageSequence.Iterator(Image.open(path)):
        held = max(1, round(frame.info.get("duration", 33) / (1000 / 30)))
        frames += [frame.convert("RGB").copy()] * held
    return frames


def load(spec):
    path, _, n = spec.rpartition(":")
    if path and n.isdigit():
        return frames_at_30fps(path)[int(n)]
    return Image.open(spec).convert("RGB")


def pair(out, specs):
    pairs = list(zip(specs[::2], specs[1::2]))
    sheet = Image.new("RGB", (2 * SIZE + 10, len(pairs) * (SIZE + LABEL)), (40, 40, 40))
    draw = ImageDraw.Draw(sheet)
    for row, specs in enumerate(pairs):
        for col, spec in enumerate(specs):
            x, y = col * (SIZE + 10), row * (SIZE + LABEL)
            sheet.paste(load(spec), (x, y))
            draw.text((x + 4, y + SIZE + 3), spec.split("/")[-1], fill=(255, 255, 255))
    sheet.save(out)


def frames(out, gif, picks):
    all_frames = frames_at_30fps(gif)
    picks = [int(n) for n in picks.split(",")]
    cols = 4
    rows = (len(picks) + cols - 1) // cols
    sheet = Image.new("RGB", (cols * SIZE, rows * (SIZE + LABEL)), (30, 30, 30))
    draw = ImageDraw.Draw(sheet)
    for k, n in enumerate(picks):
        x, y = (k % cols) * SIZE, (k // cols) * (SIZE + LABEL)
        sheet.paste(all_frames[n], (x, y))
        draw.text((x + 5, y + SIZE + 3), f"frame {n}", fill=(255, 255, 255))
    sheet.save(out)
    print(f"{len(all_frames)} frames at 30 fps")


if __name__ == "__main__":
    mode, out, *rest = sys.argv[1:]
    if mode == "pair" and rest and len(rest) % 2 == 0:
        pair(out, rest)
    elif mode == "frames" and len(rest) == 2:
        frames(out, *rest)
    else:
        sys.exit(__doc__)
