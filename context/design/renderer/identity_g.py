"""Identity G-J: four more directions after the owner's references (round 3, second batch).

G STAND BY     full PURPLE field, the band as an outlined box with checker squares, contour octagons
H SLIDING ROWS outlined rows of zone names sliding; a LIME slab lands on the device's zone, then the word
I GRID FILL    block-noise blobs, then a non-uniform grid fills cell by cell; rotated word; progress bar
J TRIANGLES    a field of half-square triangles scrolls, flips in waves and resolves into an octagon

Shared: LIME and PURPLE only (the identity's two tokens), WHITE for the band; microtext is real
data (fixture values). Every direction hands over the same way: whatever sits on the band rows
(198-317) becomes the clock band, then the clock's entry runs.
"""
import math
import random
import lib
lib.RECORD = False
from lib import *
from PIL import ImageChops, ImageFilter
from identity import circle, text_mask, finish, band_word, BAND
from identity_f import pix_digits, glyph, barcode
from anim import save_gif, DT

W = 466
_CL = {}


def clip(r=229.0):
    if r not in _CL:
        _CL[r] = circle(r)
    return _CL[r]


def mix(a, b, k):
    a, b = rgb(a), rgb(b)
    return tuple(int(round(x * (1 - k) + y * k)) for x, y in zip(a, b))


def draw(cv):
    return ImageDraw.Draw(cv.img)


def put(cv, m, col, x, y):
    """Paste a 4x mask at panel (x, y), top-left."""
    cv.img.paste(Image.new('RGB', m.size, rgb(col)), (int(round(x * SS)), int(round(y * SS))), m)


def putc(cv, m, col, cx, cy):
    put(cv, m, col, cx - m.width / SS / 2, cy - m.height / SS / 2)


def field(cv, col):
    cv.img.paste(Image.new('RGB', cv.img.size, rgb(col)), (0, 0), clip())


def outline(m, w=1.5):
    k = int(w * SS) * 2 + 1
    return ImageChops.subtract(m, m.filter(ImageFilter.MinFilter(k)))


def frame_rect(cv, x0, y0, x1, y1, col, w=1):
    cv.rect(x0, y0, x1, y0 + w, col)
    cv.rect(x0, y1 - w, x1, y1, col)
    cv.rect(x0, y0, x0 + w, y1, col)
    cv.rect(x1 - w, y0, x1, y1, col)


_TM = {}


def tm(s, face, px, sx=1.0, sy=1.0):
    key = (s, face, px, sx, sy)
    if key not in _TM:
        _TM[key] = text_mask(s, face, px, sx=sx, sy=sy)
    return _TM[key]


def reveal(cv, s, face, px, col, cx, cy, n, sy=1.0, block_col=None):
    """Cell reveal: letters before n drawn, letter n a solid block, the rest empty."""
    full = tm(s, face, px, sy=sy)
    left = cx - full.width / SS / 2
    top = cy - full.height / SS / 2
    if n >= len(s):
        put(cv, full, col, left, top)
        return
    if n > 0:
        put(cv, tm(s[:n], face, px, sy=sy), col, left, top)
    w0 = tm(s[:n], face, px, sy=sy).width / SS + (3 if n else 0) if n else 0
    w1 = tm(s[:n + 1], face, px, sy=sy).width / SS
    cv.rect(left + w0, top, left + w1, top + full.height / SS, block_col or col)


def hatch(cv, x0, y0, x1, y1, col, phase=0, pitch=8, w=4):
    m = Image.new('L', (int((x1 - x0) * SS), int((y1 - y0) * SS)), 0)
    d = ImageDraw.Draw(m)
    h = y1 - y0
    x = -h - pitch + (phase % pitch)
    while x < x1 - x0 + h:
        d.polygon([((x) * SS, h * SS), ((x + w) * SS, h * SS), ((x + w + h) * SS, 0), ((x + h) * SS, 0)], fill=255)
        x += pitch
    put(cv, m, col, x0, y0)


def blob_noise(seed, cell=20, density=0.46, iters=4):
    """Big pixel blobs: seeded cells smoothed by a majority rule, so they clump like the reference."""
    rnd = random.Random(seed)
    n = W // cell + 2
    g = [[1 if rnd.random() < density else 0 for _ in range(n)] for _ in range(n)]
    for _ in range(iters):
        h = [[0] * n for _ in range(n)]
        for j in range(n):
            for i in range(n):
                s = sum(g[(j + b) % n][(i + a) % n] for a in (-1, 0, 1) for b in (-1, 0, 1))
                h[j][i] = 1 if s >= 5 else 0
        g = h
    return g


def paint_blobs(cv, g, col, cell=20, dx=0, dy=0, region=None):
    d = draw(cv)
    for j, row in enumerate(g):
        for i, v in enumerate(row):
            if v:
                x, y = i * cell + dx - cell, j * cell + dy - cell
                if region and not region(x, y):
                    continue
                d.rectangle([x * SS, y * SS, (x + cell) * SS - 1, (y + cell) * SS - 1], fill=rgb(col))


def cross(cv, x, y, s, col, w=1):
    cv.rect(x - s, y, x + s + 1, y + w, col)
    cv.rect(x, y - s, x + w, y + s + 1, col)


def xmark(cv, x, y, s, col):
    d = draw(cv)
    d.line([((x - s) * SS, (y - s) * SS), ((x + s) * SS, (y + s) * SS)], fill=rgb(col), width=int(1.5 * SS))
    d.line([((x - s) * SS, (y + s) * SS), ((x + s) * SS, (y - s) * SS)], fill=rgb(col), width=int(1.5 * SS))


def vlabel(cv, s, face, px, col, cx, cy):
    m = tm(s, face, px).rotate(90, expand=True)
    putc(cv, m, col, cx, cy)


# =================================================================================================
# G  STAND BY
# =================================================================================================
G_PX, G_SY, G_CY = 36, 1.5, 258
G_FAINT = mix('PURPLE', 'WHITE', 0.30)
G_CROSSES = [(p[0], p[1]) for p in [(96, 120), (150, 88), (318, 104), (372, 150), (112, 356), (170, 402),
                                     (300, 372), (352, 340), (402, 170), (64, 170), (233, 70), (268, 410)]]
G_TICKS = sorted(random.Random(7).sample(range(96, 372, 4), 22))


def g_contours(cv, k, phase):
    d = draw(cv)
    n = 13
    for i in range(int(n * k)):
        R = 34 + i * 15
        ox = 16 * math.sin(phase + i * 0.5)
        oy = -3 * i + 18
        cx, cy = 233 + ox, 233 + oy
        pts = [((cx + R * math.cos(math.radians(22.5 + 45 * j))) * SS,
                (cy + R * math.sin(math.radians(22.5 + 45 * j))) * SS) for j in range(8)]
        d.line(pts + [pts[0]], fill=G_FAINT, width=SS)


G_MICRO = 'g'           # 'g' (the first version), 'in' (F's row inside the band), 'under' (F's row under it)
G_BG = 'field'          # 'field' (the first version), 'tri', 'halftone', 'blobs'


def g_bg(cv, t):
    """The background: black with a PURPLE pattern, quiet zone rows 176-340 kept black."""
    if G_BG == 'tri':
        off = (t * 0.25) % TM_
        for r in range(-1, 9):
            for c in range(-1, 9):
                kind = J_FIELD.get((c, r))
                if kind:
                    tri(cv, TX0 + c * TM_ + off, TY0 + r * TM_ + off, kind, 'PURPLE')
    elif G_BG == 'halftone':
        rnd = random.Random(4)
        appear = min(1.0, (t + 20) / 160)
        for y in range(12, 456, 8):
            for x in range(12, 456, 8):
                v, kind = rnd.random(), rnd.random()
                dx, dy = x + 3 - 233, y + 3 - 233
                r = math.hypot(dx, dy)
                if r > 226:
                    continue
                ang = math.atan2(dy, dx)
                dens = max(0.0, (r - 70) / 150) * (0.5 + 0.5 * math.cos(ang - 0.8 - t / 400)) * 1.25 * appear
                if v < dens:
                    if kind < 0.6:
                        cv.rect(x, y, x + 6, y + 6, 'PURPLE'); cv.rect(x + 2, y + 2, x + 4, y + 4, 'BLACK')
                    else:
                        cv.rect(x + 1, y + 1, x + 5, y + 5, 'PURPLE')
    elif G_BG == 'blobs':
        paint_blobs(cv, blob_noise(21 + t // 80, cell=20, density=0.5, iters=4), 'PURPLE', cell=20)
    cv.rect(0, 176, W, 362 if G_MICRO == 'under' else 341, 'BLACK')


def g_frow(cv, top, n, full, lines=None):
    """F's microtext row. full: F's own row; else shortened to fit between the checkers.
    n: elements shown, in order (the row types in element by element)."""
    if full:
        items = [('bar',), ('g', '10001 01010 00100 01010 10001'), ('g', '01110 11011 10001 11011 01110'),
                 ('p', '12 07'), ('g', '00100 01010 10101 01010 00100'), ('t',) + (lines or ('OCTOWHERE 0.1.0', 'SELF TEST 6/6 OK'))]
    else:
        items = [('bar',), ('g', '01110 11011 10001 11011 01110'), ('p', '12 07'),
                 ('g', '00100 01010 10101 01010 00100'), ('t', 'SELF TEST 6/6', 'TZDATA 2026D')]

    def run(cv, x, n):
        for i, it in enumerate(items[:n]):
            if it[0] == 'bar':
                x = barcode(cv, '0.1.0', x, top, col='LIME') + 10
            elif it[0] == 'g':
                x = glyph(cv, it[1], x, top, col='LIME') + 11
            elif it[0] == 'p':
                x = pix_digits(cv, it[1], x, top, col='LIME') + 10
            else:
                cv.text(it[1], 'mono', 14, 'LIME', x=x, y=top - 2)
                cv.text(it[2], 'mono', 14, 'LIME', x=x, y=top + 14)
                x += font('mono', 14).getlength(max(it[1], it[2], key=len)) / SS
        return x
    end = run(Canvas(), 0, len(items))
    run(cv, round(233 - end / 2), n)
    return end


def g_checker(cv, x, inv=False):
    S, c = 104, 52
    y = 206
    bgc = 'PURPLE' if G_BG == 'field' else 'BLACK'
    a, b = ('WHITE', bgc) if not inv else (bgc, 'WHITE')
    cv.rect(x, y, x + c, y + c, a)
    cv.rect(x + c, y + c, x + S, y + S, a)
    cv.rect(x + c, y, x + S, y + c, b)
    cv.rect(x, y + c, x + c, y + S, b)
    frame_rect(cv, x, y, x + S, y + S, 'WHITE', 2)


def g_tag(cv, x, y, s):
    if G_BG != 'field':
        cv.rect(x - 4, y - 4, x + 22, y + 48, 'BLACK')
    frame_rect(cv, x, y, x + 18, y + 44, 'WHITE', 1)
    vlabel(cv, s, 'bold', 11, 'WHITE', x + 9, y + 22)


def g_frame(t):
    cv = Canvas()
    if t < 60:
        if not (20 <= t < 40):
            field(cv, 'PURPLE') if G_BG == 'field' else g_bg(cv, t)
        return finish(cv)
    if G_BG == 'field':
        field(cv, 'PURPLE')
        g_contours(cv, min(1, (t - 60) / 140), t / 500)
    else:
        g_bg(cv, t)
    if t >= 130 and G_BG == 'field':
        for i, (x, y) in enumerate(G_CROSSES[:int(len(G_CROSSES) * min(1, (t - 130) / 100))]):
            cross(cv, x, y, 5, G_FAINT)
    if t >= 160:
        k = max(0.02, min(1, (t - 160) / 80))
        cv.rect(233 - 233 * k, 198, 233 + 233 * k, 200, 'WHITE')
        cv.rect(233 - 233 * k, 315, 233 + 233 * k, 317, 'WHITE')
        off = [140, 70, 24, 0][min(3, (t - 160) // 20)]
        inv = 460 <= t < 500
        g_checker(cv, -32 - off, inv)
        g_checker(cv, 394 + off, inv)
    if t >= 240:                                         # ticks and microtext
        nt = int(len(G_TICKS) * min(1, (t - 240) / 60))
        for i, x in enumerate(G_TICKS[:nt]):
            h = 6 if i % 3 else 3
            cv.rect(x, 190 - h, x + (2 if i % 4 == 0 else 1), 190, 'LIME')
            if G_MICRO == 'g':
                cv.rect(x + 10, 326, x + 11 + (i % 2), 326 + h, 'LIME')
    if t >= 250 and G_MICRO != 'g':
        g_frow(cv, 211 if G_MICRO == 'in' else 330, (t - 250) // 30 + 1, G_MICRO == 'under')
    if t >= 250 and G_MICRO == 'g':
        cv.text('SELF TEST 6/6', 'bold', 12, 'WHITE', x=84, y=208)
        cv.text('TZDATA 2026D', 'mono', 12, 'WHITE', x=194, y=208)
        frame_rect(cv, 300, 205, 352, 222, 'WHITE', 1)
        cv.text('RTC', 'mono', 11, 'WHITE', x=305, y=209)
        cv.rect(336, 209, 347, 218, 'WHITE')
    if t >= 290 and G_MICRO == 'under':
        hatch(cv, 262, 294, 382, 306, 'LIME', phase=(t - 290) * 0.4)
    if t >= 290 and G_MICRO == 'g':
        pix_digits(cv, '24 09', 84, 292, m=2, col='WHITE')
        cv.text('UTC 12:07:42', 'mono', 11, 'WHITE', x=140, y=293)
        hatch(cv, 262, 290, 382, 306, 'LIME', phase=(t - 290) * 0.4)
    if t >= 300:
        g_tag(cv, 92, 138, '0.1.0')
        g_tag(cv, 356, 370 if G_MICRO == 'under' else 336, '6/6')
    if t >= 250:
        n = (t - 250) // 16
        on = not (t in (560, 600))
        if on:
            cy = {'g': G_CY, 'in': 275, 'under': 252}[G_MICRO]
            reveal(cv, 'OCTOWHERE', 'shapiro', G_PX, 'LIME', 233, cy, n, sy=G_SY)
    return finish(cv)


def g_handover(t):
    cv = Canvas()
    if t < 40:
        cv.rect(0, 198, W, 200, 'WHITE')
        cv.rect(0, 315, W, 317, 'WHITE')
        putc(cv, tm('OCTOWHERE', 'shapiro', G_PX, sy=G_SY), 'LIME', 233, {'g': G_CY, 'in': 275, 'under': 252}[G_MICRO])
    elif t < 60:
        cv.img.paste(Image.new('RGB', cv.img.size, rgb('WHITE')), (0, 0),
                     ImageChops.multiply(clip(232), Image.new('L', cv.img.size, 0)))
        cv.rect(0, 198, W, 317, 'WHITE')
        putc(cv, tm('OCTOWHERE', 'shapiro', G_PX, sy=G_SY), 'BLACK', 233, {'g': G_CY, 'in': 275, 'under': 252}[G_MICRO])
    else:
        band_word(cv)
    return finish(cv)


G = dict(key='G', name='STAND BY', frame=g_frame, handover=g_handover, rest=700, hero=680,
         times=[0, 20, 100, 180, 220, 280, 330, 420, 470, 560, 680, 760],
         notes={0: 'The field cuts on in PURPLE', 20: 'Flicker: one frame off (brightness, free)',
                100: 'Contour octagons stack outward, faint, drifting like slices of a scanned shape',
                180: 'The band rules draw out; checker squares slam in from the edges in three steps',
                220: 'Squares land, cropped by the circle',
                280: 'Microtext in the band; the word types in, stretched 1.5x',
                330: 'Tags, hatch bar crawling, date in pixel digits',
                420: 'Rest', 470: 'The checkers swap once',
                560: 'The word flickers: two single frames off',
                680: 'Hold', 760: 'Handover: the field cuts, the box fills WHITE, it is the band'})


# =================================================================================================
# H  SLIDING ROWS
# =================================================================================================
ZONES = ['DUBLIN', 'LISBON', 'LONDON', 'REYKJAVIK', 'OSLO', 'BERLIN', 'ATHENS', 'CAIRO', 'DUBAI',
         'KARACHI', 'DHAKA', 'TOKYO', 'SYDNEY', 'AUCKLAND', 'HONOLULU', 'DENVER', 'CHICAGO', 'LIMA', 'NUUK']
H_PX, H_SY = 26, 1.25
H_ROWS = [-4, -3, -2, 2, 3, 4]
H_PITCH = 46
_HS = {}


def h_strip(k):
    if k not in _HS:
        rnd = random.Random(k * 11 + 3)
        names = ZONES[:]
        rnd.shuffle(names)
        s = '_'.join(names[:6]) + '_'
        m = outline(tm(s, 'shapiro', H_PX, sy=H_SY), 1.25)
        strip = Image.new('L', (m.width * 3, m.height), 0)
        for i in range(3):
            strip.paste(m, (i * m.width, 0))
        _HS[k] = (strip, m.width)
    return _HS[k]


def h_travel(t):
    if t < 400:
        return t
    if t < 520:
        u = t - 400
        return 400 + u - u * u / 240
    return 460


def h_centre(cv, t):
    """The centre row: an outlined strip of zones with the slab on top."""
    strip, period = h_strip(0)
    x = -((h_travel(t) * 0.35) * SS % period) / SS
    put(cv, strip, 'LIME', x - period / SS, 233 - strip.height / SS / 2)


def h_slab(cv, word, n=None, pad=10):
    full = tm(word, 'shapiro', H_PX, sy=H_SY)
    left = 233 - full.width / SS / 2
    h = full.height / SS + 14
    x0 = left - pad
    x1 = left + (full.width / SS if n is None or n >= len(word) else tm(word[:n + 1], 'shapiro', H_PX, sy=H_SY).width / SS) + pad
    cv.rect(x0 - 2, 233 - h / 2 - 2, x1 + 2, 233 + h / 2 + 2, 'BLACK')
    cv.rect(x0, 233 - h / 2, x1, 233 + h / 2, 'LIME')
    reveal(cv, word, 'shapiro', H_PX, 'BLACK', 233, 233, len(word) if n is None else n, sy=H_SY)


def h_micro(cv):
    # above the slab
    frame_rect(cv, 96, 176, 138, 194, 'LIME', 1)
    cv.text('UTC', 'mono', 11, 'LIME', x=101, y=180)
    cv.rect(125, 180, 134, 190, 'LIME')
    cv.text('12:07:42', 'bold', 14, 'LIME', x=146, y=178)
    cv.text('24 SEP', 'mono', 12, 'LIME', x=228, y=172)
    cv.text('2026', 'mono', 12, 'LIME', x=228, y=186)
    cv.rect(282, 181, 290, 189, 'LIME')
    cv.text('TZDATA', 'bold', 12, 'LIME', x=298, y=172)
    cv.text('2026D', 'bold', 12, 'LIME', x=298, y=186)
    # below
    cv.text('EUROPE/DUBLIN', 'bold', 12, 'LIME', x=110, y=272)
    cv.text('IST +01', 'mono', 12, 'LIME', x=110, y=286)
    x = barcode(cv, '0.1.0', 222, 274, h=24, col='LIME')
    cv.text('BAT 87%', 'mono', 12, 'LIME', x=x + 10, y=272)
    cv.text('USB', 'mono', 12, 'LIME', x=x + 10, y=286)


def h_frame(t):
    cv = Canvas()
    # dark block noise, changing every 60 ms
    g = blob_noise(100 + t // 60, cell=16, density=0.42, iters=2)
    paint_blobs(cv, g, fade('PURPLE', 0.42), cell=16)
    nrows = min(len(H_ROWS), max(0, (t + 20) // 20))
    order = sorted(H_ROWS, key=abs)[:nrows]
    for k in order:
        strip, period = h_strip(k)
        v = (0.18 + 0.07 * abs(k)) * (1 if k % 2 else -1)
        x = -((h_travel(t) * v) * SS % period) / SS - period / SS
        y = 233 + k * H_PITCH - strip.height / SS / 2
        if not (t >= 620 and t < 660 and k % 2):          # the flicker: odd rows drop a frame pair
            put(cv, strip, 'LIME', x, y)
    if t >= 40:
        h_centre(cv, t)
    if t >= 160:
        h_micro(cv)
    if 200 <= t < 540:
        h_slab(cv, 'DUBLIN')
    elif t >= 540:
        n = (t - 540) // 16
        h_slab(cv, 'OCTOWHERE', n=n)
    return finish(cv)


def h_handover(t):
    cv = Canvas()
    if t < 40:
        h_slab(cv, 'OCTOWHERE')
    elif t < 60:
        cv.rect(0, 212, W, 254, 'LIME')
        putc(cv, tm('OCTOWHERE', 'shapiro', H_PX, sy=H_SY), 'BLACK', 233, 233)
    else:
        band_word(cv)
    return finish(cv)


H = dict(key='H', name='SLIDING ROWS', frame=h_frame, handover=h_handover, rest=720, hero=700,
         times=[0, 40, 120, 200, 300, 440, 520, 560, 620, 700, 760, 790],
         notes={0: 'Dark PURPLE block noise, reshuffled every 60 ms',
                40: 'Rows of zone names cut in from the centre outward, outlined, sliding both ways',
                120: 'Every row running; speeds grow with distance from the centre',
                200: 'Microtext clusters; a LIME slab lands on the centre row: DUBLIN, the set zone',
                300: 'Rows keep sliding under and around the slab',
                440: 'Rows decelerate to a stop',
                520: 'Stopped', 560: 'The slab types over to OCTOWHERE, widening letter by letter',
                620: 'Flicker: alternate rows drop out for two frames',
                700: 'Hold', 760: 'Handover: rows cut, the slab spreads to the edges',
                790: 'and becomes the band'})


# =================================================================================================
# I  GRID FILL
# =================================================================================================
M, GX0, GY0 = 40, 33, 38
ROWS_I = range(1, 9)                     # grid rows 1..8 = y 78..398; row 0 and 9 carry microtext and bar
RESERVED = {(1, r) for r in range(3, 7)}  # the rotated word


def i_layout(seed=5):
    rnd = random.Random(seed)
    used = set(RESERVED)
    cells = []

    def inside(c, r, w, h):
        for (x, y) in ((GX0 + c * M, GY0 + r * M), (GX0 + (c + w) * M, GY0 + r * M),
                       (GX0 + c * M, GY0 + (r + h) * M), (GX0 + (c + w) * M, GY0 + (r + h) * M)):
            if math.hypot(x - 233, y - 233) > 227:
                return False
        return all((c + a, r + b) not in used and 0 <= c + a < 10 and r + b in ROWS_I
                   for a in range(w) for b in range(h))

    for r in ROWS_I:
        for c in range(10):
            if (c, r) in used:
                continue
            for (w, h, p) in ((2, 2, 0.28), (2, 1, 0.22), (1, 2, 0.18), (1, 1, 1.0)):
                if rnd.random() < p and inside(c, r, w, h):
                    cells.append((c, r, w, h))
                    for a in range(w):
                        for b in range(h):
                            used.add((c + a, r + b))
                    break
    # fill order: from bottom-left to top-right, with jitter
    order = sorted(range(len(cells)), key=lambda i: (cells[i][0] - cells[i][1]) * 1.0 + rnd.random() * 5)
    rank = {i: n for n, i in enumerate(order)}
    return cells, rank


I_CELLS, I_RANK = i_layout()
ICON_GLYPHS = ['01110 11111 10001 11111 11111', '11111 10001 10101 10001 11111',   # the self-test glyphs:
               '00100 00100 11011 00100 00100', '10000 10000 10000 10000 11111',   # power, clock, touch,
               '11011 11011 11011 11111 01110', '00100 01010 10101 01010 00100']   # motion, magnet, gnss


def i_cell(cv, c, r, w, h, state, idx):
    x0, y0 = GX0 + c * M + 2, GY0 + r * M + 2
    x1, y1 = GX0 + (c + w) * M - 2, GY0 + (r + h) * M - 2
    if state == 0:
        cv.rect(x0, y0, x1, y1, 'BLACK')
        frame_rect(cv, x0, y0, x1, y1, 'PURPLE', 1)
        return
    if state == 1:                                   # the block ahead of the fill
        cv.rect(x0, y0, x1, y1, 'PURPLE')
        return
    cv.rect(x0, y0, x1, y1, 'LIME')
    if w == 2 and h == 2:
        g = ICON_GLYPHS[idx % len(ICON_GLYPHS)]
        glyph(cv, g, (x0 + x1) / 2 - 22.5, (y0 + y1) / 2 - 22.5, m=9, col='BLACK')
    elif w * h == 1 and idx % 3 == 0:
        cv.rect(x0 + 10, y0 + 10, x1 - 10, y1 - 10, 'BLACK')
    elif w * h == 2 and idx % 2 == 0:
        for j in range(4):
            cv.rect(x0 + 6 + j * 7, y1 - 10, x0 + 9 + j * 7, y1 - 5, 'BLACK')


def i_frame(t, band_only=False):
    cv = Canvas()
    g = blob_noise(21 + (t // 80), cell=20, density=0.5, iters=4)
    if t < 140:
        paint_blobs(cv, g, 'PURPLE', cell=20, dx=-(t // 40) * 20)
        return finish(cv)
    paint_blobs(cv, g, fade('PURPLE', 0.5), cell=20)
    k = min(1.0, (t - 160) / 360) if t >= 160 else 0
    n = len(I_CELLS)
    for i, (c, r, w, h) in enumerate(I_CELLS):
        rk = I_RANK[i] / n
        state = 0 if t < 160 or rk > k else (1 if rk > k - 0.04 and k < 1 else 2)
        if t < 140 + (r * 10):
            continue                                  # grid outlines drop in by row
        i_cell(cv, c, r, w, h, state, i)
    # the rotated word in its slot
    if t >= 300:
        cv.rect(GX0 + M + 2, GY0 + 3 * M + 2, GX0 + 2 * M - 2, GY0 + 7 * M - 2, 'BLACK')
        on = not (t in (580, 620))
        if on:
            vlabel(cv, 'OCTOWHERE', 'bold', 26, 'LIME', GX0 + 1.5 * M, GY0 + 5 * M)
    # microtext rows and the bar
    if t >= 180:
        cv.text('SELF TEST 6/6 OK', 'mono', 12, 'LIME', cx=233, y=50)
        cv.text('TZDATA 2026D  0.1.0', 'mono', 12, 'LIME', cx=233, y=64)
        bx0, bx1, by = 150, 316, 404
        frame_rect(cv, bx0, by, bx1, by + 16, 'LIME', 1)
        segs = int(40 * k)
        for s in range(segs):
            cv.rect(bx0 + 3 + s * 4, by + 3, bx0 + 5 + s * 4, by + 13, 'LIME')
        cv.text('%3d%%' % int(100 * k), 'bold', 12, 'LIME', x=bx1 + 6, y=by + 2)
        for j in range(4):
            cv.rect(bx0 - 14, by + j * 4 + 1, bx0 - 11, by + j * 4 + 3, 'LIME')
    # registration crosses on gutter intersections
    if t >= 200:
        for (c, r) in ((3, 2), (7, 4), (5, 7), (2, 8)):
            x, y = GX0 + c * M, GY0 + r * M
            cross(cv, x, y, 7, 'PURPLE', 2)
        pix_digits(cv, '24', GX0 + 7 * M + 5, GY0 + 4 * M + 5, m=2, col='PURPLE')
        pix_digits(cv, '09', GX0 + 5 * M + 5, GY0 + 7 * M + 5, m=2, col='PURPLE')
    return finish(cv)


def i_handover(t):
    cv = Canvas()
    if t < 40:
        for i, (c, r, w, h) in enumerate(I_CELLS):
            y0, y1 = GY0 + r * M, GY0 + (r + h) * M
            if y0 >= 198 and y1 <= 318:
                i_cell(cv, c, r, w, h, 2, i)
        cv.rect(GX0 + M + 2, 198, GX0 + 2 * M - 2, 318, 'LIME')
    elif t < 60:
        cv.rect(0, 198, W, 318, 'LIME')
        putc(cv, tm('OCTOWHERE', 'shapiro', 40), 'BLACK', 233, 257.5)
    else:
        band_word(cv)
    return finish(cv)


I = dict(key='I', name='GRID FILL', frame=i_frame, handover=i_handover, rest=700, hero=680,
         times=[0, 80, 160, 240, 320, 400, 480, 540, 580, 680, 720, 760],
         notes={0: 'Big PURPLE pixel blobs, stepping sideways',
                80: 'Blobs reshuffle every 80 ms',
                160: 'Blobs dim; a non-uniform grid drops in row by row, cells outlined',
                240: 'Cells fill: a PURPLE block first, then LIME; bottom-left to top-right',
                320: 'Big cells carry the device glyphs knocked out; the word stands in its slot',
                400: 'The bar counts the fill', 480: 'Nearly full', 540: 'Full',
                580: 'The word flickers', 680: 'Hold',
                720: 'Handover: everything off the band rows cuts', 760: 'The band'})


# =================================================================================================
# J  TRIANGLES
# =================================================================================================
TM_, TX0, TY0 = 60, -7, 18
OCT = {(2, 2): 'br', (3, 2): 'full', (4, 2): 'full', (5, 2): 'bl',
       (2, 3): 'full', (3, 3): 'full', (4, 3): 'full', (5, 3): 'full',
       (2, 4): 'full', (3, 4): 'full', (4, 4): 'full', (5, 4): 'full',
       (2, 5): 'tr', (3, 5): 'full', (4, 5): 'full', (5, 5): 'tl'}
ROT = ['bl', 'tl', 'tr', 'br']


def tri(cv, x, y, kind, col, s=TM_):
    d = draw(cv)
    X0, Y0, X1, Y1 = x * SS, y * SS, (x + s) * SS - 1, (y + s) * SS - 1
    pts = {'bl': [(X0, Y0), (X0, Y1), (X1, Y1)], 'br': [(X1, Y0), (X1, Y1), (X0, Y1)],
           'tl': [(X0, Y1), (X0, Y0), (X1, Y0)], 'tr': [(X0, Y0), (X1, Y0), (X1, Y1)]}
    if kind == 'full':
        d.rectangle([X0, Y0, X1, Y1], fill=rgb(col))
    elif kind in pts:
        d.polygon(pts[kind], fill=rgb(col))


J_FIELD = {}
_r = random.Random(9)
for _r_ in range(-1, 9):
    for _c in range(-1, 9):
        v = _r.random()
        J_FIELD[(_c, _r_)] = 'bl' if v < 0.62 else ('full' if v < 0.7 and _r_ % 3 == 0 else None)


def j_kind(c, r, t):
    """Field kind while flipping: rotate 90 degrees per step, a wave from the centre, then settle."""
    base = J_FIELD.get((c, r))
    dist = math.hypot(c - 3.5, r - 3.5)
    start = 180 + dist * 26
    if t < start:
        return base
    steps = int((t - start) // 40)
    target = OCT.get((c, r))
    if target is None and r not in (3, 4) and base and base != 'full':
        target = 'outer:' + ROT[(ROT.index(base) + 2) % 4]
    if steps >= 3:
        return target
    if base is None or base == 'full':
        return target if steps >= 1 else base
    return ROT[(ROT.index(base) + steps + 1) % 4]


def j_frame(t):
    cv = Canvas()
    off = (t * 0.25) % TM_ if t < 180 else 0
    for r in range(-1, 9):
        for c in range(-1, 9):
            kind = j_kind(c, r, t) if t >= 180 else J_FIELD.get((c, r))
            if kind and kind.startswith('outer:'):
                tri(cv, TX0 + c * TM_ + off, TY0 + r * TM_ + off, kind[6:], fade('PURPLE', 0.45))
            elif kind:
                tri(cv, TX0 + c * TM_ + off, TY0 + r * TM_ + off, kind, 'PURPLE')
    # pixel bits
    if t < 360:
        glyph(cv, '00011 00011 01100 01100 11000', 250, 90, m=4, col='PURPLE')
        cv.rect(318, 96, 330, 108, 'PURPLE')
    if t >= 300:
        vlabel(cv, 'V 0.1.0', 'bold', 26, 'LIME', 52, 258)
        for y in (202, 258, 314):
            xmark(cv, 30, y, 5, 'PURPLE')
    if t >= 380:
        n = (t - 380) // 16
        if not (t in (580, 620)):
            reveal(cv, 'OCTOWHERE', 'shapiro', 36, 'LIME', 238, 257.5, n)
    if t >= 420:
        cv.rect(150, 386, 316, 420, 'BLACK')
        cv.text('SELF TEST 6/6 OK', 'mono', 12, 'LIME', cx=233, y=390)
        cv.text('TZDATA 2026D', 'mono', 12, 'LIME', cx=233, y=405)
    return finish(cv)


def j_handover(t):
    cv = Canvas()
    if t < 40:                                          # top and bottom rows of the octagon cut
        for (c, r), kind in OCT.items():
            if r in (3, 4):
                tri(cv, TX0 + c * TM_, TY0 + r * TM_, kind, 'PURPLE')
        cv.rect(TX0 + 2 * TM_, 198, TX0 + 6 * TM_, 318, 'PURPLE')
        putc(cv, tm('OCTOWHERE', 'shapiro', 36), 'LIME', 238, 257.5)
    elif t < 60:                                        # the middle rows run to the edges
        cv.rect(0, 198, W, 318, 'PURPLE')
        putc(cv, tm('OCTOWHERE', 'shapiro', 36), 'LIME', 238, 257.5)
    else:
        band_word(cv)
    return finish(cv)


J = dict(key='J', name='TRIANGLES', frame=j_frame, handover=j_handover, rest=700, hero=680,
         times=[0, 100, 200, 260, 320, 380, 440, 560, 580, 680, 720, 760],
         notes={0: 'A field of PURPLE half-square triangles, some cells empty, scrolling diagonally',
                100: 'Still scrolling; pixel bits drift over it',
                200: 'Scroll stops; triangles flip 90 degrees per step in a wave from the centre',
                260: 'Cells off the octagon drop out as the wave passes',
                320: 'The octagon: four by four cells, corners cut by triangles. Rotated version tag',
                380: 'The word types in across it', 440: 'Microtext; rest', 560: 'Rest',
                580: 'The word flickers', 680: 'Hold',
                720: 'Handover: the octagon keeps its middle rows, which are the band rows',
                760: 'The rows run to the edges and become the band'})


# =================================================================================================
DIRS = [G, H, I, J]


def render(d, t):
    return d['frame'](t) if t < d['rest'] else d['handover'](t - d['rest'])


def board(d, path):
    sc = 0.5
    w = int(W * sc)
    f = ImageFont.truetype('fonts/MonoR.otf', 13)
    fb = ImageFont.truetype('fonts/MonoB.otf', 18)
    cols = 6
    rows = (len(d['times']) + cols - 1) // cols
    cell_h = w + 84
    sh = Image.new('RGB', (cols * (w + 12) + 12, rows * cell_h + 50), (20, 20, 22))
    dr = ImageDraw.Draw(sh)
    dr.text((12, 14), '%s  %s' % (d['key'], d['name']), font=fb, fill=(210, 211, 214))
    for i, t in enumerate(d['times']):
        im = render(d, t)
        x, y = 12 + (i % cols) * (w + 12), 48 + (i // cols) * cell_h
        sh.paste(im.resize((w, w), Image.LANCZOS), (x, y))
        dr.text((x, y + w + 6), '%d MS' % t, font=f, fill=(210, 211, 214))
        words, line, yy = d['notes'][t].split(), '', y + w + 24
        for wd in words:
            if len(line + ' ' + wd) > 31:
                dr.text((x, yy), line, font=f, fill=(136, 142, 152)); yy += 16; line = wd
            else:
                line = (line + ' ' + wd).strip()
        dr.text((x, yy), line, font=f, fill=(136, 142, 152))
    sh.save(path)


def gif(d):
    fr = [finish(Canvas())] * 6
    for t in range(0, d['rest'] + 120, DT):
        fr.append(render(d, t))
    fr += [fr[-1]] * 25
    save_gif(fr, 'out/identity-%s.gif' % d['key'], DT)
    save_gif(fr, 'out/identity-%s-slow3x.gif' % d['key'], DT * 3)


# =================================================================================================
# K  FAULT (the failed start-up; not an identity direction, it goes with whichever is chosen)
# =================================================================================================
K_PART, K_GLYPH, K_N = 'MAGNET', '11011 11011 11011 11111 01110', '05'
_KS = {}


def k_strip():
    if 'run' not in _KS:
        m = tm('%s FAIL_' % K_PART, 'shapiro', 32, sy=1.3)
        strip = Image.new('L', (m.width * 4, m.height), 0)
        for i in range(4):
            strip.paste(m, (i * m.width, 0))
        _KS['run'] = (strip, m.width)
        _KS['giant'] = tm(K_PART, 'shapiro', 200)
    return _KS['run']


def k_frame(t, cell=None):
    cv = Canvas()
    field(cv, 'RED')
    # dashed rules
    for y in (86, 150, 366, 430):
        for x in range(20, 450, 46):
            cv.rect(x, y, x + 9, y + 2, 'BLACK')
    # the part's glyph, four times, between bars
    for gx, gy in ((110, 118), (356, 118), (110, 348), (356, 348)):
        cv.rect(gx - 32, gy - 38, gx + 32, gy - 30, 'BLACK')
        cv.rect(gx - 32, gy + 30, gx + 32, gy + 38, 'BLACK')
        glyph(cv, K_GLYPH, gx - 22.5, gy - 22.5, m=9, col='BLACK')
    # the band: black, giant cropped part name in RED behind, the running line in WHITE
    band = Image.new('L', cv.img.size, 0)
    ImageDraw.Draw(band).rectangle([0, 198 * SS, W * SS, 318 * SS - 1], fill=255)
    cv.img.paste(Image.new('RGB', cv.img.size, rgb('BLACK')), (0, 0), ImageChops.multiply(band, clip()))
    strip, period = k_strip()
    giant = _KS['giant']
    gx = 233 - giant.width / SS / 2 + 60 - t * 0.03
    tmp = Canvas()
    put(tmp, giant, 'WHITE', gx, 258 - giant.height / SS / 2)
    gm = tmp.img.convert('L').point(lambda v: 255 if v > 127 else 0)
    gm = ImageChops.multiply(gm, band)
    cv.img.paste(Image.new('RGB', cv.img.size, rgb('RED')), (0, 0), gm)
    cv.rect(0, 234, W, 282, 'BLACK')
    x = -((t * 0.12) * SS % period) / SS - period / SS
    put(cv, strip, 'WHITE', x, 258 - strip.height / SS / 2)
    # microtext cluster, BLACK on RED, above and below the band on the right
    cv.text('SELF TEST 5/6 OK', 'bold', 12, 'BLACK', x=162, y=164)
    cv.text('%s %s FAIL' % (K_N, K_PART), 'mono', 12, 'BLACK', x=162, y=178)
    hatch(cv, 290, 164, 318, 190, 'BLACK', phase=t * 0.06)    # 2 px a frame at 30 fps
    cv.text('NO REPLY BY DEADLINE', 'mono', 12, 'BLACK', x=162, y=328)
    x = barcode(cv, '0.1.0', 162, 345, h=12, col='BLACK')
    cv.text('0.1.0', 'bold', 12, 'BLACK', x=x + 8, y=345)
    return finish(cv)


def k_render(t):
    """t from the failing cell's turn: 300 ms on the grid, the fault screen to 2300, then the clock."""
    import startup as S
    if t < 300:
        cells = [c if c[0] != 'MAGNET' else (c[0], c[1], 1000, False) for c in S.CELLS]
        return S.post(1000 + t, cells)
    if t < 2300:
        return k_frame(t - 300)
    from anim import page, entry
    return S.still(page(entry('B', t - 2300)))


K = dict(key='K', name='FAULT, THE FAILED START-UP', times=[0, 300, 900, 1500, 2280, 2600],
         notes={0: 'The failing cell turns RED on the self-test grid, as now; held 300 ms',
                300: 'The fault screen cuts on in one frame: RED field, BLACK band',
                900: 'The part name runs through the band; the giant name drifts the other way behind',
                1500: 'The part glyph four times, dashed rules, microtext: count, part, reason, version',
                2280: 'Held 2 s in all',
                2600: 'Cut to the clock face, which runs its entry and shows its own fault states'})


def k_board(path):
    d = dict(K, frame=k_render, rest=10 ** 6, handover=None)
    board(d, path)


if __name__ == '__main__':
    import sys
    keys = sys.argv[1:] or [d['key'] for d in DIRS]
    for d in DIRS:
        if d['key'] in keys:
            board(d, 'out/identity-%s.png' % d['key'])
            render(d, d['hero']).save('out/identity-%s-hero.png' % d['key'])
    if 'K' in keys:
        k_board('out/identity-K.png')
        k_frame(600).save('out/identity-K-hero.png')
