"""Clock face, round 4, third pass: the lower block (date, plate, zone name) in the microtext
language, on variant D of clock5 (LIME band, scatter under everything, microtext above and in the
band, seconds column). Distinct from the identity's row: centred, not left-aligned; built around
the date; the plate's cell grammar kept (a mode in force is filled GRAY).
"""
import lib
lib.RECORD = False
from lib import *
from PIL import ImageFilter
import numpy as np
from face import SYM, X0, BIG, HOURS_BASE, MIN_BASE, SEC_PEN, SEC_PX
from industrial import digits, cells_text, pen_for, BAND
from identity import finish
from identity_f import pix_digits, glyph
from identity_g import frame_rect, hatch
import clockcolor as CC
import clock4 as C4
import clock5 as C5

W = 466
F = dict(C5.F, day='24', month='09', mon='SEP', wd='THU', year='2026', doy=268, week=39)


def pix_w(s, m):
    return sum(2 * m if c == ' ' else 4 * m for c in s) - m


def cells(cv, items, x, top, ground, h=20, px=14):
    """The plate's grammar: ('tag', t) filled GRAY with BLACK text; ('cell', t, col) framed."""
    padx, gap = 6, 4
    base = top + (h + cap_height('mono', px)) / 2
    for it in items:
        t = it[1]
        w = round(adv('mono', px) * len(t) + 2 * padx)
        ground.append((x, top, x + w, top + h))
        if it[0] == 'tag':
            cv.rect(x, top, x + w, top + h, 'GRAY')
            cv.text(t, 'bold', px, 'BLACK', x=x + padx, bottom=base)
        else:
            frame_rect(cv, x, top, x + w, top + h, 'GRAY', 1)
            cv.text(t, 'mono', px, it[2] if len(it) > 2 else 'WHITE', x=x + padx, bottom=base)
        x += w + gap
    return x - gap


def cells_w(items, px=14):
    return sum(round(adv('mono', px) * len(it[1]) + 12) for it in items) + 4 * (len(items) - 1)


def lower_a(cv, f, ground):
    """Two clusters either side of a rule: the date round a pixel-digit day, the zone round its tag."""
    m = 5
    # date cluster
    dw = pix_w(f['day'], m)
    tw = font('mono', 14).getlength('SEP 2026') / SS
    zw = max(font('mono', 14).getlength(f['zone']), font('mono', 14).getlength('IST +01:00')) / SS
    tagw = cells_w([('tag', f['mode'])])
    total = dw + 8 + tw + 16 + 1 + 16 + tagw + 8 + zw
    x = round(233 - total / 2)
    top = 336
    pix_digits(cv, f['day'], x, top, m=m, col='LIME')
    x += dw + 8
    cv.text(f['wd'], 'bold', 14, 'LIME', x=x, y=top)
    cv.text('%s %s' % (f['mon'], f['year']), 'mono', 14, 'LIME', x=x, y=top + 15)
    x += tw + 16
    cv.rect(x, top - 2, x + 1, top + 27, 'GRAY')
    x += 17
    cells(cv, [('tag', f['mode'])], x, top + 3, ground)
    x += tagw + 8
    cv.text('%s %s' % (f['abbr'], f['off']), 'mono', 14, 'WHITE', x=x, y=top)
    cv.text(f['zone'], 'mono', 14, 'GRAY', x=x, y=top + 15)
    # a derived line under both: day of year and ISO week, with the mark's hatch as a spacer
    cv.text('DAY %03d  WK %02d' % (f['doy'], f['week']), 'mono', 14, 'LIME', cx=233, y=378)


def lower_b(cv, f, ground):
    """The date as large pixel digits, weekday and year either side; the plate and zone on one line."""
    m = 5
    s = '%s %s' % (f['day'], f['month'])
    dw = pix_w(s, m)
    lw = font('bold', 14).getlength(f['wd']) / SS
    rw = font('mono', 14).getlength(f['year']) / SS
    total = lw + 10 + dw + 10 + rw
    x = round(233 - total / 2)
    top = 334
    cv.text(f['wd'], 'bold', 14, 'LIME', x=x, bottom=top + 25)
    x += lw + 10
    pix_digits(cv, s, x, top, m=m, col='LIME')
    x += dw + 10
    cv.text(f['year'], 'mono', 14, 'LIME', x=x, bottom=top + 25)
    items = [('tag', f['mode']), ('cell', f['abbr']), ('cell', f['off'])]
    zw = font('mono', 14).getlength(f['zone']) / SS
    total = cells_w(items) + 10 + zw
    x = round(233 - total / 2)
    x = cells(cv, items, x, 370, ground) + 10
    cv.text(f['zone'], 'mono', 14, 'GRAY', x=x, bottom=370 + (20 + cap_height('mono', 14)) / 2)


def lower_c(cv, f, ground):
    """The date as a row of cells in the plate's grammar, the zone's cells under it, the name last."""
    items = [('cell', f['wd'], 'LIME'), ('cell', f['day'], 'LIME'), ('cell', f['mon'], 'LIME'),
             ('cell', f['year'], 'LIME')]
    x = round(233 - cells_w(items) / 2)
    cells(cv, items, x, 334, ground)
    items = [('tag', f['mode']), ('cell', f['abbr']), ('cell', f['off'])]
    x = round(233 - cells_w(items) / 2)
    cells(cv, items, x, 360, ground)
    cells_text(cv, f['zone'], 'mono', 14, 'GRAY', pen_for(f['zone'], 'mono', 14, cx=233), 398)


def compose(lower, f=F, o=None):
    o = o or dict(micro_top=True, micro_band=True, band_seconds=True)
    ground = [(0, BAND[0], W, BAND[1]), (262, 90, 358, 186)]

    def draw(cv):
        digits(cv, '%02d' % f['h'], BIG, 'WHITE', X0, HOURS_BASE)
        icon_sized(cv, 262, 90, 'BLUE', SYM['gnss'], module=16, pad=8)
        band(cv, BAND[0], BAND[1], 'LIME')
        cells_text(cv, 'LOCAL', 'bold', 16, 'BLACK', pen_for('LOCAL', 'bold', 16, x=262), 225)
        digits(cv, '%02d' % f['m'], BIG, 'BLACK', X0, MIN_BASE)
        digits(cv, '%02d' % f['s'], SEC_PX, 'BLACK', SEC_PEN, MIN_BASE)
        CC.wordmark(cv, 'LIME', 'BLACK')
        if o.get('micro_top'):
            CC.micro_top(cv, f, 'LIME')
        if o.get('micro_band'):
            CC.micro_band(cv, f, 'BLACK')
        if o.get('battery_col'):
            battery_col(cv, f.get('bat'), f.get('charging', False), f.get('phase', 0))
        elif o.get('band_seconds'):
            C5.band_seconds(cv, f['s'])
        lower(cv, f, ground)
    base = Canvas()
    C4.scatter(base, 0.8, zone=(0, -1), density=0.8, centre=0.15)
    top = Canvas()
    draw(top)
    arr = np.array(top.img)
    m = Image.fromarray(((arr.sum(axis=2) > 0) * 255).astype('uint8'))
    d = ImageDraw.Draw(m)
    for (a, b, c, e) in ground:
        d.rectangle([a * SS, b * SS, c * SS - 1, e * SS - 1], fill=255)
    base.img.paste((0, 0, 0), (0, 0), m.filter(ImageFilter.MaxFilter(int(2 * SS) * 2 + 1)))
    base.img.paste(top.img, (0, 0), m)
    base.ring(230, 232, 'GRAY')
    return finish(base)


def lower_b3(cv, f, ground):
    """L2, with a third line that takes what the top row carried: UTC, satellites, battery."""
    lower_b(cv, f, ground)
    cv.text('UTC %s  SAT %02d/%02d  BAT %d%%' % (f['utc'], 7, 12, 87), 'mono', 14, 'LIME', cx=233, y=400)


def one_place(path):
    shots = [(compose(lower_b, o=dict(micro_band=True, band_seconds=True)),
              'A  BOTTOM, BAND KEEPS UTC + BATTERY'),
             (compose(lower_b3, o=dict(band_seconds=True)),
              'B  BOTTOM ONLY, BAND CLEAR'),
             (compose(lower_plain, o=dict(micro_top=True, micro_band=True, band_seconds=True)),
              'C  TOP ONLY, LOWER BLOCK AS BUILT')]
    fnt = ImageFont.truetype('fonts/MonoB.otf', 15)
    Wc, Hh = W + 24, W + 48
    sh = Image.new('RGB', (3 * Wc + 24, Hh + 24), (20, 20, 22))
    dr = ImageDraw.Draw(sh)
    for i, (im, name) in enumerate(shots):
        x, y = 24 + i * Wc, 24
        sh.paste(im, (x, y))
        im.save('out/clock7-%s.png' % 'ABC'[i])
        dr.text((x, y + W + 8), name, font=fnt, fill=(210, 211, 214))
    sh.save(path)


def lower_plain(cv, f, ground):
    from industrial import plate
    cv.text(f['date'], 'mono', 23, 'WHITE', cx=233, y=334)
    items = [('tag', f['mode']), ('cell', f['abbr']), ('cell', f['off'])]
    x = round(233 - cells_w(items) / 2)
    cells(cv, items, x, 360, ground)
    cells_text(cv, f['zone'], 'mono', 14, 'GRAY', pen_for(f['zone'], 'mono', 14, cx=233), 398)


if __name__ == '__main__' and __import__('sys').argv[1:] == ['one']:
    one_place('out/clock7.png')
elif __name__ == '__main__':
    shots = [(Image.open('out/clock5-D.png'), 'D  AS BEFORE'),
             (compose(lower_a), 'L1  TWO CLUSTERS: THE DATE, THE ZONE'),
             (compose(lower_b), 'L2  THE DATE IN PIXEL DIGITS'),
             (compose(lower_c), 'L3  THE DATE AS CELLS')]
    fnt = ImageFont.truetype('fonts/MonoB.otf', 16)
    Wc, Hh = W + 24, W + 48
    sh = Image.new('RGB', (2 * Wc + 24, 2 * Hh + 24), (20, 20, 22))
    dr = ImageDraw.Draw(sh)
    for i, (im, name) in enumerate(shots):
        x, y = 24 + (i % 2) * Wc, 24 + (i // 2) * Hh
        sh.paste(im, (x, y))
        dr.text((x, y + W + 8), name, font=fnt, fill=(210, 211, 214))
    sh.save('out/clock6.png')


# ---- code-style tokens (owner, 25 Sep): // comments and [brackets] instead of boxes ---------------
def tokens(cv, segs, y, cx=233, px=14):
    """segs: (text, face, colour). Drawn on one baseline, centred as a group on cx by advance."""
    fnt = {k: font(k, px) for k in ('mono', 'bold')}
    widths = [fnt[fc].getlength(t) / SS for t, fc, _ in segs]
    x = cx - sum(widths) / 2
    base = y + cap_height('mono', px)
    for (t, fc, col), w in zip(segs, widths):
        pen(cv, t, fc, px, col, x, base)
        x += w


def lower_code1(cv, f, ground):
    """The pixel-digit date, then one line of tokens: the mode in brackets, the zone as a comment."""
    m = 5
    s = '%s %s' % (f['day'], f['month'])
    dw = pix_w(s, m)
    lw = font('bold', 14).getlength(f['wd']) / SS
    rw = font('mono', 14).getlength(f['year']) / SS
    x = round(233 - (lw + 10 + dw + 10 + rw) / 2)
    top = 336
    cv.text(f['wd'], 'bold', 14, 'LIME', x=x, bottom=top + 25)
    x += lw + 10
    pix_digits(cv, s, x, top, m=m, col='LIME')
    x += dw + 10
    cv.text(f['year'], 'mono', 14, 'LIME', x=x, bottom=top + 25)
    tokens(cv, [('[', 'mono', 'GRAY'), (f['mode'], 'bold', 'WHITE'), (']', 'mono', 'GRAY'),
                (' %s %s ' % (f['abbr'], f['off']), 'mono', 'WHITE'),
                ('// %s' % f['zone'], 'mono', 'GRAY')], 374)


def lower_code2(cv, f, ground):
    """As code1, the tokens on two lines: the mode and offset, then the zone as a comment."""
    m = 5
    s = '%s %s' % (f['day'], f['month'])
    dw = pix_w(s, m)
    lw = font('bold', 14).getlength(f['wd']) / SS
    rw = font('mono', 14).getlength(f['year']) / SS
    x = round(233 - (lw + 10 + dw + 10 + rw) / 2)
    top = 336
    cv.text(f['wd'], 'bold', 14, 'LIME', x=x, bottom=top + 25)
    x += lw + 10
    pix_digits(cv, s, x, top, m=m, col='LIME')
    x += dw + 10
    cv.text(f['year'], 'mono', 14, 'LIME', x=x, bottom=top + 25)
    tokens(cv, [('[', 'mono', 'GRAY'), (f['mode'], 'bold', 'LIME'), (']', 'mono', 'GRAY'),
                (' %s %s' % (f['abbr'], f['off']), 'mono', 'WHITE')], 372)
    tokens(cv, [('// %s' % f['zone'], 'mono', 'GRAY')], 390)


def lower_code3(cv, f, ground):
    """No pixel digits: the date as a token line in LIME, the zone line under it."""
    tokens(cv, [('%s ' % f['wd'], 'bold', 'LIME'), ('%s %s %s' % (f['day'], f['mon'], f['year']), 'mono', 'LIME'),
                ('  [', 'mono', 'GRAY'), (f['mode'], 'bold', 'WHITE'), (']', 'mono', 'GRAY')], 340, px=19)
    tokens(cv, [('%s %s ' % (f['abbr'], f['off']), 'mono', 'WHITE'), ('// %s' % f['zone'], 'mono', 'GRAY')], 368)


def code_sheet(path):
    o = dict(micro_band=True, band_seconds=True)
    shots = [(Image.open('out/clock7-A.png'), 'A  AS BEFORE'),
             (compose(lower_code1, o=o), 'A1  DATE, ONE TOKEN LINE'),
             (compose(lower_code2, o=o), 'A2  DATE, TWO TOKEN LINES'),
             (compose(lower_code3, o=o), 'A3  NO PIXEL DIGITS')]
    fnt = ImageFont.truetype('fonts/MonoB.otf', 15)
    Wc, Hh = W + 24, W + 48
    sh = Image.new('RGB', (2 * Wc + 24, 2 * Hh + 24), (20, 20, 22))
    dr = ImageDraw.Draw(sh)
    for i, (im, name) in enumerate(shots):
        x, y = 24 + (i % 2) * Wc, 24 + (i // 2) * Hh
        sh.paste(im, (x, y))
        if i:
            im.save('out/clock8-A%d.png' % i)
        dr.text((x, y + W + 8), name, font=fnt, fill=(210, 211, 214))
    sh.save(path)


if __name__ == '__main__' and __import__('sys').argv[1:] == ['code']:
    code_sheet('out/clock8.png')


def lower_a3(palette):
    """A3 with fewer colours. palette: 'mono' all LIME; 'syntax' LIME values, GRAY syntax and
    comment; 'lines' LIME first line, GRAY second."""
    def draw(cv, f, ground):
        L, G = 'LIME', 'GRAY'
        s1 = {'mono': (L, L, L), 'syntax': (L, G, L), 'lines': (L, L, L)}[palette]
        s2 = {'mono': (L, L), 'syntax': (L, G), 'lines': (G, G)}[palette]
        tokens(cv, [('%s ' % f['wd'], 'bold', s1[0]), ('%s %s %s' % (f['day'], f['mon'], f['year']), 'mono', s1[0]),
                    ('  [', 'mono', s1[1]), (f['mode'], 'bold', s1[2]), (']', 'mono', s1[1])], 340, px=19)
        tokens(cv, [('%s %s ' % (f['abbr'], f['off']), 'mono', s2[0]), ('// %s' % f['zone'], 'mono', s2[1])], 368)
    return draw


def a3_sheet(path):
    o = dict(micro_band=True, band_seconds=True)
    shots = [(Image.open('out/clock8-A3.png'), 'A3  AS BEFORE'),
             (compose(lower_a3('mono'), o=o), 'A3a  ALL LIME'),
             (compose(lower_a3('syntax'), o=o), 'A3b  LIME, SYNTAX AND COMMENT GRAY'),
             (compose(lower_a3('lines'), o=o), 'A3c  LIME DATE LINE, GRAY ZONE LINE')]
    fnt = ImageFont.truetype('fonts/MonoB.otf', 15)
    Wc, Hh = W + 24, W + 48
    sh = Image.new('RGB', (2 * Wc + 24, 2 * Hh + 24), (20, 20, 22))
    dr = ImageDraw.Draw(sh)
    for i, (im, name) in enumerate(shots):
        x, y = 24 + (i % 2) * Wc, 24 + (i // 2) * Hh
        sh.paste(im, (x, y))
        if i:
            im.save('out/clock9-%s.png' % 'xabc'[i])
        dr.text((x, y + W + 8), name, font=fnt, fill=(210, 211, 214))
    sh.save(path)


if __name__ == '__main__' and __import__('sys').argv[1:] == ['a3']:
    a3_sheet('out/clock9.png')



# ---- the band's column as the battery (owner, 25 Sep) ---------------------------------------------
LOW = 15


def battery_col(cv, pct, charging=False, phase=0):
    """A BLACK window in the band. The charge is a LIME fill from the bottom, one step per
    percent; above it, 2 px of BLACK, then LIME hatch for what is not charge. At or under LOW %
    fill and hatch turn ORANGE. Unknown: all hatch. While charging the hatch crawls down, 1 px a
    frame, folding into the fill (USB power only)."""
    x0, x1, y0, y1 = C5.SEC_COL
    col = 'ORANGE' if (pct is not None and pct <= LOW) else 'LIME'
    cv.rect(x0, y0, x1, y1, 'BLACK')
    ix0, ix1, iy0, iy1 = x0 + 3, x1 - 3, y0 + 3, y1 - 3            # a 3 px BLACK margin inside
    h = 0 if pct is None else round((iy1 - iy0) * pct / 100)
    top_of_fill = iy1 - h
    if top_of_fill - 2 > iy0:
        hatch(cv, ix0, iy0, ix1, top_of_fill - 2, col, pitch=8, w=4, phase=phase if charging else 0)
    if h > 0:
        cv.rect(ix0, top_of_fill, ix1, iy1, col)


def battery_sheet(path):
    base = dict(micro_band=True, battery_col=True)
    lo = lower_a3('lines')
    shots = [(compose(lo, f=dict(F, bat=87), o=base), '87 %: A LIME FILL IN A BLACK WINDOW'),
             (compose(lo, f=dict(F, bat=40, charging=True, phase=4), o=base), 'CHARGING 40 %: THE HATCH CRAWLS DOWN'),
             (compose(lo, f=dict(F, bat=12), o=base), '12 %: FILL AND HATCH ORANGE'),
             (compose(lo, f=dict(F, bat=None), o=base), 'UNKNOWN: ALL HATCH, BAT --')]
    fnt = ImageFont.truetype('fonts/MonoB.otf', 15)
    Wc, Hh = W + 24, W + 48
    sh = Image.new('RGB', (2 * Wc + 24, 2 * Hh + 24), (20, 20, 22))
    dr = ImageDraw.Draw(sh)
    for i, (im, name) in enumerate(shots):
        x, y = 24 + (i % 2) * Wc, 24 + (i // 2) * Hh
        sh.paste(im, (x, y))
        im.save('out/clock10-%d.png' % i)
        dr.text((x, y + W + 8), name, font=fnt, fill=(210, 211, 214))
    sh.save(path)


if __name__ == '__main__' and __import__('sys').argv[1:] == ['bat']:
    battery_sheet('out/clock10.png')


def battery_col_hatch(cv, pct, charging=False, phase=0):
    """Alternative: the charge is LIME hatch from the bottom; the missing part is the window's
    BLACK. Low: ORANGE hatch. While charging the hatch crawls up, rising with the charge."""
    x0, x1, y0, y1 = C5.SEC_COL
    col = 'ORANGE' if (pct is not None and pct <= LOW) else 'LIME'
    cv.rect(x0, y0, x1, y1, 'BLACK')
    ix0, ix1, iy0, iy1 = x0 + 3, x1 - 3, y0 + 3, y1 - 3
    frame_rect(cv, x0 + 1, y0 + 1, x1 - 1, y1 - 1, col, 1)
    h = 0 if pct is None else round((iy1 - iy0) * pct / 100)
    if h > 0:
        hatch(cv, ix0, iy1 - h, ix1, iy1, col, pitch=8, w=4, phase=-phase if charging else 0)


def battery_sheet2(path):
    lo = lower_a3('lines')
    global battery_col
    keep = battery_col
    battery_col = battery_col_hatch
    base = dict(micro_band=True, battery_col=True)
    try:
        shots = [(compose(lo, f=dict(F, bat=87), o=base), '87 %: HATCH IS CHARGE'),
                 (compose(lo, f=dict(F, bat=40, charging=True, phase=4), o=base), 'CHARGING 40 %: THE HATCH CRAWLS UP'),
                 (compose(lo, f=dict(F, bat=12), o=base), '12 %: ORANGE'),
                 (compose(lo, f=dict(F, bat=None), o=base), 'UNKNOWN: AN EMPTY WINDOW, BAT --')]
    finally:
        battery_col = keep
    fnt = ImageFont.truetype('fonts/MonoB.otf', 15)
    Wc, Hh = W + 24, W + 48
    sh = Image.new('RGB', (2 * Wc + 24, 2 * Hh + 24), (20, 20, 22))
    dr = ImageDraw.Draw(sh)
    for i, (im, name) in enumerate(shots):
        x, y = 24 + (i % 2) * Wc, 24 + (i // 2) * Hh
        sh.paste(im, (x, y))
        dr.text((x, y + W + 8), name, font=fnt, fill=(210, 211, 214))
    sh.save(path)


if __name__ == '__main__' and __import__('sys').argv[1:] == ['bat2']:
    battery_sheet2('out/clock11.png')


def battery_unknown(cv, style, t=0):
    """Ideas for the column while the battery is unknown (until the power controller answers)."""
    x0, x1, y0, y1 = C5.SEC_COL
    cv.rect(x0, y0, x1, y1, 'BLACK')
    ix0, ix1, iy0, iy1 = x0 + 3, x1 - 3, y0 + 3, y1 - 3
    if style == 'gray':                     # the rule's "valid but unconfirmed" colour, full height
        frame_rect(cv, x0 + 1, y0 + 1, x1 - 1, y1 - 1, 'GRAY', 1)
        hatch(cv, ix0, iy0, ix1, iy1, 'GRAY', pitch=8, w=4)
    elif style == 'dashes':                 # the '--' of BAT --, as a stack of bars
        frame_rect(cv, x0 + 1, y0 + 1, x1 - 1, y1 - 1, 'LIME', 1)
        for k in range(-1, 2):
            cy = (iy0 + iy1) / 2 + k * 16
            cv.rect(ix0 + 8, cy - 3, ix1 - 8, cy + 3, 'LIME')
    elif style == 'checker':                # a sparse checker of the scatter's cells
        frame_rect(cv, x0 + 1, y0 + 1, x1 - 1, y1 - 1, 'LIME', 1)
        for j, y in enumerate(range(iy0 + 2, iy1 - 5, 8)):
            for i, x in enumerate(range(ix0 + 2, ix1 - 5, 8)):
                if (i + j) % 2 == 0:
                    cv.rect(x, y, x + 5, y + 5, 'LIME')
    elif style == 'scan':                   # a hatch block sweeping up and down: probing
        frame_rect(cv, x0 + 1, y0 + 1, x1 - 1, y1 - 1, 'LIME', 1)
        span = (iy1 - iy0) - 20
        p = (t % 40) / 20
        y = iy0 + span * (p if p <= 1 else 2 - p)
        hatch(cv, ix0, y, ix1, y + 20, 'LIME', pitch=8, w=4)


def unknown_sheet(path):
    lo = lower_a3('lines')
    global battery_col
    keep = battery_col
    shots = []
    for style, name in (('gray', 'U1  GRAY HATCH: UNCONFIRMED'), ('dashes', 'U2  BARS: THE -- OF BAT --'),
                        ('checker', 'U3  A SPARSE CHECKER'), ('scan', 'U4  A SCANNING BLOCK (GIF)')):
        battery_col = (lambda st: (lambda cv, pct, charging=False, phase=0: battery_unknown(cv, st, phase)))(style)
        shots.append((compose(lo, f=dict(F, bat=None, phase=7), o=dict(micro_band=True, battery_col=True)), name))
    battery_col = keep
    fnt = ImageFont.truetype('fonts/MonoB.otf', 15)
    Wc, Hh = W + 24, W + 48
    sh = Image.new('RGB', (2 * Wc + 24, 2 * Hh + 24), (20, 20, 22))
    dr = ImageDraw.Draw(sh)
    for i, (im, name) in enumerate(shots):
        x, y = 24 + (i % 2) * Wc, 24 + (i // 2) * Hh
        sh.paste(im, (x, y))
        dr.text((x, y + W + 8), name, font=fnt, fill=(210, 211, 214))
    sh.save(path)
    # the scan as a close-up GIF, one step a frame
    from anim import save_gif
    battery_col = lambda cv, pct, charging=False, phase=0: battery_unknown(cv, 'scan', phase)
    fr = [compose(lo, f=dict(F, bat=None, phase=n), o=dict(micro_band=True, battery_col=True)).crop((233, 150, 466, 390))
          for n in range(0, 40, 2)]
    battery_col = keep
    save_gif(fr * 3, 'out/clock12-scan.gif', 40)


if __name__ == '__main__' and __import__('sys').argv[1:] == ['unk']:
    unknown_sheet('out/clock12.png')
