"""Clock face, round 4: the chosen treatment, every state and the entry.

Chosen with the owner on 25 Sep:
- the built layout; the band LIME in the normal states (ORANGE stopped, WHITE no zone, RED no
  data); the wordmark LIME off the band
- the PURPLE scatter under everything, thinner toward the centre, with a 2 px BLACK halo round
  every element; fills are solid ground, so knockout stays BLACK
- microtext in the band: UTC and battery; the band's right end a battery column (LIME hatch is
  the charge, BLACK the rest; ORANGE at 15 % or under; GRAY hatch at full height while unknown;
  the hatch crawls up while charging)
- the lower block as tokens: `THU 24 SEP 2026  [AUTO]` in LIME, `IST +01:00 // EUROPE/DUBLIN`
  in GRAY
"""
import lib
lib.RECORD = False
from lib import *
from PIL import ImageFilter
import numpy as np
from face import SYM, X0, BIG, HOURS_BASE, MIN_BASE, SEC_PEN, SEC_PX
from industrial import digits, cells_text, pen_for, BAND
from identity import finish
from identity_g import frame_rect, hatch
import clockcolor as CC
import clock4 as C4
import clock5 as C5
import clock6 as C6
from anim import ramp, save_gif

W = 466
LOW = 15
LINE2_MAX = 320        # px: the second line's widest; past it the name takes a third line
F = dict(C6.F, bat=87, charging=False)

STATES = {
    #          band      label      icon      glyph        ring
    'gnss':   ('LIME',   'LOCAL',   'BLUE',   'gnss',     'GRAY'),
    'rtc':    ('LIME',   'LOCAL',   'GRAY',   'rtc',      'GRAY'),
    'manual': ('LIME',   'LOCAL',   'BLUE',   'gnss',     'GRAY'),
    'stopped': ('ORANGE', 'STOPPED', 'ORANGE', 'stopped', 'GRAY'),
    'nozone': ('WHITE',  'NO ZONE', 'WHITE',  'nozone',   'GRAY'),
    'nodata': ('RED',    'CLOCK',   'RED',    'nodata',   'RED'),
}


BAT_ON_NODATA = 'WHITE'      # the battery is not faulted when the clock is: no RED for it


def battery_col(cv, pct, charging=False, phase=0, k=1.0, band_col='LIME'):
    """k: the entry's build (the hatch draws up from the bottom; may pass 1 with an overshoot).
    The hatch takes the band's colour; ORANGE at LOW % or under; GRAY while unknown."""
    x0, x1, y0, y1 = C5.SEC_COL
    cv.rect(x0, y0, x1, y1, 'BLACK')
    ix0, ix1, iy0, iy1 = x0 + 3, x1 - 3, y0 + 3, y1 - 3
    if pct is None:
        col, h = 'GRAY', iy1 - iy0
    else:
        col = 'ORANGE' if pct <= LOW else (BAT_ON_NODATA if band_col == 'RED' and BAT_ON_NODATA else band_col)
        h = round((iy1 - iy0) * pct / 100)
    frame_rect(cv, x0 + 1, y0 + 1, x1 - 1, y1 - 1, col, 1)
    h = min(iy1 - iy0, round(h * k))
    if h > 0:
        hatch(cv, ix0, iy1 - h, ix1, iy1, col, pitch=8, w=4, phase=-phase if charging else 0)


def band_lines(cv, state, f, k=1.0):
    """UTC and battery, BLACK in the band's free rows. Typed in by cell reveal on entry."""
    utc = 'UTC %s' % f['utc'] if state not in ('stopped', 'nodata') else 'UTC --:--'
    b = f.get('bat')
    bat = 'BAT --' if b is None else 'BAT %d%%' % b
    if f.get('charging'):
        bat += ' CHG'
    cells_text(cv, utc, 'mono', 14, 'BLACK', 262, 238 + cap_height('mono', 14), k)
    cells_text(cv, bat, 'mono', 14, 'BLACK', 262, 256 + cap_height('mono', 14), k)


def tok_line(cv, segs, y, px, k=1.0):
    """C6.tokens with a cell reveal: k of the characters shown, a block ahead of them."""
    if k >= 1:
        C6.tokens(cv, segs, y, px=px)
        return
    fnt = {n: font(n, px) for n in ('mono', 'bold')}
    widths = [fnt[fc].getlength(t) / SS for t, fc, _ in segs]
    x = 233 - sum(widths) / 2
    base = y + cap_height('mono', px)
    total = sum(len(t) for t, _, _ in segs)
    shown = k * (total + 1)
    n = 0
    for (t, fc, col), w in zip(segs, widths):
        cells_text(cv, t, fc, px, col, x, base, max(0.0, min(1.0, (shown - n) / max(1, len(t)))))
        n += len(t)
        x += w


def lower(cv, state, f, kd=1.0, kz=1.0):
    L, G = 'LIME', 'GRAY'
    mode = [('  [', 'mono', L), (f['mode'], 'bold', L), (']', 'mono', L)]
    if state == 'stopped':
        l1 = [('WAITING FOR GNSS', 'mono', G)]
    elif state == 'nozone':
        l1 = [('NO FIX YET', 'mono', G)] + [(a, b, G) for a, b, _ in mode]
    elif state == 'nodata':
        l1 = None
    else:
        l1 = [('%s ' % f['wd'], 'bold', L), ('%s %s %s' % (f['day'], f['mon'], f['year']), 'mono', L)] + mode
    if l1:
        tok_line(cv, l1, 340, 19, kd)
    if state == 'nozone':
        l2 = None                       # no zone, so no name line (as built)
    else:
        l2 = [('%s %s ' % (f['abbr'], f['off']), 'mono', G), ('// %s' % f['zone'], 'mono', G)]
    if l2:
        w = sum(font(fc, 14).getlength(t) for t, fc, _ in l2) / SS
        if w > LINE2_MAX:                  # a long name moves to a third line
            tok_line(cv, l2[:1], 368, 14, kz)
            tok_line(cv, l2[1:], 386, 14, kz)
        else:
            tok_line(cv, l2, 368, 14, kz)


def face(state='gnss', f=F, e=None, scatter_k=1.0):
    """e: the built entry's accents (anim.entry), plus 'bat' and 'lines' for the new parts."""
    e = e or dict(ring=1, icon_fade=1, icon_rows=5, label=1, plate=1, zone=1, mark=1)
    band_col, label, icon_col, glyph, ring = STATES[state]
    if state == 'manual':
        f = dict(f, mode='MANUAL')
    ground = [(0, BAND[0], W, BAND[1]), (262, 90, 358, 186)]
    withheld = state in ('stopped', 'nozone')

    def draw(cv):
        if state != 'nodata':
            digits(cv, '--' if withheld else '%02d' % f['h'], BIG, 'WHITE', X0, HOURS_BASE)
        if e['icon_rows'] > 0:
            icon_sized(cv, 262, 90, icon_col, SYM[glyph], module=16, pad=8, rows_shown=e['icon_rows'])
        band(cv, BAND[0], BAND[1], band_col)
        cells_text(cv, label, 'bold', 16, 'BLACK', pen_for(label, 'bold', 16, x=262), 225, e['label'])
        if state == 'nodata':
            cv.text('NO DATA', 'shapiro', 28, 'BLACK', x=71, cy=257.5)
        else:
            digits(cv, '--' if withheld else '%02d' % f['m'], BIG, 'BLACK', X0, MIN_BASE)
            digits(cv, '--' if withheld else '%02d' % f['s'], SEC_PX, 'BLACK', SEC_PEN, MIN_BASE)
            band_lines(cv, state, f, e['label'])
        battery_col(cv, f.get('bat'), f.get('charging'), f.get('phase', 0), e.get('bat', 1.0), band_col)
        if e.get('mark', 1) > 0:
            CC.wordmark(cv, band_col, 'BLACK')
        lower(cv, state, f, e['plate'], e['zone'])
    base = Canvas()
    if scatter_k > 0:
        C4.scatter(base, 0.8, zone=(0, -1), density=0.8 * scatter_k, centre=0.15)
    top = Canvas()
    draw(top)
    arr = np.array(top.img)
    m = Image.fromarray(((arr.sum(axis=2) > 0) * 255).astype('uint8'))
    d = ImageDraw.Draw(m)
    for (a, b, c, g) in ground:
        d.rectangle([a * SS, b * SS, c * SS - 1, g * SS - 1], fill=255)
    base.img.paste((0, 0, 0), (0, 0), m.filter(ImageFilter.MaxFilter(int(2 * SS) * 2 + 1)))
    base.img.paste(top.img, (0, 0), m)
    if e['ring'] > 0:
        base.ring(230, 232, fade(ring, e['ring']))
    return finish(base)


def out_cubic(u): return 1 - (1 - u) ** 3


def out_back(u):
    c1 = 1.70158
    return 1 + (c1 + 1) * (u - 1) ** 3 + c1 * (u - 1) ** 2


def in_quad(u): return u * u
def in_expo(u): return 0.0 if u <= 0 else (1.0 if u >= 1 else 2 ** (10 * u - 10))


def _u(t, start, dur):
    return max(0.0, min(1.0, (t - start) / dur))


# the entry, chosen 25 Sep: M6 (overshooting fills, snapping wordmark) with M5's late scatter
ENTRY = [  # element, start ms, duration ms, curve
    ('scatter', 0, 120, in_quad),
    ('ring', 0, 110, out_back),
    ('icon', 60, 150, out_cubic),
    ('label', 100, 120, out_cubic),       # the band label and the band's two lines
    ('bat', 160, 80, out_back),
    ('plate', 160, 120, out_cubic),       # the lower block's first line
    ('zone', 240, 160, out_cubic),        # the lower block's second line
    ('mark', 300, 160, in_expo),
]


def entry(t):
    e = dict(icon_fade=1.0)
    sk = 0.0
    for name, start, dur, fn in ENTRY:
        k = fn(_u(t, start, dur)) if t >= start else 0.0
        if name == 'scatter':
            sk = k
        elif name == 'icon':
            e['icon_rows'] = int(round(k * 5))
        else:
            e[name] = k
    return e, sk


NAMES = [('gnss', 'LOCAL, GNSS'), ('rtc', 'LOCAL, RTC'), ('manual', 'LOCAL, MANUAL'),
         ('stopped', 'STOPPED'), ('nozone', 'NO ZONE'), ('nodata', 'NO DATA')]


if __name__ == '__main__' and not __import__('sys').argv[1:]:
    fnt = ImageFont.truetype('fonts/MonoB.otf', 16)
    Wc, Hh = W + 24, W + 48
    sh = Image.new('RGB', (3 * Wc + 24, 2 * Hh + 24), (20, 20, 22))
    dr = ImageDraw.Draw(sh)
    for i, (st, name) in enumerate(NAMES):
        im = face(st)
        im.save('out/clock4-state-%s.png' % st)
        x, y = 24 + (i % 3) * Wc, 24 + (i // 3) * Hh
        sh.paste(im, (x, y))
        dr.text((x, y + W + 8), name, font=fnt, fill=(210, 211, 214))
    sh.save('out/clock4-states.png')
    fr = [finish(Canvas())] * 5
    for t in range(0, 500, 20):
        e, sk = entry(t)
        fr.append(face('gnss', e=e, scatter_k=sk))
    fr += [fr[-1]] * 40
    save_gif(fr, 'out/clock4-entry.gif', 20)
    save_gif(fr, 'out/clock4-entry-slow3x.gif', 60)
    sc = 0.4
    w = int(W * sc)
    ts = [0, 40, 80, 120, 180, 240, 320, 460]
    st = Image.new('RGB', (len(ts) * (w + 8) + 8, w + 36), (20, 20, 22))
    ds = ImageDraw.Draw(st)
    for i, t in enumerate(ts):
        e, sk = entry(t)
        st.paste(face('gnss', e=e, scatter_k=sk).resize((w, w), Image.LANCZOS), (8 + i * (w + 8), 8))
        ds.text((8 + i * (w + 8), w + 12), '%d MS' % t, font=ImageFont.truetype('fonts/MonoR.otf', 13), fill=(210, 211, 214))
    st.save('out/clock4-entry-strip.png')


def battery_sheet(path):
    shots = [(dict(F, bat=87), '87 %'), (dict(F, bat=40, charging=True, phase=4), 'CHARGING, 40 %'),
             (dict(F, bat=12), '12 %: ORANGE'), (dict(F, bat=None), 'UNKNOWN: GRAY, FULL HEIGHT')]
    fnt = ImageFont.truetype('fonts/MonoB.otf', 15)
    Wc, Hh = W + 24, W + 48
    sh = Image.new('RGB', (4 * Wc + 24, Hh + 24), (20, 20, 22))
    dr = ImageDraw.Draw(sh)
    for i, (f, name) in enumerate(shots):
        im = face('gnss', f=f)
        sh.paste(im, (24 + i * Wc, 24))
        dr.text((24 + i * Wc, 24 + W + 8), name, font=fnt, fill=(210, 211, 214))
    sh.save(path)
    fr = [face('gnss', f=dict(F, bat=40, charging=True, phase=p)).crop((233, 150, 466, 390)) for p in range(8)]
    save_gif(fr * 4, 'out/clock4-charging.gif', 20)


if __name__ == '__main__' and __import__('sys').argv[1:] == ['final']:
    battery_sheet('out/clock4-battery.png')
    fr = [finish(Canvas())] * 5
    for t in range(0, 500, 20):
        e, sk = entry(t)
        fr.append(face('gnss', e=e, scatter_k=sk))
    fr += [fr[-1]] * 40
    save_gif(fr, 'out/clock4-entry.gif', 20)
    save_gif(fr, 'out/clock4-entry-slow3x.gif', 60)
    sc = 0.4
    w = int(W * sc)
    ts = [0, 40, 80, 120, 160, 200, 240, 300, 380, 460]
    st = Image.new('RGB', (len(ts) * (w + 8) + 8, w + 36), (20, 20, 22))
    ds = ImageDraw.Draw(st)
    for i, t in enumerate(ts):
        e, sk = entry(t)
        st.paste(face('gnss', e=e, scatter_k=sk).resize((w, w), Image.LANCZOS), (8 + i * (w + 8), 8))
        ds.text((8 + i * (w + 8), w + 12), '%d MS' % t, font=ImageFont.truetype('fonts/MonoR.otf', 13), fill=(210, 211, 214))
    st.save('out/clock4-entry-strip.png')


if __name__ == '__main__' and __import__('sys').argv[1:] == ['long']:
    f = dict(F, zone='AMERICA/ARGENTINA/BUENOS_AIRES', abbr='ART', off='-03:00', wd='WED', day='30', mode='MANUAL')
    face('manual', f=f).save('out/clock4-state-longest.png')
