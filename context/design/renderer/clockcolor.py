"""Clock face, round 4 exploration: more colour in the normal operating state (LOCAL), and a
background with more character. The built layout is unchanged; only colours and a background
layer vary. Fixture: LOCAL, GNSS, 13:07:42, Thu 24 Sep 2026, Europe/Dublin.

Background rules kept from the built face:
- every antialiased edge sits on a known flat colour, so the pattern keeps clear of every text
  and symbol by a margin (the clear boxes below), and stops 2 px short of the band;
- the face at rest draws nothing: the pattern is static, or steps rarely (the 24-hour variant
  moves once an hour);
- it stops inside radius 228, clear of the ring.
"""
import math
import lib
lib.RECORD = False
from lib import *
from PIL import ImageChops
from face import SYM, X0, BIG, HOURS_BASE, MIN_BASE, SEC_PEN, SEC_PX
from industrial import digits, cells_text, pen_for, plate, BAND, FIX
from wordmark2 import _letters, _LCACHE
from identity import finish

W = 466
GREEN = '#01E67C'
YELLOW = '#ECDB0B'
PINK = '#E8337C'
VIOLET = '#B32BE5'

# clear boxes around every element on the field (panel px, inclusive-exclusive), margin 6 px
M = 6
CLEAR = [
    (120 - M, 48 - M, 346 + M, 63 + M),       # the top microtext row, when shown (widest)
    (71 - M, 90 - M, 217 + M, 186 + M),       # hours
    (262 - M, 90 - M, 358 + M, 186 + M),      # icon
    (120 - M, 334 - M, 346 + M, 351 + M),     # date row
    (140 - M, 360 - M, 326 + M, 380 + M),     # plate (widest case)
    (108 - M, 388 - M, 358 + M, 402 + M),     # zone name (longest)
]


def mark_mask():
    if 'l' not in _LCACHE:
        _LCACHE['l'] = _letters()
    u = None
    for m, _ in _LCACHE['l']:
        u = m if u is None else ImageChops.lighter(u, m)
    return u


def wordmark(cv, field_col='WHITE', band_col='BLACK'):
    t = mark_mask()
    rows = Image.new('L', t.size, 0)
    ImageDraw.Draw(rows).rectangle([0, BAND[0] * SS, N * SS - 1, BAND[1] * SS - 1], fill=255)
    on_band = ImageChops.multiply(t, rows)
    cv.img.paste(Image.new('RGB', cv.img.size, rgb(field_col)), (0, 0), ImageChops.subtract(t, on_band))
    cv.img.paste(Image.new('RGB', cv.img.size, rgb(band_col)), (0, 0), on_band)


def mark_box():
    bb = mark_mask().getbbox()
    return tuple(v / SS for v in bb)


def _hash(i, salt):
    x = (i * 2654435761 + salt * 40503) & 0xFFFFFFFF
    x ^= x >> 15
    x = (x * 2246822519) & 0xFFFFFFFF
    x ^= x >> 13
    return (x & 0xFFFF) / 65536.0


def cleared(x, y, w, extra=()):
    for (a, b, c, d) in list(CLEAR) + list(extra):
        if x + w > a and x < c and y + w > b and y < d:
            return True
    return False


def scatter(cv, col='PURPLE', angle=0.8, density=1.0, pitch=8, extra=(), vignette=False):
    """The identity's halftone scatter, static, kept off the band and every element."""
    mb = mark_box()
    ex = list(extra) + [(mb[0] - M, mb[1] - M, mb[2] + M, mb[3] + M)]
    i = 0
    for y in range(-2, W, pitch):
        for x in range(12, W - 6, pitch):
            i += 1
            v, kind = _hash(i, 1), _hash(i, 2)
            yy = y if y < 318 else y + 2
            if 196 < yy + 6 and yy < 320:
                continue
            dx, dy = x + 3 - 233, yy + 3 - 233
            r = math.hypot(dx, dy)
            if r > 226:
                continue
            if cleared(x, yy, 6, ex):
                continue
            th = math.atan2(dy, dx)
            if vignette:                                   # rim only: nothing inside r 150
                radial = min(1.0, max(0.0, (r - 150) / 70)) ** 1.3
                turn = 0.55 + 0.45 * math.cos(th - angle)
            else:
                radial = 0.35 + 0.65 * min(1.0, max(0.0, (r - 60) / 160))
                turn = 0.4 + 0.6 * math.cos(th - angle)
            if v < radial * turn * density:
                if kind < 0.6:
                    cv.rect(x, yy, x + 6, yy + 6, col); cv.rect(x + 2, yy + 2, x + 4, yy + 4, 'BLACK')
                else:
                    cv.rect(x + 1, yy + 1, x + 5, yy + 5, col)


def hour_angle(h, m):
    """A 24-hour dial with noon at the top, stepping once an hour: 15 degrees an hour."""
    deg = (h - 12) * 15                      # 0 at noon, clockwise
    return math.radians(deg - 90)            # screen angle (0 = right, clockwise positive)


def hatch_field(cv, col='PURPLE', pitch=12, stripe=3):
    """45-degree hatch over the field wherever it is clear of the elements and the band."""
    import numpy as np
    n = W * SS
    ys, xs = np.mgrid[0:n, 0:n]
    x, y = xs / SS, ys / SS
    r = np.hypot(x - 233, y - 233)
    on = (((x - 233) + (y - 233)) / np.sqrt(2)) % pitch < stripe
    keep = on & (r < 226) & ~((y > 192) & (y < 324))
    mb = mark_box()
    for (a, b, c, d) in list(CLEAR) + [(mb[0] - M, mb[1] - M, mb[2] + M, mb[3] + M)]:
        keep &= ~((x > a) & (x < c) & (y > b) & (y < d))
    m = Image.fromarray((keep * 255).astype('uint8'))
    cv.img.paste(Image.new('RGB', cv.img.size, rgb(col)), (0, 0), m)


def grid_marks(cv, col='GRAY', pitch=29):
    """Registration crosses on a square grid centred on the panel, where clear."""
    mb = mark_box()
    ex = [(mb[0] - M, mb[1] - M, mb[2] + M, mb[3] + M)]
    for gy in range(-8, 9):
        for gx in range(-8, 9):
            x, y = 233 + gx * pitch, 233 + gy * pitch
            if math.hypot(x - 233, y - 233) > 218 or 190 < y < 326:
                continue
            if cleared(x - 4, y - 4, 9, ex):
                continue
            cv.rect(x - 4, y, x + 5, y + 1, col)
            cv.rect(x, y - 4, x + 1, y + 5, col)


GNSS_G = '00100 01010 10101 01010 00100'
SATS = (7, 12)                      # fixture: satellites in use / in view (grade 1 data)
BAT = 87                            # fixture: battery percent (grade 1 data)


def mark15(cv, x, y, col):
    import marks_frame as MF
    for j, r in enumerate(MF.ROW['L1']):
        for i, b in enumerate(r):
            if b == '1':
                cv.rect(x + i, y + j, x + i + 1, y + j + 1, col)
    return x + 15


def micro_top(cv, f, col='LIME', top=48):
    """The identity's row, cut down for the top cap: the mark, UTC in pixel digits, the GNSS glyph,
    satellites in use / in view. Centred on x 233."""
    from identity_f import pix_digits, glyph

    def run(cv, x):
        x = mark15(cv, x, top, col) + 10
        x = pix_digits(cv, f.get('utc', '12:07').replace(':', ' '), x, top, col=col) + 10
        x = glyph(cv, GNSS_G, x, top, col=col) + 8
        cv.text('%02d/%02d' % SATS, 'mono', 14, col, x=x, y=top + 3)
        return x + font('mono', 14).getlength('00/00') / SS
    end = run(Canvas(), 0)
    run(cv, round(233 - end / 2))


def micro_band(cv, f, col='BLACK'):
    """Two microtext lines in the band's free rows, between LOCAL and the seconds."""
    cv.text('UTC %s' % f.get('utc', '12:07'), 'mono', 14, col, x=262, y=238)
    b = f.get('bat', BAT)
    t = 'BAT --' if b is None else ('BAT %d%%' % b)
    if f.get('charging'):
        t += ' CHG'
    cv.text(t, 'mono', 14, col, x=262, y=256)


def face(o, f=FIX):
    """o: band, hours, ring, mark_field, mark_band, date, zone, bg, bg_col, bg_angle, bg_density,
    icon (colour), seconds, label."""
    cv = Canvas()
    ring = o.get('ring', 'GRAY')
    if o.get('bg'):
        ang = o.get('bg_angle', 0.8)
        if o['bg'] == 'scatter24':
            ang = hour_angle(f['h'], f['m'])
        if o['bg'] in ('scatter', 'scatter24', 'vignette', 'vignette24'):
            if o['bg'] == 'vignette24':
                ang = hour_angle(f['h'], f['m'])
            scatter(cv, o.get('bg_col', 'PURPLE'), ang, o.get('bg_density', 1.0),
                    vignette=o['bg'].startswith('vignette'))
        elif o['bg'] == 'hatch':
            hatch_field(cv, o.get('bg_col', 'PURPLE'))
        elif o['bg'] == 'grid':
            grid_marks(cv, o.get('bg_col', 'GRAY'))
    cv.ring(230, 232, ring)
    digits(cv, '%02d' % f['h'], BIG, o.get('hours', 'WHITE'), X0, HOURS_BASE)
    icon_sized(cv, 262, 90, o.get('icon', 'BLUE'), SYM['gnss'], module=16, pad=8)
    band(cv, BAND[0], BAND[1], o.get('band', 'WHITE'))
    ink = o.get('band_ink', 'BLACK')
    cells_text(cv, 'LOCAL', 'bold', 16, o.get('label', ink), pen_for('LOCAL', 'bold', 16, x=262), 225)
    digits(cv, '%02d' % f['m'], BIG, ink, X0, MIN_BASE)
    digits(cv, '%02d' % f['s'], SEC_PX, o.get('seconds', ink), SEC_PEN, MIN_BASE)
    cv.text(f['date'], 'mono', 23, o.get('date', 'WHITE'), cx=233, y=334)
    plate(cv, [('tag', f['mode']), ('cell', f['abbr']), ('cell', f['off'])], 360)
    cells_text(cv, f['zone'], 'mono', 14, o.get('zone', 'GRAY'), pen_for(f['zone'], 'mono', 14, cx=233), 398)
    wordmark(cv, o.get('mark_field', 'WHITE'), o.get('mark_band', ink))
    if o.get('micro_top'):
        micro_top(cv, f, o.get('micro_col', 'LIME'))
    if o.get('micro_band'):
        micro_band(cv, f, ink)
    return finish(cv)


VARIANTS = [
    ('A', 'AS BUILT', dict()),
    ('B', 'LIME BAND', dict(band='LIME')),
    ('C', 'LIME BAND, LIME WORDMARK', dict(band='LIME', mark_field='LIME')),
    ('D', 'LIME HOURS, WHITE BAND', dict(hours='LIME', mark_field='LIME')),
    ('E', 'GREEN BAND (BOARD #01E67C)', dict(band=GREEN)),
    ('F', 'LIME BAND, PURPLE SCATTER', dict(band='LIME', mark_field='LIME', bg='scatter')),
    ('G', 'LIME BAND, SCATTER AS A 24-HOUR HAND', dict(band='LIME', mark_field='LIME', bg='scatter24')),
    ('H', 'WHITE BAND, PURPLE SCATTER', dict(bg='scatter')),
    ('I', 'LIME BAND, VIOLET SCATTER (BOARD #B32BE5)', dict(band='LIME', mark_field='LIME', bg='scatter', bg_col=VIOLET, bg_density=0.8)),
]


def sheet(path, variants, cols=3):
    f = ImageFont.truetype('fonts/MonoB.otf', 16)
    Wc, Hh = W + 24, W + 48
    rows = (len(variants) + cols - 1) // cols
    sh = Image.new('RGB', (cols * Wc + 24, rows * Hh + 24), (20, 20, 22))
    d = ImageDraw.Draw(sh)
    for i, (k, name, o) in enumerate(variants):
        x, y = 24 + (i % cols) * Wc, 24 + (i // cols) * Hh
        im = face(o)
        sh.paste(im, (x, y))
        im.save('out/clock-colour-%s.png' % k)
        d.text((x, y + W + 8), '%s  %s' % (k, name), font=f, fill=(210, 211, 214))
    sh.save(path)


MICRO = [
    ('P', 'TOP ROW: MARK, UTC, GNSS, SATELLITES', dict(band='LIME', mark_field='LIME', micro_top=True)),
    ('Q', 'IN THE BAND: UTC AND BATTERY', dict(band='LIME', mark_field='LIME', micro_band=True)),
    ('R', 'BOTH, WITH THE RIM SCATTER AS A 24-HOUR HAND', dict(band='LIME', mark_field='LIME', micro_top=True,
                                                            micro_band=True, bg='vignette24', bg_density=1.3)),
]

BACKGROUNDS = [
    ('J', 'LIME BAND, SCATTER AT THE RIM ONLY', dict(band='LIME', mark_field='LIME', bg='vignette', bg_density=1.2)),
    ('K', 'LIME BAND, RIM SCATTER AS A 24-HOUR HAND', dict(band='LIME', mark_field='LIME', bg='vignette24', bg_density=1.3)),
    ('L', 'LIME BAND, PURPLE HATCH ON THE FIELD', dict(band='LIME', mark_field='LIME', bg='hatch')),
    ('M', 'LIME BAND, REGISTRATION CROSSES', dict(band='LIME', mark_field='LIME', bg='grid')),
    ('N', 'LIME BAND, PURPLE REGISTRATION CROSSES', dict(band='LIME', mark_field='LIME', bg='grid', bg_col='PURPLE')),
    ('O', 'WHITE BAND, LIME RIM SCATTER', dict(bg='vignette', bg_col='LIME', bg_density=1.0)),
]


if __name__ == '__main__':
    import sys
    if 'micro' in sys.argv:
        sheet('out/clock-microtext.png', MICRO)
    elif 'bg' in sys.argv:
        sheet('out/clock-backgrounds.png', BACKGROUNDS)
    else:
        sheet('out/clock-colour.png', VARIANTS)
