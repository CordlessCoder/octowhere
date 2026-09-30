# /// script
# requires-python = ">=3.11"
# dependencies = ["pillow", "numpy"]
# ///
"""Writes the compass's settled background textures into `crates/octowhere-ui/src/ui/compass_texture.rs`.

    uv run tools/compass-texture.py

The textures are the design's fixtures from `renderer/concept/compass_noise.py` in the design
project: heading on blocks seed 4, calibration on triangles seed 8, interference on blocks seed 5
(`context/design/handoffs/IMPLEMENTATION-HANDOFF-2026-09-30/`). `blocks`, `plate` and
`triangles` below are that source's, unchanged but for drawing through a recorder, so Python's
generator gives the same shapes. The script records each shape in the order drawn, repaints the
recording and checks it against the source's own drawing pixel for pixel before writing.
"""

import math
import random
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw

W = 466
BLUE = [(4, 11, 29), (6, 19, 49), (9, 31, 72), (15, 47, 103), (25, 66, 140), (37, 88, 170)]
OUT = Path(__file__).resolve().parents[1] / "crates/octowhere-ui/src/ui/compass_texture.rs"


def highlight(col):
    return tuple(min(255, int(v * 1.30) + 2) for v in col)


# Shade codes: 0 black, 1 to 5 a palette level, and 8 added for its highlight. The source never
# draws level 0.
SHADES = {(0, 0, 0): 0}
for level, col in enumerate(BLUE[1:], 1):
    SHADES[col] = level
    SHADES[highlight(col)] = level | 8


class Recorder:
    """Draws like `ImageDraw` and keeps each shape: ("rect", x0, y0, x1, y1, shade) with inclusive
    corners, or ("triangle", x, y, rising, shade)."""

    def __init__(self):
        self.image = Image.new("RGB", (W, W))
        self.draw = ImageDraw.Draw(self.image)
        self.shapes = []

    def rectangle(self, box, fill):
        self.draw.rectangle(box, fill=fill)
        self.shapes.append(("rect", *box, SHADES[fill]))

    def line(self, box, fill):
        # Every line the source draws is one row.
        x0, y0, x1, y1 = box
        assert y0 == y1
        self.draw.line(box, fill=fill)
        self.shapes.append(("rect", x0, y0, x1, y1, SHADES[fill]))

    def point(self, xy, fill):
        self.draw.point(xy, fill=fill)
        self.shapes.append(("rect", *xy, *xy, SHADES[fill]))

    def polygon(self, points, fill):
        self.draw.polygon(points, fill=fill)
        # Both of the source's triangles have 19 px legs on the cell's corner (x, y).
        x, y = points[2][0], points[0][1]
        rising = points == [(x + 19, y), (x + 19, y + 19), (x, y + 19)]
        assert rising or points == [(x, y), (x + 19, y), (x, y + 19)]
        self.shapes.append(("triangle", x, y, rising, SHADES[fill]))


def blocks(seed):
    rng = random.Random(34383 + seed * 2081)
    d = Recorder()
    centres = []
    for i in range(15):
        x = rng.randrange(-20, 440)
        y = rng.choice([rng.randrange(20, 115), rng.randrange(340, 444), rng.randrange(145, 329)])
        w = rng.choice([26, 43, 66, 91, 122]); h = rng.choice([8, 13, 21, 34, 46]); lev = rng.randrange(1, 5)
        plate(d, x, y, w, h, BLUE[lev], rng)
        centres.append((x + w // 2, y + h // 2))
    for i in range(110):
        cx, cy = rng.choice(centres); x = cx + rng.randrange(-66, 67); y = cy + rng.randrange(-42, 43)
        w = rng.choice([2, 4, 6, 9, 14, 22, 32]); h = rng.choice([2, 3, 4, 7, 12])
        plate(d, x, y, w, h, BLUE[rng.randrange(1, 5)], rng)
    return d


def plate(d, x, y, w, h, col, rng):
    d.rectangle((x, y, x + w - 1, y + h - 1), fill=col)
    if h > 4:
        for yy in range(y + rng.randrange(3), y + h, 3):
            d.line((x, yy, x + w - 1, yy), fill=tuple(min(255, int(v * 1.30) + 2) for v in col))
    if w > 25 and rng.random() < .6:
        xx = x + rng.randrange(5, w - 6)
        d.rectangle((xx, y, xx + rng.randrange(3, 9), y + rng.randrange(2, min(8, h) + 1)), fill=(0, 0, 0))


def triangles(seed):
    rng = random.Random(44003 + seed * 481)
    d = Recorder()
    for y in range(14, 460, 21):
        for x in range(14, 460, 21):
            # Broken clusters, with large black gaps; no shifting of a prior frame.
            radial = math.hypot(x - 233, y - 233)
            if rng.random() > (.46 if radial > 144 else .16): continue
            lev = rng.choice([1, 2, 2, 3, 4])
            if rng.random() < .5: points = [(x, y), (x + 19, y), (x, y + 19)]
            else: points = [(x + 19, y), (x + 19, y + 19), (x, y + 19)]
            d.polygon(points, fill=BLUE[lev])
            if rng.random() < .1: d.point((x + 10, y + 10), fill=BLUE[5])
    return d


def triangle_rows(rising):
    """Each row's first and last column, relative to the triangle's corner, as Pillow fills it."""
    im = Image.new("L", (40, 40))
    points = [(19, 0), (19, 19), (0, 19)] if rising else [(0, 0), (19, 0), (0, 19)]
    ImageDraw.Draw(im).polygon(points, fill=255)
    a = np.array(im)
    rows = []
    for y in range(40):
        xs = np.nonzero(a[y])[0]
        if len(xs):
            assert y == len(rows) and xs[-1] - xs[0] + 1 == len(xs)
            rows.append((int(xs[0]), int(xs[-1])))
    return rows


def repaint(shapes):
    a = np.zeros((W, W, 3), dtype=np.uint8)
    colours = {code: col for col, code in SHADES.items()}
    spans = {False: triangle_rows(False), True: triangle_rows(True)}
    for shape in shapes:
        if shape[0] == "rect":
            _, x0, y0, x1, y1, shade = shape
            rows = [(y, x0, x1) for y in range(y0, y1 + 1)]
        else:
            _, x, y, rising, shade = shape
            rows = [(y + dy, x + a0, x + a1) for dy, (a0, a1) in enumerate(spans[rising])]
        for y, x0, x1 in rows:
            if 0 <= y < W:
                x0, x1 = max(x0, 0), min(x1, W - 1)
                if x0 <= x1:
                    a[y, x0 : x1 + 1] = colours[shade]
    return a


def rust(name, doc, shapes):
    lines = [f"/// {doc}", f"pub const {name}: &[Shape] = &["]
    for shape in shapes:
        if shape[0] == "rect":
            _, x0, y0, x1, y1, shade = shape
            lines.append(f"    Shape::Rect({x0}, {y0}, {x1 - x0 + 1}, {y1 - y0 + 1}, {shade}),")
        else:
            _, x, y, rising, shade = shape
            lines.append(f"    Shape::Triangle({x}, {y}, {'true' if rising else 'false'}, {shade}),")
    lines.append("];")
    return "\n".join(lines)


def main():
    fixtures = [("HEADING", blocks(4)), ("CALIBRATION", triangles(8)), ("INTERFERENCE", blocks(5))]
    for name, d in fixtures:
        assert (repaint(d.shapes) == np.array(d.image)).all(), f"{name} does not repaint the same"
    # `Shape::rows` in the firmware fills a triangle this way.
    assert triangle_rows(False) == [(0, 19 - dy) for dy in range(20)]
    assert triangle_rows(True) == [(19 - dy, 19) for dy in range(20)]
    out = [
        "//! The compass's settled background textures, written by `tools/compass-texture.py` from the",
        "//! design's `compass_noise.py`. Do not edit by hand.",
        "",
        "use super::compass_screen::Shape;",
        "",
        rust("HEADING", "Blocks, seed 4, in the order drawn.", fixtures[0][1].shapes),
        "",
        rust("CALIBRATION", "Triangles, seed 8, in the order drawn.", fixtures[1][1].shapes),
        "",
        rust("INTERFERENCE", "Blocks, seed 5, in the order drawn.", fixtures[2][1].shapes),
    ]
    OUT.write_text("\n".join(out) + "\n")
    for name, d in fixtures:
        print(f"{name}: {len(d.shapes)} shapes")


if __name__ == "__main__":
    main()
