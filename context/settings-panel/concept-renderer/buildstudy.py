"""Icon build orders compared, per glyph, one frame per 20 ms."""
from lib import *
from face import SYM

TILES = {'gnss': 'BLUE', 'rtc': 'GRAY', 'stopped': 'ORANGE', 'nozone': 'WHITE'}


def modules(sym):
    return [(i, j) for j, row in enumerate(sym.split()) for i, b in enumerate(row) if b == '1']


def steps(sym, order):
    """List of module sets, one per frame, ending complete."""
    ms = modules(sym)
    if order == 'module':      # one module per frame, reading order
        return [set(ms[:k]) for k in range(0, len(ms) + 1)]
    if order == 'row':         # one row per frame, top down: always 5 frames
        return [set(m for m in ms if m[1] < k) for k in range(0, 6)]
    if order == 'centre':      # rings from the centre out: always 3 frames
        return [set(m for m in ms if max(abs(m[0] - 2), abs(m[1] - 2)) < k) for k in range(0, 4)]


def draw(sym, tile, shown, scale=0.6):
    cv = Canvas()
    x, y, m, p = 185, 185, 16, 8
    cv.rect(x, y, x + 96, y + 96, tile)
    for (i, j) in shown:
        cv.rect(x + p + m * i, y + p + m * j, x + p + m * (i + 1), y + p + m * (j + 1), 'BLACK')
    im = cv.img.resize((466, 466), Image.BOX).crop((175, 175, 291, 291))
    return im


f = ImageFont.truetype('fonts/MonoR.otf', 14)
rows = []
for order in ('module', 'row', 'centre'):
    for g, tile in TILES.items():
        rows.append((order, g, [draw(SYM[g], tile, s) for s in steps(SYM[g], order)]))
maxn = max(len(r[2]) for r in rows)
cw = 116 + 8
sh = Image.new('RGB', (190 + maxn * cw, len(rows) * (cw + 4) + 30), (28, 28, 28))
d = ImageDraw.Draw(sh)
for k in range(maxn):
    d.text((190 + k * cw, 6), '%d MS' % (k * 20), font=f, fill=(136, 142, 152))
for r, (order, g, ims) in enumerate(rows):
    y = 28 + r * (cw + 4)
    d.text((8, y + 50), '%s  %s' % (order.upper(), g.upper()), font=f, fill=(210, 211, 214))
    for k, im in enumerate(ims):
        sh.paste(im, (190 + k * cw, y))
sh.save('out/study-icon-build-orders.png')
print(sh.size)
