"""Settings panel: horizontal scrolling grid, and bolder looks on the same structure.

Items, column-major (two per column): ZONE, BRIGHTNESS | COMPASS, DEVICE | BATTERY, GNSS.
BATTERY and GNSS are proposals: grade-2 data, not passed to the screens yet. Their values here
are synthetic.
"""
import lib
from lib import *
from settings import GLYPH

GLYPH = dict(GLYPH)
GLYPH['battery'] = '01110 11111 10001 11111 11111'
GLYPH['gnss'] = '00100 01010 10101 01010 00100'     # the clock's GNSS glyph

ITEMS = [
    ('zone', 'ZONE', 'IST', 'WHITE', 'AUTO'),
    ('bright', 'BRIGHTNESS', '47%', 'WHITE', None),
    ('cal', 'COMPASS', 'CAL 54%', 'ORANGE', None),
    ('device', 'DEVICE', '0.1.0', 'WHITE', None),
    ('battery', 'BATTERY', '87%', 'WHITE', None),
    ('gnss', 'GNSS', 'FIX  9', 'BLUE', None),
]
NCOL = 3


def value_line(cv, cx, y, value, colour, tag, px=16):
    vcol = 'ORANGE' if colour == 'ORANGE' else 'WHITE'
    if tag:
        w_tag = adv('bold', 14) * len(tag) + 10
        w_val = cv.ink_size(value, 'mono', px)[0]
        x0 = round(cx - (w_tag + 6 + w_val) / 2)
        cv.rect(x0, y - 3, round(x0 + w_tag), y + 15, 'GRAY')
        cv.text(tag, 'bold', 14, 'BLACK', cx=x0 + w_tag / 2, cy=y + 6)
        cv.text(value, 'mono', px, vcol, x=x0 + w_tag + 6, cy=y + 6)
    else:
        cv.text(value, 'mono', px, vcol, cx=cx, cy=y + 6)


def pips(cv, first_visible, y=400, n=NCOL, shown=2):
    """One square mark per column; the columns in view are filled."""
    size, gap = 8, 6
    x0 = round(233 - (n * size + (n - 1) * gap) / 2)
    for i in range(n):
        x = x0 + i * (size + gap)
        if first_visible <= i < first_visible + shown:
            cv.rect(x, y, x + size, y + size, 'WHITE')
        else:
            cv.rect(x, y, x + size, y + size, 'GRAY')
            cv.rect(x + 2, y + 2, x + size - 2, y + size - 2, 'BLACK')


# ---- V1: clean scrolling grid ---------------------------------------------------------------
COLW = 136          # column pitch


def v_clean(scroll=0.0):
    """scroll in columns: 0 shows columns 0-1 centred, 1 shows 1-2."""
    cv = Canvas()
    cv.ring(230, 232, 'GRAY')
    cv.text('SETTINGS', 'shapiro', 16, 'WHITE', cx=233, y=50)
    for i, (key, name, value, col, tag) in enumerate(ITEMS):
        c, r = divmod(i, 2)
        cx = 233 + (c - 0.5 - scroll) * COLW
        x = round(cx - 48)
        y = (84, 232)[r]
        icon_sized(cv, x, y, col, GLYPH[key], module=16, pad=8)
        cv.text(name, 'bold', 14, 'GRAY', cx=cx, y=y + 104)
        value_line(cv, cx, y + 120, value, col, tag)
    pips(cv, round(scroll))
    cv.ring(230, 232, 'GRAY')
    return cv


# ---- V2: registration grid: full-chord rules make the cells --------------------------------------
def v_grid(scroll=0.0):
    cv = Canvas()
    rows = (72, 218, 364)             # horizontal rules: cell rows 72..217 and 218..363
    colw = 146
    g0 = 233 + (0 - 1 - scroll) * colw
    g1 = 233 + (NCOL - 1 - scroll) * colw
    for y in rows:
        cv.rect(round(g0), y, round(g1) + 1, y + 1, 'GRAY')
    for i, (key, name, value, col, tag) in enumerate(ITEMS):
        c, r = divmod(i, 2)
        left = 233 + (c - 1 - scroll) * colw
        top = rows[r] + 1
        # vertical rules at cell edges, clipped to the grid rows
        for xe in (left, left + colw):
            xr = round(xe)
            cv.rect(xr, rows[0], xr + 1, rows[2] + 1, 'GRAY')
        cv.text('%02d' % (i + 1), 'mono', 14, 'GRAY', x=left + 10, y=top + 9)
        icon_sized(cv, round(left + colw - 10 - 66), top + 9, col, GLYPH[key], module=10, pad=8)
        cv.text(name, 'bold', 14, 'GRAY', x=left + 10, y=top + 100)
        vcol = 'ORANGE' if col == 'ORANGE' else 'WHITE'
        if tag:
            w_tag = adv('bold', 14) * len(tag) + 10
            cv.rect(round(left + 10), top + 119, round(left + 10 + w_tag), top + 137, 'GRAY')
            cv.text(tag, 'bold', 14, 'BLACK', cx=left + 10 + w_tag / 2, cy=top + 128)
            cv.text(value, 'mono', 16, vcol, x=left + 16 + w_tag, cy=top + 128)
        else:
            cv.text(value, 'mono', 16, vcol, x=left + 10, cy=top + 128)
    # the rules outside the grid rows are cropped to the circle by the panel itself
    m = Image.new('L', (N * SS, N * SS), 0)
    ImageDraw.Draw(m).ellipse([(C - 232) * SS, (C - 232) * SS, (C + 232) * SS - 1, (C + 232) * SS - 1], fill=255)
    black = Image.new('RGB', cv.img.size, (0, 0, 0))
    inv = Image.eval(m, lambda v: 255 - v)
    cv.img.paste(black, (0, 0), inv)
    cv.text('SETTINGS', 'shapiro', 16, 'WHITE', cx=233, y=38)
    pips(cv, round(scroll), y=388)
    cv.ring(230, 232, 'GRAY')
    return cv


# ---- V3: band spine: the clock's band carries the title between the rows ------------------------
def v_band(scroll=0.0):
    cv = Canvas()
    cv.ring(230, 232, 'GRAY')
    band_rows = (199, 267)
    clipped_band(cv, *band_rows, 'WHITE')
    cv.text('SETTINGS', 'shapiro', 28, 'BLACK', x=62, cy=(band_rows[0] + band_rows[1]) / 2)
    # page count, knocked out of the band, right
    cv.text('%d/%d' % (round(scroll) + 1, NCOL - 1), 'bold', 16, 'BLACK', right=404, cy=(band_rows[0] + band_rows[1]) / 2)
    for i, (key, name, value, col, tag) in enumerate(ITEMS):
        c, r = divmod(i, 2)
        cx = 233 + (c - 0.5 - scroll) * COLW
        x = round(cx - 48)
        if r == 0:
            y = 88
            icon_sized(cv, x, y, col, GLYPH[key], module=16, pad=8)
            cv.text(name, 'bold', 14, 'GRAY', cx=cx, y=60)
            value_line(cv, cx, 188 - 16, value, col, tag) if False else None
        else:
            y = 280
            icon_sized(cv, x, y, col, GLYPH[key], module=16, pad=8)
            cv.text(name, 'bold', 14, 'GRAY', cx=cx, y=384)
    # values: row 1 values ride in the band under each icon, knocked out
    for i, (key, name, value, col, tag) in enumerate(ITEMS):
        c, r = divmod(i, 2)
        cx = 233 + (c - 0.5 - scroll) * COLW
        if r == 0:
            pass
    return cv


# ---- V4: giant carousel: one row, the centred item huge, its name and value in the band ----------
def v_carousel(focus=1.0):
    """focus: index of the item centred (fractional mid-scroll)."""
    cv = Canvas()
    cv.ring(230, 232, 'GRAY')
    pitch = 196
    for i, (key, name, value, col, tag) in enumerate(ITEMS):
        cx = 233 + (i - focus) * pitch
        icon_sized(cv, round(cx - 76), 44, col, GLYPH[key], module=28, pad=6)       # 152 px
    clipped_band(cv, 214, 318, 'WHITE')
    key, name, value, col, tag = ITEMS[round(focus)]
    cv.text(name, 'shapiro', 30, 'BLACK', x=62, y=232)
    vt = ('%s %s' % (tag, value)) if tag else value
    cv.text(vt, 'bold', 40, 'BLACK', x=62, bottom=302)
    # position: a row of cells, the centred one filled
    n = len(ITEMS)
    size, gap = 10, 6
    x0 = round(233 - (n * size + (n - 1) * gap) / 2)
    for i in range(n):
        x = x0 + i * (size + gap)
        if i == round(focus):
            cv.rect(x, 340, x + size, 340 + size, 'WHITE')
        else:
            cv.rect(x, 340, x + size, 340 + size, 'GRAY')
            cv.rect(x + 2, 342, x + size - 2, 348, 'BLACK')
    cv.text('TAP TO OPEN', 'mono', 14, 'GRAY', cx=233, y=372)
    cv.text('DRAG UP TO CLOSE', 'mono', 14, 'GRAY', cx=233, y=392)
    return cv


if __name__ == '__main__':
    lib.RECORD = False
    shots = [
        (v_clean(0).render(), 'SCROLLING GRID'),
        (v_clean(0.45).render(), 'SCROLLING GRID, MID-DRAG'),
        (v_grid(0).render(), 'REGISTRATION GRID'),
        (v_grid(0.45).render(), 'REGISTRATION GRID, MID-DRAG'),
        (v_carousel(1).render(), 'CAROUSEL: BRIGHTNESS CENTRED'),
        (v_carousel(1.45).render(), 'CAROUSEL, MID-DRAG'),
        (v_carousel(2).render(), 'CAROUSEL: COMPASS CENTRED'),
        (v_carousel(0).render(), 'CAROUSEL: ZONE CENTRED'),
    ]
    f = ImageFont.truetype('fonts/MonoR.otf', 16)
    cols = 4
    W, Hh = 466 + 24, 466 + 44
    sh = Image.new('RGB', (cols * W + 24, 2 * Hh + 24), (28, 28, 28))
    d = ImageDraw.Draw(sh)
    for i, (im, label) in enumerate(shots):
        x, y = 24 + (i % cols) * W, 24 + (i // cols) * Hh
        sh.paste(im, (x, y))
        d.text((x, y + 474), label, font=f, fill=(136, 142, 152))
    sh.save('out/settings-explore.png')
