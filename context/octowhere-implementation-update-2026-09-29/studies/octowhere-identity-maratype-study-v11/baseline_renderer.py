"""Native-size OCTOWHERE icon/identity options and exact reference annotation.

Requires the earlier octowhere-design-project as a sibling directory. The
reference frame is decoded from the user-supplied Marathon logo animation.
This study does not modify the handed-off design or firmware.
"""
from pathlib import Path
import math
import os
import subprocess
import sys
from PIL import Image, ImageChops, ImageDraw, ImageFont

HERE = Path(__file__).resolve().parent
WORK = HERE.parent
PROJECT = WORK / 'octowhere-design-project'
RENDERER = PROJECT / 'renderer'
os.chdir(RENDERER)
sys.path.insert(0, str(RENDERER))

from concept import startup_s1_g18 as G18
from concept import startup_s1_g17 as G17
import identity_g2 as G2
import marks_frame as MF

LIME = (192, 254, 4)
WHITE = (210, 211, 214)
PANEL = (18, 20, 23)
INK = (215, 218, 222)
PURPLE = (24, 7, 62)
MONO = ImageFont.truetype(str(RENDERER / 'fonts/MonoB.otf'), 17)
SMALL = ImageFont.truetype(str(RENDERER / 'fonts/MonoB.otf'), 14)


def annotate_reference():
    source = HERE / '_decoded-reference-frame.png'
    subprocess.run(['ffmpeg', '-hide_banner', '-loglevel', 'error', '-ss', '3.00',
                    '-i', str(PROJECT / 'references/video/marathon-official-logo-animation.mp4'),
                    '-frames:v', '1', '-y', str(source)], check=True)
    im = Image.open(source).convert('RGB')
    assert im.size == (3840, 2160)
    d = ImageDraw.Draw(im)
    pink = (255, 55, 128)
    # Source-frame coordinates, not an approximate redraw: the square itself
    # spans roughly x 1118–1185, y 1529–1594.
    d.rectangle((1107, 1518, 1197, 1605), outline=pink, width=9)
    d.line(((1197, 1605), (1270, 1686)), fill=pink, width=8)
    label_font = ImageFont.truetype(str(RENDERER / 'fonts/MonoB.otf'), 44)
    d.text((1288, 1653), 'FLASHING DOT / SQUARE', font=label_font, fill=pink)
    d.text((1288, 1709), 'MARATHON LOGO / 3.00 S', font=label_font, fill=WHITE)
    im.save(HERE / 'reference-square-outlined-3.00s.png', optimize=True)
    im.crop((975, 1440, 1375, 1690)).resize((800, 500), Image.Resampling.NEAREST).save(
        HERE / 'reference-square-detail.png', optimize=True)
    source.unlink()


def icon_options():
    base = Image.open(HERE / 'compass-hold-level-base.png').convert('RGB')
    options = [
        ('A / CURRENT L', '10000 10000 10000 10000 11111'),
        ('B / CENTERED HORIZON', '00000 00100 11111 00100 00000'),
        ('C / LEVEL VIAL', '00000 11111 10001 10101 11111'),
        ('D / DOT ABOVE LINE', '00000 00100 00000 11111 00000'),
    ]
    sheet = Image.new('RGB', (2 * 486 + 20, 2 * 516 + 20), PANEL)
    d = ImageDraw.Draw(sheet)
    for index, (label, pattern) in enumerate(options):
        im = base.copy()
        idraw = ImageDraw.Draw(im)
        # Preserve the firmware's 66 px outlined frame (x 200–265, y 87–152)
        # and substitute only the 50 × 50 five-module glyph inside it.
        idraw.rectangle((203, 90, 262, 149), fill=(0, 0, 0))
        for row, bits in enumerate(pattern.split()):
            for col, bit in enumerate(bits):
                if bit == '1':
                    x, y = 208 + 10 * col, 95 + 10 * row
                    idraw.rectangle((x, y, x + 9, y + 9), fill=WHITE)
        im.save(HERE / f'hold-level-{chr(65 + index).lower()}.png', optimize=True)
        x = 20 + index % 2 * 486
        y = 20 + index // 2 * 516
        sheet.paste(im, (x, y))
        d.text((x + 5, y + 475), label, font=MONO, fill=INK)
        d.text((x + 5, y + 498), pattern.replace(' ', '/'), font=SMALL, fill=(136, 142, 152))
    sheet.save(HERE / 'hold-level-icon-options.png', optimize=True)


def hash32(v):
    v = (v ^ (v >> 16)) * 0x7feb352d & 0xffffffff
    v = (v ^ (v >> 15)) * 0x846ca68b & 0xffffffff
    return (v ^ (v >> 16)) & 0xffffffff


def marks(cx, cy, radius, facing, density, seed):
    result = {}
    for iy in range(-2, 62):
        y = -2 + 8 * iy
        for ix in range(-2, 62):
            x = 12 + 8 * ix
            yy = y + 2 if y > 340 else y
            # Taller natural title and the enlarged subtitle share one clear field.
            if yy + 6 >= 163 and yy <= 340: continue
            if not (0 <= x - 3 and x + 3 < 466 and 0 <= yy - 3 and yy + 3 < 466): continue
            if math.hypot(x - 233, yy - 233) > 230: continue
            dx, dy = x - cx, y - cy
            distance = math.hypot(dx, dy)
            if distance > radius: continue
            radial = .45 + .55 * max(0, min(1, (distance - 40) / 180))
            turn = .45 + .55 * ((dx * math.cos(facing) + dy * math.sin(facing)) / distance if distance else 1)
            h = hash32(seed ^ hash32(ix * 0x9e3779b1 & 0xffffffff) ^ hash32(iy * 0x85ebca77 & 0xffffffff))
            if h / 2**32 >= max(0, radial * turn * density): continue
            kind = hash32(h ^ 0x12b591) & 0xffffffff
            result[(x, yy)] = kind / 2**32 < .60
    return result


def g19_scatter(source, n):
    im = source.copy()
    p = im.load()
    for y in range(466):
        for x in range(466):
            r, g, b = p[x, y]
            if b > r * 1.6 and b > g * 2 and r <= 48 and g <= 18:
                p[x, y] = (0, 0, 0)
    facing = -.65 if n < 66 else -.65 + .09 * min(5, (n - 66) // 11 + 1)
    combined = marks(190, 334, 125, 2.55, .40, 0x8c71)
    combined.update(marks(270, 145, 150, facing, .65, 0x8c70))
    scatter = Image.new('RGB', (466, 466))
    d = ImageDraw.Draw(scatter)
    for (x, y), hollow in combined.items():
        if hollow:
            d.rectangle((x - 3, y - 3, x + 2, y + 2), fill=PURPLE)
            d.rectangle((x - 1, y - 1, x, y), fill=(0, 0, 0))
        else:
            d.rectangle((x - 2, y - 2, x + 1, y + 1), fill=PURPLE)
    sp = scatter.load()
    for y in range(466):
        for x in range(466):
            if p[x, y] == (0, 0, 0) and sp[x, y] != (0, 0, 0):
                p[x, y] = sp[x, y]
    return im


def identity_variant(n, logo_xy, replace_lower):
    old_items = G2.ROW_ITEMS
    old_small = G17.small_logo
    if replace_lower:
        new_items = [item if item[0] != 'mark' else
                     ('g', '11111 10001 10101 10001 11111') for item in old_items]
        G2.ROW_ITEMS = new_items

    def placed_logo(cv, frame):
        state = G17.state(frame - G17.LOGO_START)
        if state in ('pre', 'off'): return
        x0, y0 = logo_xy
        module = 1.30
        for row, bits in enumerate(MF.ROW['L1']):
            for col, bit in enumerate(bits):
                if bit != '1' or (state == 'partial' and col not in (0, 7, 14)): continue
                x, y = x0 + col * module, y0 + row * module
                cv.rect(x, y, x + module, y + module, 'LIME')

    G17.small_logo = placed_logo
    try:
        result = g19_scatter(G18.frame(n, 'plume'), n)
    finally:
        G17.small_logo = old_small
        G2.ROW_ITEMS = old_items
    return result


def identity_options():
    options = [
        ('CURRENT / TWO MARKS', (407, 180), False),
        ('A / RIGHT OF UPPER FIELD', (428, 180), True),
        ('B / HIGH RIGHT ARC', (418, 135), True),
        ('C / TOP-RIGHT EDGE', (397, 102), True),
    ]
    sheet = Image.new('RGB', (2 * 486 + 20, 2 * 516 + 20), PANEL)
    d = ImageDraw.Draw(sheet)
    for index, (label, pos, replace_lower) in enumerate(options):
        im = identity_variant(100, pos, replace_lower)
        im.save(HERE / f'identity-position-{index}.png', optimize=True)
        x = 20 + index % 2 * 486
        y = 20 + index // 2 * 516
        sheet.paste(im, (x, y))
        d.text((x + 5, y + 475), label, font=MONO, fill=INK)
    sheet.save(HERE / 'identity-logo-position-options.png', optimize=True)
    original = Image.open(PROJECT / 'renderer/concept/startup-g19-matched/settled.png').convert('RGB')
    diff = ImageChops.difference(original, Image.open(HERE / 'identity-position-0.png'))
    print('Baseline recompose difference bbox:', diff.getbbox())


def main():
    annotate_reference()
    icon_options()
    identity_options()


if __name__ == '__main__': main()
