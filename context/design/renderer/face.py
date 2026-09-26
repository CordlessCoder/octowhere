"""Clock face, direction B: every state, from one layout table."""
from lib import *

# ---- layout (panel px) ------------------------------------------------------
X0 = 62                 # pen of the first digit of hours and minutes
BIG = 136               # Fraktion Mono Bold, hours and minutes
HOURS_BASE = 184
BAND = (198, 318)       # rows 198..317, full chord
MIN_BASE = 305
COL = 262               # right column ink left
ICON_XY = (262, 90)
CAPTION_TOP = 131
SEC_PX = 40
SEC_PEN = 260          # pen of the first seconds digit; ink starts at the column
DATE_TOP = 334
ZONE1_TOP = 361
ZONE2_TOP = 377

# ---- symbols: 5x5 rows, 1 = black module --------------------------------------
SYM = {
    'gnss':    '00100 01010 10101 01010 00100',   # set from GNSS since boot
    'rtc':     '11111 10001 10101 10001 11111',   # running on the RTC, not confirmed since boot
    'stopped': '01010 01010 01010 01010 01010',   # clock stopped since last set
    'nozone':  '01110 10001 00110 00000 00100',   # automatic, no zone found yet
    'nodata':  '11100 11000 00100 00011 00111',   # canonical, shared with the compass
}

FIX = dict(h=13, m=7, s=42, date='THU 24 SEP 2026', abbr='IST', off='+01:00',
           zone='EUROPE/DUBLIN', utc='12:07')


def digits(cv, s, px, col, x0, base, name=None):
    """Tabular digits: glyph i has its pen at x0 + i * advance."""
    a = adv('bold', px)
    f = font('bold', px)
    sw = stroke('bold', px)

    def draw(dd, fill):
        for i, ch in enumerate(s):
            dd.text(((x0 + i * a) * SS, base * SS), ch, font=f, fill=fill, anchor='ls', stroke_width=sw, stroke_fill=fill)
    draw(cv.d, rgb(col))
    cv._record(name, lambda dd: draw(dd, 255))


def face(state='gnss', mode='AUTO', f=FIX, accents=1.0, band_col=None):
    """state: gnss | rtc | stopped | nozone | nodata"""
    cv = Canvas()
    fault = state == 'nodata'
    ring = 'RED' if fault else 'GRAY'
    cv.ring(230, 232, fade(ring, accents), name='ring')

    tile = {'gnss': 'BLUE', 'rtc': 'GRAY', 'stopped': 'ORANGE', 'nozone': 'WHITE', 'nodata': 'RED'}[state]
    band = band_col or {'gnss': 'WHITE', 'rtc': 'WHITE', 'stopped': 'ORANGE', 'nozone': 'WHITE', 'nodata': 'RED'}[state]
    caption = 'CLOCK' if fault else 'LOCAL'
    known = state in ('gnss', 'rtc')

    # hours on the field
    if known:
        digits(cv, '%02d' % f['h'], BIG, 'WHITE', X0, HOURS_BASE, name='hours')
    elif not fault:
        digits(cv, '--', BIG, 'WHITE', X0, HOURS_BASE, name='hours')

    # right column, upper
    icon(cv, *ICON_XY, fade(tile, accents), SYM[state])
    cv.text(caption, 'bold', 16, 'GRAY', x=COL, y=CAPTION_TOP, name='caption')
    if state == 'stopped':
        cv.text('STOPPED', 'bold', 24, 'ORANGE', x=COL, bottom=HOURS_BASE, name='state')
    elif state == 'nozone':
        cv.text('NO ZONE', 'mono', 20, 'GRAY', x=COL, bottom=HOURS_BASE, name='state')

    # band
    cv.rect(0, BAND[0], N, BAND[1], band, name='band')
    if fault:
        cv.text('NO DATA', 'shapiro', 28, 'BLACK', cx=233, cy=(BAND[0] + BAND[1]) / 2, name='nodata')
    else:
        digits(cv, '%02d' % f['m'] if known else '--', BIG, 'BLACK', X0, MIN_BASE, name='minutes')
        if known:
            digits(cv, '%02d' % f['s'], SEC_PX, 'BLACK', SEC_PEN, MIN_BASE, name='seconds')

    # rails
    if known:
        cv.text(f['date'], 'mono', 23, 'WHITE', cx=233, y=DATE_TOP, name='date')
    elif state == 'stopped':
        cv.text('WAITING FOR GNSS', 'mono', 19, 'GRAY', cx=233, y=DATE_TOP + 2, name='date')
    elif state == 'nozone':
        cv.text('UTC ' + f['utc'], 'mono', 23, 'GRAY', cx=233, y=DATE_TOP, name='date')
    if state == 'nozone':
        cv.text('AUTO', 'mono', 14, 'GRAY', cx=233, y=ZONE1_TOP, name='zone1')
        cv.text('WAITING FOR GNSS', 'mono', 14, 'GRAY', cx=233, y=ZONE2_TOP, name='zone2')
    else:
        cv.text('%s  %s  %s' % (mode, f['abbr'], f['off']), 'mono', 14, 'GRAY', cx=233, y=ZONE1_TOP, name='zone1')
        cv.text(f['zone'], 'mono', 14, 'GRAY', cx=233, y=ZONE2_TOP, name='zone2')
    return cv


if __name__ == '__main__':
    cv = face('gnss')
    cv.save('out/b-gnss.png')
    for n, b in cv.log:
        print(n, b)
