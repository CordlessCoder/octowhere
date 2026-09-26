"""Display settings, round 3: two new panel cells and the timeout screen.

The grid grows to four columns, column-major:
  01 ZONE     03 TIMEOUT     05 COMPASS   07 BATTERY
  02 BRIGHT.  04 ALWAYS ON   06 GNSS      08 DEVICE
TIMEOUT opens a one-step list in the picker's grammar. ALWAYS ON toggles on a tap.
"""
import lib
lib.RECORD = False
from lib import *
import panel as P
from picker2 import box, field, BTN, HINT_TOP, ICON_XY
from face import X0, BAND

P.GLYPH['timeout'] = '11111 01110 00100 01110 11111'   # hourglass
P.GLYPH['aod'] = '00000 01110 11011 01110 00000'       # an open eye

TIMEOUTS = ['15 S', '30 S', '1 MIN', '5 MIN', 'NEVER']


def cells8(f=P.FIX):
    six = P._cells6(f)
    t = f.get('timeout', '1 MIN')
    on = f.get('aod', False)
    timeout = ('timeout', 'TIMEOUT', 'timeout', 'WHITE', t, 'WHITE', None)
    aod = ('aod', 'ALWAYS ON', 'aod', 'WHITE', '' if on else 'OFF', 'GRAY', 'ON' if on else None)
    return six[:2] + [timeout, aod] + six[2:]


if not hasattr(P, '_cells6'):
    P._cells6 = P.cells
P.cells = cells8
FULL8 = dict(P.FULL, rows=[5] * 8, index=[1.0] * 8, name=[1.0] * 8)
P.NCOL = 4
P.MAXSCROLL = 2


def timeout_screen(sel='1 MIN'):
    cv = Canvas()
    cv.ring(230, 232, 'GRAY')
    cv.text('DRAG TO SET TIMEOUT', 'mono', 14, 'GRAY', cx=233, y=HINT_TOP)
    box(cv, *BTN, 'CANCEL', False)
    icon_sized(cv, *ICON_XY, 'WHITE', P.GLYPH['timeout'], module=16, pad=8)
    field(cv)
    i = TIMEOUTS.index(sel)
    cv.text(sel, 'bold', 56, 'WHITE', x=X0, cy=257.5)
    if i > 0:
        cv.text(TIMEOUTS[i - 1], 'mono', 23, 'GRAY', x=X0, cy=174)
    if i < len(TIMEOUTS) - 1:
        cv.text(TIMEOUTS[i + 1], 'mono', 23, 'GRAY', x=X0, cy=341)
    cv.text('%d/%d' % (i + 1, len(TIMEOUTS)), 'mono', 14, 'GRAY', cx=233, y=381)
    cv.text('TAP TO KEEP', 'mono', 14, 'GRAY', cx=233, y=401)
    return cv


if __name__ == '__main__':
    shots = [
        (P.panel(dict(P.FIX, timeout='1 MIN', aod=False), e=FULL8).render(), 'PANEL, EIGHT CELLS'),
        (P.panel(dict(P.FIX, timeout='1 MIN', aod=True), scroll=0.5, e=FULL8).render(), 'PANEL, MID-SCROLL, ALWAYS ON'),
        (P.panel(dict(P.FIX, timeout='1 MIN', aod=True), scroll=2, e=FULL8).render(), 'PANEL, SCROLLED TO THE END'),
        (timeout_screen('1 MIN').render(), 'TIMEOUT'),
    ]
    for im, n in shots:
        pass
    f = ImageFont.truetype('fonts/MonoR.otf', 16)
    cols = 4
    Wc, Hh = 466 + 24, 466 + 44
    sh = Image.new('RGB', (cols * Wc + 24, Hh + 24), (28, 28, 28))
    d = ImageDraw.Draw(sh)
    for i, (im, label) in enumerate(shots):
        x, y = 24 + (i % cols) * Wc, 24
        sh.paste(im, (x, y))
        d.text((x, y + 474), label, font=f, fill=(136, 142, 152))
        im.save('out/display-%d.png' % i)
    sh.save('out/display-settings.png')
