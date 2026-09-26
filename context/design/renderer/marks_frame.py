"""A second family of marks, after the owner's idea (25 Sep): an outline like an icon tile's,
surrounded by four corners and four edge lines. Octagonal symmetry, drawn in strokes.

Geometry on the 5 x 5 module grid, module m, stroke w = frame_for(m) (the icons' frame rule):
  the tile: a square outline over the inner 3 x 3 modules (1m to 4m)
  corners:  L brackets in the four corner modules, arms 1m along the outer edge
  edges:    L1 a line along the outer edge over the middle module (2m to 3m)
            L2 a tick from the outer edge inward, 0.6m long, centred on the middle module
  L3:       the outer ring as a broken octagon: eight sides, each short of its corners by a gap
Card: m = 42 (210 x 210). Row: a hand-set 15 x 15 px version, 1 px strokes.
"""
import math
import lib
lib.RECORD = False
from lib import *
from PIL import ImageChops
from identity import finish, circle

W = 466
M_CARD = 42


def strokes(d, x0, y0, m, w, variant, fill=255, parts=('tile', 'outer')):
    """Draw the mark with its top-left at (x0, y0) px, in SS units onto an L-mode draw."""
    S = 5 * m

    def R(ax, ay, bx, by):
        d.rectangle([(x0 + ax) * SS, (y0 + ay) * SS, (x0 + bx) * SS - 1, (y0 + by) * SS - 1], fill=fill)

    if 'tile' in parts:
        a, b = m, 4 * m
        R(a, a, b, a + w); R(a, b - w, b, b); R(a, a, a + w, b); R(b - w, a, b, b)
    if 'outer' not in parts:
        return
    if variant in ('L1', 'L2'):
        # corner brackets
        for cx, sx in ((0, 1), (S, -1)):
            for cy, sy in ((0, 1), (S, -1)):
                xa, xb = sorted((cx, cx + sx * m)); ya, yb = sorted((cy, cy + sy * w))
                R(xa, ya, xb, yb)
                xa, xb = sorted((cx, cx + sx * w)); ya, yb = sorted((cy, cy + sy * m))
                R(xa, ya, xb, yb)
        if variant == 'L1':
            R(2 * m, 0, 3 * m, w); R(2 * m, S - w, 3 * m, S)
            R(0, 2 * m, w, 3 * m); R(S - w, 2 * m, S, 3 * m)
        else:
            t = 0.6 * m
            c0, c1 = 2.5 * m - w / 2, 2.5 * m + w / 2
            R(c0, 0, c1, t); R(c0, S - t, c1, S); R(0, c0, t, c1); R(S - t, c0, S, c1)
    elif variant == 'L3':
        cut = S / (2 + math.sqrt(2))
        pts = [(cut, 0), (S - cut, 0), (S, cut), (S, S - cut), (S - cut, S), (cut, S), (0, S - cut), (0, cut)]
        gap = 0.28 * m
        for k in range(8):
            (ax, ay), (bx, by) = pts[k], pts[(k + 1) % 8]
            L = math.hypot(bx - ax, by - ay)
            ux, uy = (bx - ax) / L, (by - ay) / L
            nx, ny = uy, -ux                                # outward normal (clockwise order)
            p0 = (ax + ux * gap, ay + uy * gap)
            p1 = (bx - ux * gap, by - uy * gap)
            poly = [p0, p1, (p1[0] - nx * w, p1[1] - ny * w), (p0[0] - nx * w, p0[1] - ny * w)]
            d.polygon([((x0 + px) * SS, (y0 + py) * SS) for px, py in poly], fill=fill)


# 15 x 15 px row versions, hand set: 1 = lit
ROW = {
    'L1': ['111000111000111', '100000000000001', '100000000000001', '000111111111000',
           '000100000001000', '000100000001000', '100100000001001', '100100000001001',
           '100100000001001', '000100000001000', '000100000001000', '000111111111000',
           '100000000000001', '100000000000001', '111000111000111'],
    'L2': ['111000010000111', '100000010000001', '100000000000001', '000111111111000',
           '000100000001000', '000100000001000', '000100000001000', '110100000001011',
           '000100000001000', '000100000001000', '000100000001000', '000111111111000',
           '100000000000001', '100000010000001', '111000010000111'],
    'L3': ['000011111110000', '000000000000000', '010000000000010', '100111111111001',
           '100100000001001', '100100000001001', '100100000001001', '100100000001001',
           '100100000001001', '100100000001001', '100100000001001', '100111111111001',
           '010000000000010', '000000000000000', '000011111110000'],
}


def row_img(v, scale=4, col='LIME'):
    im = Image.new('RGB', (15 * scale + 8, 15 * scale + 8), (0, 0, 0))
    d = ImageDraw.Draw(im)
    for j, r in enumerate(ROW[v]):
        for i, b in enumerate(r):
            if b == '1':
                d.rectangle([4 + i * scale, 4 + j * scale, 4 + (i + 1) * scale - 1, 4 + (j + 1) * scale - 1], fill=rgb(col))
    return im


def mask(v, s=1.0, parts=('tile', 'outer')):
    m = M_CARD * s
    w = frame_for(M_CARD) * s
    img = Image.new('L', (W * SS, W * SS), 0)
    strokes(ImageDraw.Draw(img), 233 - 2.5 * m, 233 - 2.5 * m, m, w, v, parts=parts)
    return img


def card(v, stage, s=1.0):
    cv = Canvas()
    clip = circle(232)

    def paint(msk, col):
        cv.img.paste(Image.new('RGB', cv.img.size, rgb(col)), (0, 0), ImageChops.multiply(msk, clip))
    full = Image.new('L', cv.img.size, 255)
    if stage == 'partial':
        paint(full, 'LIME'); paint(mask(v, parts=('tile',)), 'BLACK')
    elif stage == 'solid':
        paint(full, 'LIME'); paint(mask(v), 'BLACK')
    elif stage == 'inverted':
        paint(mask(v), 'LIME')
    elif stage == 'scaled':
        paint(mask(v, s), 'LIME')
    elif stage == 'impact':
        paint(full, 'LIME'); paint(mask(v, s), 'BLACK')
    return finish(cv)


if __name__ == '__main__':
    f = ImageFont.truetype('fonts/MonoR.otf', 13)
    fb = ImageFont.truetype('fonts/MonoB.otf', 18)
    sc = 0.4
    cw = int(W * sc)
    stages = [('partial', 1, 'PARTIAL: THE TILE'), ('solid', 1, 'SOLID'), ('inverted', 1, 'INVERTED'),
              ('scaled', 1.9, 'SCALED 1.9X'), ('impact', 1.9, 'IMPACT 1.9X'), ('impact', 2.3, 'IMPACT 2.3X')]
    names = {'L1': 'L1  EDGE LINES ALONG THE SIDES', 'L2': 'L2  EDGE TICKS POINTING IN',
             'L3': 'L3  A BROKEN OCTAGON AROUND THE TILE'}
    x_row = 12
    xs0 = 130
    sh = Image.new('RGB', (xs0 + len(stages) * (cw + 10) + 12, 3 * (cw + 56) + 40), (20, 20, 22))
    d = ImageDraw.Draw(sh)
    d.text((x_row, 12), 'ROW, 15 PX (x4)', font=f, fill=(210, 211, 214))
    for i, (_, _, h) in enumerate(stages):
        d.text((xs0 + i * (cw + 10), 12), h, font=f, fill=(210, 211, 214))
    for j, v in enumerate(('L1', 'L2', 'L3')):
        y = 36 + j * (cw + 56)
        d.text((x_row, y), names[v], font=fb, fill=(210, 211, 214))
        sh.paste(row_img(v), (x_row, y + 40))
        for i, (st, s, _) in enumerate(stages):
            sh.paste(card(v, st, s).resize((cw, cw), Image.LANCZOS), (xs0 + i * (cw + 10), y + 28))
    sh.save('out/marks-frame.png')


# ---- L3 staggered (25 Sep, owner): edges further out than the diagonals, and shorter -----------------
# All in px at 1x, about the centre. tile: half-size of the outline square; rd, re: apothems of the
# diagonal and edge strokes' outer faces; ld, le: their lengths. Stroke w = 11 (frame_for(42)).
STAGGER = {
    'L3a': dict(tile=52, rd=96, re=118, ld=64, le=40),
    'L3b': dict(tile=52, rd=96, re=128, ld=64, le=30),
    'L3c': dict(tile=56, rd=102, re=118, ld=72, le=40),
}


def stagger_mask(key, s=1.0, parts=('tile', 'outer'), w=11):
    p = STAGGER[key]
    img = Image.new('L', (W * SS, W * SS), 0)
    d = ImageDraw.Draw(img)
    c = 233.0
    w = w * s

    def poly(pts):
        d.polygon([((c + x) * SS, (c + y) * SS) for x, y in pts], fill=255)

    def rect(x0, y0, x1, y1):
        poly([(x0, y0), (x1, y0), (x1, y1), (x0, y1)])
    t = p['tile'] * s
    if 'tile' in parts:
        rect(-t, -t, t, -t + w); rect(-t, t - w, t, t); rect(-t, -t, -t + w, t); rect(t - w, -t, t, t)
    if 'outer' not in parts:
        return img
    re, le = p['re'] * s, p['le'] * s / 2
    rect(-le, -re, le, -re + w); rect(-le, re - w, le, re); rect(-re, -le, -re + w, le); rect(re - w, -le, re, le)
    rd, ld = p['rd'] * s, p['ld'] * s / 2
    for ang in (45, 135, 225, 315):
        a = math.radians(ang)
        ux, uy = math.cos(a), math.sin(a)          # outward normal of the diagonal stroke
        tx, ty = -uy, ux                           # along the stroke
        o = (ux * rd, uy * rd)
        i = (ux * (rd - w), uy * (rd - w))
        poly([(o[0] + tx * ld, o[1] + ty * ld), (o[0] - tx * ld, o[1] - ty * ld),
              (i[0] - tx * ld, i[1] - ty * ld), (i[0] + tx * ld, i[1] + ty * ld)])
    return img


def stagger_row():
    g = [[0] * 15 for _ in range(15)]
    for k in range(6, 9):
        g[0][k] = g[14][k] = g[k][0] = g[k][14] = 1
    for (x, y) in ((1, 3), (2, 2), (3, 1)):
        for (X, Y) in ((x, y), (14 - x, y), (x, 14 - y), (14 - x, 14 - y)):
            g[Y][X] = 1
    for k in range(4, 11):
        g[4][k] = g[10][k] = g[k][4] = g[k][10] = 1
    return [''.join(map(str, r)) for r in g]


ROW['L3s'] = stagger_row()


def card_stagger(key, stage, s=1.0):
    cv = Canvas()
    clip = circle(232)

    def paint(msk, col):
        cv.img.paste(Image.new('RGB', cv.img.size, rgb(col)), (0, 0), ImageChops.multiply(msk, clip))
    full = Image.new('L', cv.img.size, 255)
    if stage == 'partial':
        paint(full, 'LIME'); paint(stagger_mask(key, parts=('tile',)), 'BLACK')
    elif stage == 'solid':
        paint(full, 'LIME'); paint(stagger_mask(key), 'BLACK')
    elif stage == 'inverted':
        paint(stagger_mask(key), 'LIME')
    elif stage == 'scaled':
        paint(stagger_mask(key, s), 'LIME')
    elif stage == 'impact':
        paint(full, 'LIME'); paint(stagger_mask(key, s), 'BLACK')
    return finish(cv)


def board_stagger(path):
    f = ImageFont.truetype('fonts/MonoR.otf', 13)
    fb = ImageFont.truetype('fonts/MonoB.otf', 18)
    sc = 0.4
    cw = int(W * sc)
    stages = [('solid', 1, 'SOLID'), ('inverted', 1, 'INVERTED'), ('scaled', 1.9, 'SCALED 1.9X'),
              ('impact', 1.9, 'IMPACT 1.9X')]
    rows = [('L1', 'L1  AS BEFORE, FOR REFERENCE', None),
            ('L3', 'L3  AS BEFORE', None),
            ('L3a', 'L3a  EDGES 22 PX OUT FROM THE DIAGONALS, 40 LONG', STAGGER['L3a']),
            ('L3b', 'L3b  EDGES 32 PX OUT, 30 LONG', STAGGER['L3b']),
            ('L3c', 'L3c  LESS STAGGER: 16 PX OUT, 40 LONG, LARGER TILE', STAGGER['L3c'])]
    xs0 = 130
    sh = Image.new('RGB', (xs0 + len(stages) * (cw + 10) + 12, len(rows) * (cw + 56) + 40), (20, 20, 22))
    d = ImageDraw.Draw(sh)
    d.text((12, 12), 'ROW, 15 PX (x4)', font=f, fill=(210, 211, 214))
    for i, (_, _, h) in enumerate(stages):
        d.text((xs0 + i * (cw + 10), 12), h, font=f, fill=(210, 211, 214))
    for j, (k, name, p) in enumerate(rows):
        y = 36 + j * (cw + 56)
        d.text((12, y), name, font=fb, fill=(210, 211, 214))
        sh.paste(row_img('L3s' if p else k), (12, y + 40))
        for i, (st, s, _) in enumerate(stages):
            im = card_stagger(k, st, s) if p else card(k, st, s)
            sh.paste(im.resize((cw, cw), Image.LANCZOS), (xs0 + i * (cw + 10), y + 28))
    sh.save(path)


if __name__ == '__main__':
    board_stagger('out/marks-stagger.png')


# ---- hatched centre (25 Sep, owner) --------------------------------------------------------------
import numpy as np
HATCH = dict(pitch=12, stripe=5, inset=5)     # 45 deg, rising to the right, as the identity's hatch


def hatch_square(img, half_inner, s=1.0, **kw):
    """Hatch the square |x|,|y| < half_inner - inset about the centre into an L mask (in place)."""
    h = dict(HATCH, **kw)
    a = (half_inner - h['inset']) * s
    arr = np.array(img)
    n = arr.shape[0]
    ys, xs = np.mgrid[0:n, 0:n]
    x = xs / SS - 233.0
    y = ys / SS - 233.0
    inside = (np.abs(x) < a) & (np.abs(y) < a)
    d = ((x + y) / np.sqrt(2)) % (h['pitch'] * s)
    arr[inside & (d < h['stripe'] * s)] = 255
    return Image.fromarray(arr)


def hatched_mask(key, s=1.0, parts=('tile', 'outer')):
    if key == 'L1':
        m = mask('L1', s, parts)
        return hatch_square(m, 1.5 * M_CARD - frame_for(M_CARD), s)
    m = stagger_mask(key, s, parts)
    return hatch_square(m, STAGGER[key]['tile'] - 11, s)


def hatched_row(key):
    rows = [list(r) for r in ROW['L1' if key == 'L1' else 'L3s']]
    lo, hi = (4, 10) if key == 'L1' else (5, 9)      # the tile's inside, px
    for yy in range(lo, hi + 1):
        for xx in range(lo, hi + 1):
            if (xx + yy) % 3 == 0:
                rows[yy][xx] = '1'
    return [''.join(r) for r in rows]


def card_hatched(key, stage, s=1.0):
    cv = Canvas()
    clip = circle(232)

    def paint(msk, col):
        cv.img.paste(Image.new('RGB', cv.img.size, rgb(col)), (0, 0), ImageChops.multiply(msk, clip))
    full = Image.new('L', cv.img.size, 255)
    if stage == 'solid':
        paint(full, 'LIME'); paint(hatched_mask(key), 'BLACK')
    elif stage == 'inverted':
        paint(hatched_mask(key), 'LIME')
    elif stage == 'scaled':
        paint(hatched_mask(key, s), 'LIME')
    elif stage == 'impact':
        paint(full, 'LIME'); paint(hatched_mask(key, s), 'BLACK')
    return finish(cv)


def board_hatched(path):
    f = ImageFont.truetype('fonts/MonoR.otf', 13)
    fb = ImageFont.truetype('fonts/MonoB.otf', 18)
    sc = 0.4
    cw = int(W * sc)
    stages = [('solid', 1, 'SOLID'), ('inverted', 1, 'INVERTED'), ('scaled', 1.9, 'SCALED 1.9X'),
              ('impact', 1.9, 'IMPACT 1.9X')]
    rows = [('L1', 'L1  HATCHED CENTRE'), ('L3a', 'L3a  HATCHED CENTRE')]
    xs0 = 130
    sh = Image.new('RGB', (xs0 + len(stages) * (cw + 10) + 12, len(rows) * (cw + 56) + 40), (20, 20, 22))
    d = ImageDraw.Draw(sh)
    d.text((12, 12), 'ROW, 15 PX (x4)', font=f, fill=(210, 211, 214))
    for i, (_, _, h) in enumerate(stages):
        d.text((xs0 + i * (cw + 10), 12), h, font=f, fill=(210, 211, 214))
    for j, (k, name) in enumerate(rows):
        y = 36 + j * (cw + 56)
        d.text((12, y), name, font=fb, fill=(210, 211, 214))
        ROW[k + 'h'] = hatched_row(k)
        sh.paste(row_img(k + 'h'), (12, y + 40))
        for i, (st, s, _) in enumerate(stages):
            sh.paste(card_hatched(k, st, s).resize((cw, cw), Image.LANCZOS), (xs0 + i * (cw + 10), y + 28))
    sh.save(path)


# ---- corners out (25 Sep, owner, corrected): the diagonals further out than the edges, and shorter ----
STAGGER.update({
    'L3d': dict(tile=52, rd=126, re=90, ld=40, le=48),
    'L3e': dict(tile=52, rd=136, re=86, ld=30, le=56),
    'L3f': dict(tile=56, rd=124, re=98, ld=46, le=46),
})


def corners_out_row(hatch=False):
    g = [[0] * 15 for _ in range(15)]
    for (x, y) in ((0, 2), (1, 1), (2, 0)):                    # corners: 3 px diagonals at the extremes
        for (X, Y) in ((x, y), (14 - x, y), (x, 14 - y), (14 - x, 14 - y)):
            g[Y][X] = 1
    for k in range(6, 9):                                      # edges: 3 px, two in from the border
        g[2][k] = g[12][k] = g[k][2] = g[k][12] = 1
    for k in range(5, 10):                                     # the tile, 5 x 5
        g[5][k] = g[9][k] = g[k][5] = g[k][9] = 1
    if hatch:
        for yy in range(6, 9):
            for xx in range(6, 9):
                if (xx + yy) % 2 == 0:
                    g[yy][xx] = 1
    return [''.join(map(str, r)) for r in g]


ROW['L3o'] = corners_out_row()
ROW['L3oh'] = corners_out_row(True)


def board_corners(path, pop=1.8):
    f = ImageFont.truetype('fonts/MonoR.otf', 13)
    fb = ImageFont.truetype('fonts/MonoB.otf', 18)
    sc = 0.4
    cw = int(W * sc)
    cols = [('solid', 1, False, 'SOLID'), ('inverted', 1, False, 'INVERTED'),
            ('impact', pop, False, 'IMPACT %.1fX' % pop), ('solid', 1, True, 'SOLID, HATCHED'),
            ('impact', pop, True, 'IMPACT, HATCHED')]
    rows = [('L3d', 'L3d  CORNERS 36 PX OUT FROM THE EDGES, 40 LONG'),
            ('L3e', 'L3e  CORNERS 50 PX OUT, 30 LONG'),
            ('L3f', 'L3f  LESS STAGGER: 26 PX OUT, 46 LONG, LARGER TILE')]
    xs0 = 130
    sh = Image.new('RGB', (xs0 + len(cols) * (cw + 10) + 12, len(rows) * (cw + 56) + 40), (20, 20, 22))
    d = ImageDraw.Draw(sh)
    d.text((12, 12), 'ROW, 15 PX (x4)', font=f, fill=(210, 211, 214))
    for i, (_, _, _, h) in enumerate(cols):
        d.text((xs0 + i * (cw + 10), 12), h, font=f, fill=(210, 211, 214))
    for j, (k, name) in enumerate(rows):
        y = 36 + j * (cw + 56)
        d.text((12, y), name, font=fb, fill=(210, 211, 214))
        sh.paste(row_img('L3o'), (12, y + 40))
        sh.paste(row_img('L3oh'), (12, y + 40 + 72))
        for i, (st, s, hat, _) in enumerate(cols):
            im = card_hatched(k, st, s) if hat else card_stagger(k, st, s)
            sh.paste(im.resize((cw, cw), Image.LANCZOS), (xs0 + i * (cw + 10), y + 28))
    sh.save(path)
