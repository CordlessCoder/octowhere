"""Industrial pass on B, and pushed versions of the extreme prototypes.

Shared devices, all carrying real data or real labels:
- the 5x5 symbol at 96 px, filling the hours' cap height
- a zone plate: mode as a filled tag, abbreviation and offset in ruled cells
- field labels knocked out of the band
Every function takes `a`, a dict of animation parameters, so the same drawing code
renders still frames and the entry and exit sequences.
"""
from lib import *
from face import SYM, X0, BIG, HOURS_BASE, MIN_BASE, SEC_PEN, SEC_PX

FIX = dict(h=13, m=7, s=42, date='THU 24 SEP 2026', mode='AUTO', abbr='IST', off='+01:00', zone='EUROPE/DUBLIN')
ICON96 = dict(module=16, pad=8)          # 96 x 96: rows 90..185, the hours' cap height
BAND = (198, 318)
FULL = dict(ring=1.0, icon=25, labels=1.0, plate=1.0, rule=1.0)   # everything shown


def digits(cv, s, px, col, x0, base, name=None, clip=None):
    a = adv('bold', px)
    f = font('bold', px)

    def draw(dd, fill):
        for i, ch in enumerate(s):
            dd.text(((x0 + i * a) * SS, base * SS), ch, font=f, fill=fill, anchor='ls')
    draw(cv.d, rgb(col))
    cv._record(name, lambda dd: draw(dd, 255))


# ---- block-reveal text ---------------------------------------------------------
def cells_text(cv, s, face, px, col, pen_x, base, k=1.0, block=None):
    """Mono text on its character cells. k in 0..1 reveals left to right:
    each cell shows a solid block for one step before its glyph. Blocks never
    show a wrong character, only where one will be."""
    a = adv(face, px)
    ch_h = cap_height(face, px)
    n = len(s)
    shown = k * (n + 1)          # cells < shown-1 are glyphs, the cell at shown-1 is a block
    f = font(face, px)
    for i, c in enumerate(s):
        if c == ' ':
            continue
        x = pen_x + i * a
        if i < shown - 1:
            cv.d.text((x * SS, base * SS), c, font=f, fill=rgb(col), anchor='ls')
        elif i < shown:
            cv.rect(round(x + 1), round(base - ch_h), round(x + a - 1), round(base), block or col)


def pen_for(s, face, px, cx=None, x=None):
    """Pen x that puts the ink left at x, or centres the ink on cx."""
    w, _ = Canvas().ink_size(s, face, px)
    left = x if x is not None else cx - w / 2
    return left - lsb(s, face, px)


# ---- zone plate ----------------------------------------------------------------
def plate(cv, cells, top, k=1.0, h=20):
    """Cells centred on x 233. ('tag', text) is filled GRAY with BLACK text;
    ('cell', text) is a 1 px GRAY frame with WHITE text. k reveals cells in order."""
    px, padx, gap = 14, 6, 4
    ws = [adv('mono', px) * len(t) + 2 * padx for _, t in cells]
    x = round(233 - (sum(ws) + gap * (len(ws) - 1)) / 2)
    ch = cap_height('mono', px)
    base = top + (h + ch) / 2
    for i, ((kind, t), w) in enumerate(zip(cells, ws)):
        w = round(w)
        if i >= k * len(cells):
            break
        if kind == 'tag':
            cv.rect(x, top, x + w, top + h, 'GRAY')
            cells_text(cv, t, 'bold', px, 'BLACK', x + padx, base)
        else:
            cv.rect(x, top, x + w, top + h, 'GRAY')
            cv.rect(x + 1, top + 1, x + w - 1, top + h - 1, 'BLACK')
            cells_text(cv, t, 'mono', px, 'WHITE', x + padx, base)
        x += w + gap


def icon_build(cv, x, y, tile, sym, n, **kw):
    """n in 0..25+: tile appears at n>=1, then black modules in reading order."""
    if n <= 0:
        return
    total = sym.count('1')
    icon_sized(cv, x, y, tile, sym, shown=set(range(min(total, max(0, n - 1)))), **kw)


# ---- V1: B, industrial -----------------------------------------------------------
def v1(state='gnss', f=FIX, a=FULL):
    cv = Canvas()
    fault = state == 'nodata'
    known = state in ('gnss', 'rtc')
    ring = 'RED' if fault else 'GRAY'
    if a['ring'] > 0:
        cv.ring(230, 232, fade(ring, a['ring']), name='ring')
    tile = {'gnss': 'BLUE', 'rtc': 'GRAY', 'stopped': 'ORANGE', 'nozone': 'WHITE', 'nodata': 'RED'}[state]
    band = {'stopped': 'ORANGE', 'nodata': 'RED'}.get(state, 'WHITE')

    if known:
        digits(cv, '%02d' % f['h'], BIG, 'WHITE', X0, HOURS_BASE, name='hours')
    elif not fault:
        digits(cv, '--', BIG, 'WHITE', X0, HOURS_BASE, name='hours')
    icon_build(cv, 262, 90, tile, SYM[state], a['icon'], **ICON96)

    clipped_band(cv, BAND[0], BAND[1], band, name='band')
    label = {'gnss': 'LOCAL', 'rtc': 'LOCAL', 'stopped': 'STOPPED', 'nozone': 'NO ZONE', 'nodata': 'CLOCK'}[state]
    lface = 'bold'
    cells_text(cv, label, lface, 16, 'BLACK', pen_for(label, lface, 16, x=262), 225, a['labels'])
    if fault:
        cv.text('NO DATA', 'shapiro', 28, 'BLACK', x=71, cy=257.5, name='nodata')
    else:
        digits(cv, '%02d' % f['m'] if known else '--', BIG, 'BLACK', X0, MIN_BASE, name='minutes')
        if known:
            digits(cv, '%02d' % f['s'], SEC_PX, 'BLACK', SEC_PEN, MIN_BASE, name='seconds')

    if known:
        cv.text(f['date'], 'mono', 23, 'WHITE', cx=233, y=334, name='date')
    elif state == 'stopped':
        cv.text('WAITING FOR GNSS', 'mono', 19, 'GRAY', cx=233, y=336, name='date')
    elif state == 'nozone':
        cv.text('UTC 12:07', 'mono', 23, 'GRAY', cx=233, y=334, name='date')
    if state == 'nozone':
        plate(cv, [('tag', 'AUTO'), ('cell', 'NO FIX YET')], 360, a['plate'])
    else:
        plate(cv, [('tag', f['mode']), ('cell', f['abbr']), ('cell', f['off'])], 360, a['plate'])
        if a['plate'] >= 1:
            z = f['zone']
            cells_text(cv, z, 'mono', 14, 'GRAY', pen_for(z, 'mono', 14, cx=233), 398, a['labels'])
    return cv


# ---- V2: crop, industrial ----------------------------------------------------------
def v2(f=FIX, a=FULL):
    """Crop: digits at 230 px; a metadata rail on the field between hours and band."""
    cv = Canvas()
    px = 230
    ad = adv('bold', px)
    x0 = 233 - ad - 2
    digits(cv, '%02d' % f['h'], px, 'WHITE', x0, 198)
    # rail rows 206..229 on the field
    icon_build(cv, 40, 206, 'BLUE', SYM['gnss'], a['icon'], module=4, pad=2)
    cells_text(cv, 'LOCAL', 'bold', 14, 'GRAY', 72, 223, a['labels'])
    cells_text(cv, '%s %s' % (f['abbr'], f['off']), 'mono', 14, 'GRAY', 132, 223, a['labels'])
    cv.rect(368, 206, 426, 230, 'WHITE')
    cv.text('%02d' % f['s'], 'bold', 22, 'BLACK', cx=397, cy=218)
    cv.rect(0, 236, N, N, 'WHITE')
    digits(cv, '%02d' % f['m'], px, 'BLACK', x0, 410)
    if a['ring'] > 0:
        cv.ring(230, 232, fade('GRAY', a['ring']))
    return cv


# ---- V3: wipe, industrial ----------------------------------------------------------
def v3(f=FIX, a=FULL, s=None):
    s = f['s'] if s is None else s
    cv = Canvas()
    if a['ring'] > 0:
        cv.ring(230, 232, fade('GRAY', a['ring']))
    digits(cv, '%02d' % f['h'], BIG, 'WHITE', X0, HOURS_BASE)
    icon_build(cv, 262, 90, 'BLUE', SYM['gnss'], a['icon'], **ICON96)
    edge = round(N * (s + 1) / 60)

    def band(c, filled):
        fg = 'BLACK' if filled else 'WHITE'
        if filled:
            c.rect(0, BAND[0], N, BAND[1], 'WHITE')
        else:
            c.rect(0, BAND[0], N, BAND[1], fade('GRAY', 0.35))
            c.rect(0, BAND[0] + 2, N, BAND[1] - 2, 'BLACK')
        # scale: a tick every 5 s on the band's top edge, a longer one every 15 s
        for k in range(12):
            x = round(N * k / 12)
            c.rect(x, BAND[0], x + 2, BAND[0] + (12 if k % 3 == 0 else 6), fg if filled else 'GRAY')
        digits(c, '%02d' % f['m'], BIG, fg, X0, MIN_BASE)
        digits(c, '%02d' % s, SEC_PX, fg, SEC_PEN, MIN_BASE)
        cells_text(c, 'LOCAL', 'bold', 16, fg if filled else 'GRAY', pen_for('LOCAL', 'bold', 16, x=262), 225, a['labels'])
    band(cv, False)
    fill = Canvas()
    band(fill, True)
    cv.img.paste(fill.img.crop((0, BAND[0] * SS, edge * SS, BAND[1] * SS)), (0, BAND[0] * SS))
    cv.text(f['date'], 'mono', 23, 'WHITE', cx=233, y=334)
    plate(cv, [('tag', f['mode']), ('cell', f['abbr']), ('cell', f['off'])], 360, a['plate'])
    if a['plate'] >= 1:
        cells_text(cv, f['zone'], 'mono', 14, 'GRAY', pen_for(f['zone'], 'mono', 14, cx=233), 398, a['labels'])
    return cv


# ---- V4: display face, industrial ----------------------------------------------------
def v4(f=FIX, a=FULL):
    cv = Canvas()
    if a['ring'] > 0:
        cv.ring(230, 232, fade('GRAY', a['ring']))
    icon_build(cv, 200, 64, 'BLUE', SYM['gnss'], a['icon'], module=10, pad=8)
    cells_text(cv, 'LOCAL', 'bold', 16, 'GRAY', pen_for('LOCAL', 'bold', 16, x=276), 101, a['labels'])
    cells_text(cv, 'THU 24 SEP', 'bold', 16, 'WHITE', pen_for('THU 24 SEP', 'bold', 16, x=276), 127, a['labels'])
    cv.text('%s %s' % (f['abbr'], f['off']), 'mono', 14, 'GRAY', x=200, y=148)
    cv.rect(0, 166, N, 300, 'WHITE')
    cv.text('%02d:%02d' % (f['h'], f['m']), 'shapiro', 88, 'BLACK', cx=233, cy=233)
    # seconds: 60 cells, 4 px on a 6 px pitch, a taller cell every 15 s
    x = 233 - 30 * 6 + 1
    for k in range(60):
        tall = k % 15 == 0
        cv.rect(x + k * 6, 310, x + k * 6 + 4, 326 if tall else 322, 'WHITE' if k <= f['s'] else fade('GRAY', 0.35))
    for k, lab in enumerate(['00', '15', '30', '45']):
        cv.text(lab, 'mono', 14, 'GRAY', x=x + k * 90, y=333)
    plate(cv, [('tag', f['mode']), ('cell', f['zone'])], 360, a['plate'])
    return cv


if __name__ == '__main__':
    ims = []
    for n, fn in [('v1-industrial', lambda: v1()), ('v2-crop', lambda: v2()), ('v3-wipe', lambda: v3(s=17)),
                  ('v4-display', lambda: v4())]:
        ims.append(fn().save('out/ind-%s.png' % n))
    sh = Image.new('RGB', (486 * 4 - 20, 466), (28, 28, 28))
    for i, im in enumerate(ims):
        sh.paste(im, (i * 486, 0))
    sh.save('out/sheet-industrial.png')
    st = [v1(s).save('out/ind-v1-%s.png' % s) for s in ['gnss', 'rtc', 'stopped', 'nozone', 'nodata']]
    sh = Image.new('RGB', (486 * 5 - 20, 466), (28, 28, 28))
    for i, im in enumerate(st):
        sh.paste(im, (i * 486, 0))
    sh.save('out/sheet-industrial-v1-states.png')
