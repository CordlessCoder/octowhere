"""OCTOWHERE marks on the 5 x 5 module grid.

1. Every mark with the octagon's symmetry (D4: the four rotations and four reflections of the
   square grid): six cell orbits, so 64 patterns. Each is checked against the device's icons in
   use: the same glyph, or near (few modules apart), would make the mark read as a function.
2. A shortlist, each drawn at the microtext row's size, at icon size next to its nearest icon,
   and on the logo card (solid, and the 2.3x impact frame).
"""
import lib
lib.RECORD = False
from lib import *
from PIL import ImageChops
from identity import finish, circle

IN_USE = {   # every 5x5 glyph the device shows today
    'GNSS': '00100 01010 10101 01010 00100', 'RTC': '11111 10001 10101 10001 11111',
    'STOPPED': '01010 01010 01010 01010 01010', 'NO ZONE': '01110 10001 00110 00000 00100',
    'NO DATA': '11100 11000 00100 00011 00111', 'HEADING': '00100 01110 10101 00100 00100',
    'CALIBRATING': '01110 10001 10000 10001 01110', 'INTERFERENCE': '10101 01110 11011 01110 10101',
    'TOP EDGE': '11111 00100 00100 00100 00100', 'ZONE': '11011 10001 00100 10001 11011',
    'BRIGHTNESS': '00001 00011 00111 01111 11111', 'DEVICE': '00100 00000 01100 00100 01110',
    'CLEAR': '10001 01010 00100 01010 10001', 'BATTERY': '01110 11111 10001 11111 11111',
    'TOUCH': '00100 00100 11011 00100 00100', 'MOTION': '10000 10000 10000 10000 11111',
    'MAGNET': '11011 11011 11011 11111 01110', 'TIMEOUT': '11111 01110 00100 01110 11111',
    'ALWAYS ON': '00000 01110 11011 01110 00000', 'GLOBE': '01110 10101 11111 10101 01110',
}

# D4 orbits of the 5x5 grid
ORBITS = {
    'corner': [(0, 0), (0, 4), (4, 0), (4, 4)],
    'edge1': [(0, 1), (0, 3), (1, 0), (3, 0), (4, 1), (4, 3), (1, 4), (3, 4)],
    'mid': [(0, 2), (2, 0), (4, 2), (2, 4)],
    'diag': [(1, 1), (1, 3), (3, 1), (3, 3)],
    'inner': [(1, 2), (2, 1), (3, 2), (2, 3)],
    'centre': [(2, 2)],
}
ORDER = ['corner', 'edge1', 'mid', 'diag', 'inner', 'centre']


def grid_of(rows):
    return [[c == '1' for c in r] for r in rows.split()]


def rows_of(g):
    return ' '.join(''.join('1' if v else '0' for v in r) for r in g)


def dist(a, b):
    return sum(x != y for x, y in zip(a.replace(' ', ''), b.replace(' ', '')))


def nearest(rows):
    name = min(IN_USE, key=lambda k: dist(rows, IN_USE[k]))
    return name, dist(rows, IN_USE[name])


def d4_all():
    out = []
    for bits in range(64):
        g = [[False] * 5 for _ in range(5)]
        for i, o in enumerate(ORDER):
            if bits >> (5 - i) & 1:
                for (r, c) in ORBITS[o]:
                    g[r][c] = True
        out.append(rows_of(g))
    return out


def draw_glyph(cv, rows, x, y, m, col):
    for j, r in enumerate(rows.split()):
        for i, b in enumerate(r):
            if b == '1':
                cv.rect(x + i * m, y + j * m, x + (i + 1) * m, y + (j + 1) * m, col)


def glyph_img(rows, m, col='WHITE', bg=(0, 0, 0), pad=6, frame=None):
    s = 5 * m + 2 * pad
    im = Image.new('RGB', (s, s), bg)
    d = ImageDraw.Draw(im)
    if frame:
        d.rectangle([0, 0, s - 1, s - 1], outline=rgb(frame), width=max(2, int(m / 4 + 0.5)))
    for j, r in enumerate(rows.split()):
        for i, b in enumerate(r):
            if b == '1':
                d.rectangle([pad + i * m, pad + j * m, pad + (i + 1) * m - 1, pad + (j + 1) * m - 1], fill=rgb(col))
    return im


def card(rows, stage, mod=38, pop=2.3):
    cv = Canvas()
    m = Image.new('L', cv.img.size, 0)
    d = ImageDraw.Draw(m)
    mm = mod * (pop if stage == 'impact' else 1)
    x0 = 233 - 2.5 * mm
    for j, r in enumerate(rows.split()):
        for i, b in enumerate(r):
            if b == '1':
                d.rectangle([(x0 + i * mm) * SS, (x0 + j * mm) * SS, (x0 + (i + 1) * mm) * SS - 1,
                             (x0 + (j + 1) * mm) * SS - 1], fill=255)
    clip = circle(232)
    field = 'LIME'
    mark = 'BLACK'
    cv.img.paste(Image.new('RGB', cv.img.size, rgb(field)), (0, 0), clip)
    cv.img.paste(Image.new('RGB', cv.img.size, rgb(mark)), (0, 0), ImageChops.multiply(m, clip))
    return finish(cv)


SHORT = [
    ('A', '01110 10001 10101 10001 01110', 'M3: octagon ring, centre module. A position inside the octagon'),
    ('B', '01110 11111 11011 11111 01110', 'Solid octagon, a pinhole at the centre. The same idea in negative'),
    ('C', '01110 11101 11111 11111 01110', 'Solid octagon, the hole up and right: a position, not a centre'),
    ('D', '01010 11111 11111 11111 01110', 'Solid octagon with a north notch'),
    ('E', '01110 11011 10001 11011 01110', "Ring with four inner teeth: the glyph already in the identity's row"),
    ('F', '10001 01110 01010 01110 10001', 'Ring with four diagonal spokes: eight directions'),
    ('G', '01110 11111 11111 11111 01110', 'Solid octagon. Plain, strongest at impact'),
    ('H', '11011 10101 01110 10101 11011', 'Octagon frame broken into corners around a diamond'),
]


def catalogue(path):
    pats = d4_all()
    f = ImageFont.truetype('fonts/MonoR.otf', 12)
    fb = ImageFont.truetype('fonts/MonoB.otf', 16)
    cols, cw, ch = 8, 150, 136
    sh = Image.new('RGB', (cols * cw + 24, 8 * ch + 60), (20, 20, 22))
    d = ImageDraw.Draw(sh)
    d.text((12, 14), 'EVERY 5 x 5 MARK WITH THE OCTAGON\'S SYMMETRY (64). RED: AN ICON IN USE. ORANGE: 1-3 MODULES FROM ONE',
           font=fb, fill=(210, 211, 214))
    short = {r: k for k, r, _ in SHORT}
    for n, rows in enumerate(pats):
        x, y = 12 + (n % cols) * cw, 48 + (n // cols) * ch
        name, dd = nearest(rows)
        col = 'RED' if dd == 0 else ('ORANGE' if dd <= 3 else 'WHITE')
        g = glyph_img(rows, 14, col=col, pad=8)
        sh.paste(g, (x, y))
        label = '%02d' % n
        if dd <= 3:
            label += ' %s%s' % (name, '' if dd == 0 else ' +%d' % dd)
        d.text((x, y + 92), label, font=f, fill=rgb(col) if dd <= 3 else (136, 142, 152))
        if rows in short:
            d.text((x, y + 108), 'SHORTLIST %s' % short[rows], font=f, fill=rgb('LIME'))
    sh.save(path)


def shortlist(path):
    f = ImageFont.truetype('fonts/MonoR.otf', 13)
    fb = ImageFont.truetype('fonts/MonoB.otf', 18)
    sc = 0.42
    cw = int(466 * sc)
    rh = cw + 20
    heads = ['', 'ROW, 3 PX', 'ICON, 48 PX', 'NEAREST ICON', 'CARD, SOLID', 'CARD, IMPACT 2.3X']
    xs = [12, 330, 420, 520, 620, 620 + cw + 12]
    sh = Image.new('RGB', (xs[-1] + cw + 12, len(SHORT) * rh + 50), (20, 20, 22))
    d = ImageDraw.Draw(sh)
    for x, h in zip(xs, heads):
        d.text((x, 14), h, font=f, fill=(210, 211, 214))
    for k, (key, rows, idea) in enumerate(SHORT):
        y = 40 + k * rh
        name, dd = nearest(rows)
        d.text((12, y), key, font=fb, fill=(210, 211, 214))
        d.text((40, y + 2), rows, font=f, fill=(136, 142, 152))
        yy, line = y + 28, ''
        for w in idea.split():
            if len(line + ' ' + w) > 38:
                d.text((12, yy), line, font=f, fill=(210, 211, 214)); yy += 17; line = w
            else:
                line = (line + ' ' + w).strip()
        d.text((12, yy), line, font=f, fill=(210, 211, 214))
        warn = 'SAME AS %s' % name if dd == 0 else '%d MODULES FROM %s' % (dd, name)
        d.text((12, yy + 24), warn, font=f, fill=rgb('ORANGE') if dd <= 4 else (136, 142, 152))
        sh.paste(glyph_img(rows, 3, col='LIME', pad=4), (xs[1], y + 30))
        sh.paste(glyph_img(rows, 8, col='WHITE', pad=4, frame='WHITE'), (xs[2], y + 30))
        sh.paste(glyph_img(IN_USE[name], 8, col='GRAY', pad=4, frame='GRAY'), (xs[3], y + 30))
        d.text((xs[3], y + 82), name, font=f, fill=(136, 142, 152))
        sh.paste(card(rows, 'solid').resize((cw, cw), Image.LANCZOS), (xs[4], y))
        sh.paste(card(rows, 'impact').resize((cw, cw), Image.LANCZOS), (xs[5], y))
    sh.save(path)


if __name__ == '__main__':
    catalogue('out/marks5-catalogue.png')
    shortlist('out/marks5-shortlist.png')
    for key, rows, _ in SHORT:
        print(key, rows, nearest(rows))
