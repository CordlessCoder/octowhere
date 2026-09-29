"""Centered octagonal opening and Maratype identity title study.

Preserves the selected horizon and V3 subtitle/identity timing. Run beside
the earlier octowhere-design-project.
"""
from pathlib import Path
import math
import os
import subprocess
import sys

from PIL import Image, ImageChops, ImageDraw, ImageFont

HERE = Path(__file__).resolve().parent
WORK = HERE.parent
RENDERER = WORK / 'octowhere-design-project' / 'renderer'
os.chdir(RENDERER)
sys.path.insert(0, str(RENDERER))
import baseline_renderer as V1
from concept import startup_s1_g18 as G18
from concept import startup_s1_g17 as G17
from concept import startup_s1_g11 as G11
from concept import startup_s1 as S1
import identity_g2 as G2
import identity_f as F
import identity_g as IG
import lib
import marks_frame as MF
import startup30 as S30
from anim import FRAME_MS
from identity import finish
from lib import Canvas, SS

LIME = (192, 254, 4)
PANEL = (18, 20, 23)
WHITE = (210, 211, 214)
SMALL = ImageFont.truetype(str(RENDERER / 'fonts/MonoB.otf'), 15)
ROW_Y = 311
ROW_GAP = 8
LOGO_X = 437
LOGO_Y = 212
LOGO_MODULE = 0.80
SELECTED_STYLE = 'needle'
SELECTED_SCALE = 1.17  # 12 × 14 source silhouette becomes ~14 × 16 px
HEAD_STYLE = 'round'
TITLE_VARIANT = 'natural'
TICK_LAYOUT = 'aligned'
CORNER_GAP = 10
DOT_FIRST_FRAME = 22
DOT_PERIOD = 30


lib.FONT['maratype'] = str(HERE / 'Maratype.otf')
ORIGINAL_ART = G11.art()
TITLE_SETTINGS = {
    # face, px, x-stretch, y-stretch, optical centre x/y
    'fit': ('maratype', 49, 2.27, .75, 233, 233),
    'medium': ('maratype', 72, 1.40, .70, 233, 222),
    'tall': ('maratype', 95, 1.00, 1.00, 233, 200),
    # No transforms: ~399.5px wide, matching the former ~398.25px title.
    'natural': ('maratype', 112, 1.00, 1.00, 233, 233),
}


def build_title_art(style):
    if style == 'shapiro':
        return ORIGINAL_ART
    face, px, sx, sy, cx, cy = TITLE_SETTINGS[style]
    mask = IG.tm(G11.G10.TITLE, face, px, sx=sx, sy=sy)
    edge = IG.outline(mask, w=1.55)
    left = round((cx - mask.width / SS / 2) * SS)
    top = round((cy - mask.height / SS / 2) * SS)
    bounds = [0] + [min(edge.width, IG.tm(G11.G10.TITLE[:i], face, px,
                                           sx=sx, sy=sy).width)
                    for i in range(1, len(G11.G10.TITLE))] + [edge.width]
    return mask, edge, left, top, bounds


ARTS = {name: build_title_art(name) for name in ('shapiro', *TITLE_SETTINGS)}


def title_ink_bounds(style):
    """Filled title after the same 4x-to-panel BOX raster pass."""
    mask, _, left, top, _ = ARTS[style]
    surface = Image.new('L', (466 * SS, 466 * SS), 0)
    surface.paste(mask, (left, top))
    return surface.resize((466, 466), Image.Resampling.BOX).getbbox()


INK_BOUNDS = {name: title_ink_bounds(name) for name in ARTS}
TITLE_LEFT, _, TITLE_RIGHT, _ = INK_BOUNDS[TITLE_VARIANT]
ROW_LEFT = TITLE_LEFT
# The row already has the correct height. Give its 67px barcode advance the
# entire missing width, distributing that space over its bars and intervals.
# The mono advance exceeds its visible ink by about 2 panel pixels. Compensate
# so the *visible* row, rather than its typographic advance, reaches the edge.
BARCODE_ADVANCE = (TITLE_RIGHT - TITLE_LEFT) - (25 + 72 + 25 + 134.375 + 4*ROW_GAP) + 2
BARCODE_SCALE = BARCODE_ADVANCE / 67
MARK_ORIGIN = round(ROW_LEFT + BARCODE_ADVANCE + ROW_GAP)


def title_layout(style):
    left, top, right, bottom = INK_BOUNDS[style]
    pluses = ((left - CORNER_GAP, top - CORNER_GAP),
              (right - 1 + CORNER_GAP, bottom - 1 + CORNER_GAP))
    logo = (right - 1 + 5, top - 2)
    return pluses, logo


def dot_strength(n):
    """Reference-shaped pulse at half speed: dark every 30 frames."""
    phase = (n - DOT_FIRST_FRAME) % DOT_PERIOD
    return 0.5 * (1 - math.cos(2 * math.pi * phase / DOT_PERIOD))


def octagon_symbol(cv, n):
    x, y = MARK_ORIGIN, ROW_Y
    # 25px square aligns visually with the two-line 29px copy band.
    cv.rect(x, y, x + 25, y + 25, 'LIME')
    cv.poly([(x + 8, y + 3), (x + 17, y + 3),
             (x + 22, y + 8), (x + 22, y + 17),
             (x + 17, y + 22), (x + 8, y + 22),
             (x + 3, y + 17), (x + 3, y + 8)], 'BLACK')
    k = dot_strength(n)
    if k > .005:
        cv.rect(x + 10, y + 10, x + 15, y + 15, tuple(round(c*k) for c in LIME))


def wide_barcode(cv, data, x, y, h=25):
    """Keep the source bit pattern, widen every bar and interval equally."""
    at = 0
    for byte in data.encode():
        for k in range(4):
            w = 2 if (byte >> k) & 1 else 1
            cv.rect(x + at*BARCODE_SCALE, y,
                    x + (at+w)*BARCODE_SCALE, y+h, 'LIME')
            at += w+2
    assert at == 67
    return x + at*BARCODE_SCALE


def micro_row(cv, count, n):
    """Balanced 25px visual band with the established sequential reveal."""
    if count <= 0:
        return
    x = ROW_LEFT
    x = wide_barcode(cv, '0.1.0', x, ROW_Y) + ROW_GAP
    if count < 2:
        return
    octagon_symbol(cv, n)
    x += 25 + ROW_GAP
    if count < 3:
        return
    # Pixel digits: 4px horizontal modules, 5px vertical modules.
    for char in '12 07':
        if char == ' ':
            x += 8
            continue
        for j, row in enumerate(F.PIX[char].split()):
            for i, bit in enumerate(row):
                if bit == '1':
                    cv.rect(x + i*4, ROW_Y + j*5,
                            x + (i+1)*4, ROW_Y + (j+1)*5, 'LIME')
        x += 16
    x += ROW_GAP
    if count < 4:
        return
    for j, row in enumerate('00100 01010 10101 01010 00100'.split()):
        for i, bit in enumerate(row):
            if bit == '1':
                cv.rect(x+i*5, ROW_Y+j*5, x+(i+1)*5, ROW_Y+(j+1)*5, 'LIME')
    x += 25 + ROW_GAP
    if count < 5:
        return
    cv.text(G2.LINES[0], 'mono', 14, 'LIME', x=x, y=ROW_Y-2)
    cv.text(G2.LINES[1], 'mono', 14, 'LIME', x=x, y=ROW_Y+14)


def marker_outer(head):
    """Normalized 12 × 14 pin; only the head profile changes."""
    if head == 'angular':
        return [(3, 0), (9, 0), (12, 3), (12, 8),
                (6, 14), (0, 8), (0, 3)]
    if head == 'round':
        # Upper semicircle: smooth at full-card scale, softened at 14px.
        arc = [(6 + 6 * math.cos(math.pi + math.pi * i / 32),
                6 + 6 * math.sin(math.pi + math.pi * i / 32))
               for i in range(33)]
        return arc + [(12, 8), (6, 14), (0, 8)]
    if head == 'soft':
        def bezier(a, b, c, d, t):
            return ((1-t)**3 * a[0] + 3*(1-t)**2*t*b[0] + 3*(1-t)*t*t*c[0] + t**3*d[0],
                    (1-t)**3 * a[1] + 3*(1-t)**2*t*b[1] + 3*(1-t)*t*t*c[1] + t**3*d[1])
        left = [bezier((0, 6), (0, 2), (2, 0), (6, 0), i / 16) for i in range(17)]
        right = [bezier((6, 0), (10, 0), (12, 2), (12, 6), i / 16) for i in range(1, 17)]
        return left + right + [(12, 8), (6, 14), (0, 8)]
    raise ValueError(head)


HOLE_PREVIOUS = [(5, 2), (7, 2), (9, 4), (9, 6),
                 (7, 8), (5, 8), (3, 6), (3, 4)]
MARKER_HOLE = [(5, 3), (7, 3), (9, 5), (9, 7),
               (7, 9), (5, 9), (3, 7), (3, 5)]


def hole_points(variant):
    if variant == 'centered':
        return MARKER_HOLE
    if variant == 'previous':
        return HOLE_PREVIOUS
    raise ValueError(variant)


def marker_logo(cv, frame, style, scale=SELECTED_SCALE,
                head=HEAD_STYLE, x=LOGO_X, y=LOGO_Y,
                hole_variant='centered'):
    state = G17.state(frame - G17.LOGO_START)
    if state in ('pre', 'off'):
        return
    if style == 'original':
        for j, bits in enumerate(MF.ROW['L1']):
            for i, bit in enumerate(bits):
                if bit != '1' or (state == 'partial' and i not in (0, 7, 14)):
                    continue
                xx, yy = x + i * LOGO_MODULE, y + j * LOGO_MODULE
                cv.rect(xx, yy, xx + LOGO_MODULE, yy + LOGO_MODULE, 'LIME')
        return
    if state == 'partial':
        # Three registration-like surviving stems, matching the prior flicker cue.
        for xx, yy, h in ((0, 3, 4), (6, 3, 6), (11, 3, 4)):
            cv.rect(x + xx * scale, y + yy * scale,
                    x + xx * scale + 1, y + (yy + h) * scale, 'LIME')
        return
    if style == 'faceted':
        outer = [(3, 0), (9, 0), (12, 3), (12, 7), (6, 12),
                 (0, 7), (0, 3)]
        hole = [(5, 2), (7, 2), (9, 4), (9, 5), (7, 7),
                (5, 7), (3, 5), (3, 4)]
    elif style == 'open':
        outer = [(3, 0), (9, 0), (12, 3), (12, 7), (6, 12),
                 (0, 7), (0, 3)]
        hole = [(4.5, 2), (7.5, 2), (10, 4), (10, 6),
                (7.5, 8), (4.5, 8), (2, 6), (2, 4)]
    elif style == 'needle':
        outer = marker_outer(head)
        hole = hole_points(hole_variant)
    else:
        raise ValueError(style)
    cv.poly([(x + xx * scale, y + yy * scale) for xx, yy in outer], 'LIME')
    cv.poly([(x + xx * scale, y + yy * scale) for xx, yy in hole], 'BLACK')


def identity_frame(n, style=SELECTED_STYLE, tick_layout=TICK_LAYOUT,
                   logo_scale=SELECTED_SCALE, head=HEAD_STYLE,
                   title_variant=TITLE_VARIANT, hole_variant='centered'):
    old_row = G2.row
    old_logo = G17.small_logo
    old_ticks = G17.lime_ticks
    old_art = G11.art
    pluses, logo_xy = title_layout(title_variant)

    def row_with_octagon(cv, count):
        micro_row(cv, count, n)

    def lowered_logo(cv, frame):
        marker_logo(cv, frame, style, logo_scale, head, *logo_xy, hole_variant)

    def compact_ticks(cv, frame):
        if tick_layout == 'original':
            return old_ticks(cv, frame)
        if frame < 36:
            return
        state = G17.state(frame - G17.FLICKER_START)
        color = 'LIME' if state == 'on' else G17.DIM_LIME
        if state == 'partial':
            color = (111, 149, 5)
        if tick_layout == 'center':
            positions = ((23, 233), (443, 233))
        elif tick_layout == 'diagonal':
            positions = ((23, 215), (443, 250))
        elif tick_layout == 'aligned':
            positions = pluses
        else:
            raise ValueError(tick_layout)
        for x, y in positions:
            # Seven-pixel arms, exactly one panel pixel thick at their center.
            cv.rect(x - 3, y, x + 4, y + 1, color)
            if state != 'partial':
                cv.rect(x, y - 3, x + 1, y + 4, color)

    G2.row, G17.small_logo, G17.lime_ticks, G11.art = (
        row_with_octagon, lowered_logo, compact_ticks, lambda: ARTS[title_variant])
    try:
        return V1.g19_scatter(G18.frame(n, 'plume'), n)
    finally:
        G2.row, G17.small_logo, G17.lime_ticks, G11.art = (
            old_row, old_logo, old_ticks, old_art)


def board():
    options = [('shapiro', 'PREVIOUS INSET / SHAPIRO'),
               ('medium', 'V9 / STRETCHED MARATYPE'),
               ('natural', 'V10 / NATURAL MARATYPE')]
    sheet = Image.new('RGB', (2 * 486 + 20, 2 * 516 + 20), PANEL)
    d = ImageDraw.Draw(sheet)
    for index, (style, label) in enumerate(options):
        im = identity_frame(97, title_variant=style)
        im.save(HERE / f'identity-title-{style}.png', optimize=True)
        x, y = 20 + index % 2 * 486, 20 + index // 2 * 516
        sheet.paste(im, (x, y))
        d.text((x + 4, y + 475), label, font=SMALL, fill=WHITE)
    sheet.save(HERE / 'identity-typeface-comparison.png', optimize=True)


def change_board():
    """At-size composition and enlarged subtitle from the immediately prior study."""
    prior = Image.open(HERE / 'v10-identity-frame-097.png').convert('RGB')
    current = identity_frame(97)
    sheet = Image.new('RGB', (992, 726), PANEL)
    d = ImageDraw.Draw(sheet)
    for i, (im, label, band) in enumerate((
            (prior, 'V10 / COMPACT ROW', (0, 304, 466, 344)),
            (current, 'V11 / FULL-WIDTH ROW', (0, 304, 466, 344)))):
        x = 20 + i*486
        sheet.paste(im, (x, 20))
        d.text((x+4, 494), label, font=SMALL, fill=WHITE)
        crop = im.crop(band).resize((466, 80), Image.Resampling.NEAREST)
        sheet.paste(crop, (x, 550))
        d.text((x+4, 650), 'SUBTITLE / 2x HEIGHT', font=SMALL, fill=WHITE)
    sheet.save(HERE / 'layout-before-after.png', optimize=True)


def pinhole_board():
    sheet = Image.new('RGB', (992, 516), PANEL)
    detail = Image.new('RGB', (2 * 216 + 20, 246), PANEL)
    d, dd = ImageDraw.Draw(sheet), ImageDraw.Draw(detail)
    x0, y0 = title_layout(TITLE_VARIANT)[1]
    for i, (variant, label) in enumerate((('previous', 'PREVIOUS / HOLE CENTER Y5'),
                                          ('centered', 'CENTERED / HEAD + HOLE Y6'))):
        im = identity_frame(97, hole_variant=variant)
        x = 20 + i * 486
        sheet.paste(im, (x, 20))
        d.text((x+4, 495), label, font=SMALL, fill=WHITE)
        crop = im.crop((x0-4, y0-4, x0+20, y0+20))
        detail.paste(crop.resize((192, 192), Image.Resampling.NEAREST),
                     (20+i*216, 12))
        dd.text((22+i*216, 216), 'OLD / Y5' if i == 0 else 'CENTERED / Y6',
                font=SMALL, fill=WHITE)
    sheet.save(HERE / 'pinhole-position-comparison.png', optimize=True)
    detail.save(HERE / 'pinhole-detail.png', optimize=True)


def type_motion_board():
    picks = [(24, 'TYPE 24'), (38, 'TYPE 38'), (42, 'FILL 42'),
             (44, 'OUTLINE 44'), (51, 'PARTIAL 51'), (69, 'SETTLED 69')]
    size, gap, cap = 233, 12, 28
    sheet = Image.new('RGB', (3*size+4*gap, 2*(size+cap)+3*gap), PANEL)
    d = ImageDraw.Draw(sheet)
    for i,(n,label) in enumerate(picks):
        im = identity_frame(n)
        x=gap+(i%3)*(size+gap); y=gap+(i//3)*(size+cap+gap)
        sheet.paste(im.resize((size,size), Image.Resampling.LANCZOS),(x,y))
        d.text((x+3,y+size+4),label,font=SMALL,fill=WHITE)
    sheet.save(HERE / 'maratype-motion-checkpoints.png',optimize=True)


def dot_detail():
    # Use true panel pixels enlarged with nearest-neighbour resampling.
    phases = [(82, 'DARK / 0 MS'), (90, 'RISING / 267 MS'),
              (97, 'BRIGHT / 500 MS'), (105, 'FALLING / 767 MS')]
    sheet = Image.new('RGB', (800, 310), PANEL)
    d = ImageDraw.Draw(sheet)
    for i, (n, caption) in enumerate(phases):
        im = identity_frame(n)
        crop = im.crop((MARK_ORIGIN - 4, ROW_Y - 4, MARK_ORIGIN + 19, ROW_Y + 19))
        x = 20 + i * 200
        sheet.paste(crop.resize((184, 184), Image.Resampling.NEAREST), (x, 30))
        d.text((x, 230), caption, font=SMALL, fill=WHITE)
    d.text((21, 271), 'SQUARE STEADY / CENTRAL DOT PULSES EVERY 1000 MS', font=SMALL, fill=WHITE)
    sheet.save(HERE / 'identity-octagon-dot-phases.png', optimize=True)


def impact_mask(scale=1.0, head=HEAD_STYLE, hole_variant='centered'):
    """Full-screen version of the same 12 × 14 needle geometry and hole."""
    unit = 17.5 * scale   # normal mark: 210 px wide, 245 px high
    x0, y0 = 233 - 6 * unit, 233 - 7 * unit
    outer = marker_outer(head)
    hole = hole_points(hole_variant)
    mask = Image.new('L', (466 * SS, 466 * SS), 0)
    d = ImageDraw.Draw(mask)
    for points, fill in ((outer, 255), (hole, 0)):
        d.polygon([((x0 + xx * unit) * SS, (y0 + yy * unit) * SS)
                   for xx, yy in points], fill=fill)
    return mask


def marker_impact(n, head=HEAD_STYLE, hole_variant='centered'):
    """Existing 19-frame impact cadence with the new map marker silhouette."""
    if n == 18:
        return Image.new('RGB', (466, 466), 'black')
    cv = Canvas()
    full = Image.new('L', cv.img.size, 255)
    mark = impact_mask(1.9 if n >= 13 else 1.0, head, hole_variant)
    if n < 9 or n == 17:
        G17.LC.paint(cv, full, 'LIME')
    if n < 3:
        G17.LC.paint(cv, ImageChops.multiply(mark, G17.LC.stripes()), 'BLACK')
    elif n < 9:
        G17.LC.paint(cv, mark, 'BLACK')
    elif n < 17:
        G17.LC.paint(cv, mark, 'LIME')
    else:
        G17.LC.paint(cv, mark, 'BLACK')
    return finish(cv)


def impact_board():
    samples = [(1, 'STRIPES'), (4, 'DARK MARK'), (10, 'LIGHT MARK'),
               (14, 'POP 1.9×'), (17, 'IMPACT')]
    tile, gap = 186, 12
    row_h = tile + 50
    sheet = Image.new('RGB', (5 * tile + 6 * gap, 2 * row_h + 2 * gap), PANEL)
    d = ImageDraw.Draw(sheet)
    for row, (title, render) in enumerate((
            ('HOLE 1 UNIT HIGH', lambda n: marker_impact(n, 'round', 'previous')),
            ('HOLE CENTERED', lambda n: marker_impact(n, 'round', 'centered')))):
        for col, (frame, name) in enumerate(samples):
            im = render(frame)
            x = gap + col * (tile + gap)
            y = gap + row * (row_h + gap) + 20
            sheet.paste(im.resize((tile, tile), Image.Resampling.LANCZOS), (x, y))
            d.text((x, y + tile + 5), f'{frame:02d} / {name}', font=SMALL, fill=WHITE)
            if row == 1:
                im.save(HERE / f'impact-frame-{frame:02d}.png', optimize=True)
        d.text((gap + 2, gap + row * (row_h + gap) + 2), title,
               font=SMALL, fill=WHITE)
    sheet.save(HERE / 'impact-hole-comparison.png', optimize=True)


def boundary_board():
    samples = [('IDENTITY / 119', identity_frame(119)),
               ('IMPACT / 00', marker_impact(0)),
               ('IMPACT / 17', marker_impact(17)),
               ('BLACK / 18', marker_impact(18)),
               ('CLOCK / 01', S1.clock_entry(1)),
               ('CLOCK / 16', S1.clock_entry(16))]
    size, gap, cap = 300, 12, 28
    sheet = Image.new('RGB', (3 * size + 4 * gap,
                             2 * (size + cap) + 3 * gap), PANEL)
    d = ImageDraw.Draw(sheet)
    for i, (label, frame) in enumerate(samples):
        x = gap + (i % 3) * (size + gap)
        y = gap + (i // 3) * (size + cap + gap)
        sheet.paste(frame.resize((size, size), Image.Resampling.LANCZOS), (x, y))
        d.text((x + 3, y + size + 5), label, font=SMALL, fill=WHITE)
    sheet.save(HERE / 'sequence-boundaries.png', optimize=True)


def save_mp4(frames, path):
    cmd = ['ffmpeg', '-hide_banner', '-loglevel', 'error', '-y',
           '-f', 'rawvideo', '-pix_fmt', 'rgb24', '-s', '466x466',
           '-r', '30', '-i', 'pipe:0', '-an', '-c:v', 'libx264',
           '-pix_fmt', 'yuv420p', '-crf', '10', '-movflags', '+faststart',
           str(path)]
    with subprocess.Popen(cmd, stdin=subprocess.PIPE) as p:
        for frame in frames:
            p.stdin.write(frame.convert('RGB').tobytes())
        p.stdin.close()
        if p.wait():
            raise RuntimeError(f'ffmpeg failed: {path}')


def animation():
    identity = [identity_frame(n) for n in range(120)]
    impact = [marker_impact(n) for n in range(19)]
    for n in (56, 82, 97, 112, 119):
        identity[n].save(HERE / f'identity-frame-{n:03d}.png', optimize=True)
    save_mp4(identity + impact, HERE / 'identity-and-impact.mp4')
    selftest = [S1.post(n * FRAME_MS) for n in range(S30.LAST + S30.HOLD)]
    clock = [S1.clock_entry(n) for n in range(1, 17)]
    full = selftest + identity + impact + clock + [clock[-1]] * 18
    assert len(full) == 203
    save_mp4(full, HERE / 'startup-complete.mp4')
    clock[-1].save(HERE / 'clock-handoff.png', optimize=True)


def main():
    HERE.mkdir(exist_ok=True)
    board()
    change_board()
    type_motion_board()
    dot_detail()
    boundary_board()
    animation()
    print('ink', INK_BOUNDS, 'motion study', TITLE_VARIANT,
          'layout', title_layout(TITLE_VARIANT), 'head', HEAD_STYLE)


if __name__ == '__main__':
    main()
