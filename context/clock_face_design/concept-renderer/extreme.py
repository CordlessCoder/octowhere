"""Level-4 prototypes in the B family. Same fixture: 13:07:42, Thu 24 Sep, Europe/Dublin."""
from lib import *
from face import SYM, digits

H, M, S = 13, 7, 42


def x1_crop():
    """Hours and minutes at 250 px, hard-cropped by the round edge."""
    cv = Canvas()
    px = 250
    a = adv('bold', px)
    x0 = 233 - a - 4
    digits(cv, '13', px, 'WHITE', x0, 214)
    cv.rect(0, 232, N, N, 'WHITE')
    digits(cv, '07', px, 'BLACK', x0, 420)
    # seconds chip riding the band edge, right
    cv.rect(392, 214, 450, 252, 'BLACK')
    cv.text('%02d' % S, 'bold', 28, 'WHITE', cx=418, cy=233)
    cv.ring(230, 232, 'GRAY')
    return cv


def x2_wipe(S=S):
    """The band is the seconds: white fills left to right, one step a second.
    Minutes flip from white to black where the fill passes under them."""
    cv = Canvas()
    cv.ring(230, 232, 'GRAY')
    X0, BIG = 62, 136
    digits(cv, '%02d' % H, BIG, 'WHITE', X0, 184)
    icon(cv, 262, 84, 'BLUE', SYM['gnss'])
    cv.text('LOCAL', 'bold', 16, 'GRAY', x=262, y=125)
    edge = round(N * (S + 1) / 60)
    # empty part of the band: outlined, so the band keeps its bounds
    cv.rect(0, 198, N, 318, fade('GRAY', 0.35))
    cv.rect(0, 200, N, 316, 'BLACK')
    digits(cv, '%02d' % M, BIG, 'WHITE', X0, 307)
    cv.text('%02d' % S, 'bold', 40, 'WHITE', x=262, bottom=307)
    # filled part, with the minutes redrawn in black clipped to it
    fill = Canvas()
    fill.rect(0, 198, edge, 318, 'WHITE')
    digits(fill, '%02d' % M, BIG, 'BLACK', X0, 307)
    fill.text('%02d' % S, 'bold', 40, 'BLACK', x=262, bottom=307)
    cv.img.paste(fill.img.crop((0, 198 * SS, edge * SS, 318 * SS)), (0, 198 * SS))
    cv.text('THU 24 SEP 2026', 'mono', 23, 'WHITE', cx=233, y=334)
    cv.text('AUTO  IST  +01:00', 'mono', 14, 'GRAY', cx=233, y=361)
    cv.text('EUROPE/DUBLIN', 'mono', 14, 'GRAY', cx=233, y=377)
    return cv


def x3_ladder():
    """The face as a drum of hours: past hours above, coming hours below,
    the current hour in the band beside the minutes. Same grammar as the picker."""
    cv = Canvas()
    cv.ring(230, 232, 'GRAY')
    pitch = 50
    mid = 257.5
    for d in (-2, -1, 1, 2):
        y = mid + d * pitch + (22 if d > 0 else -22)
        k = {1: 1.0, 2: 0.5}[abs(d)]
        cv.text('%02d' % ((H + d) % 24), 'mono', 34, fade('GRAY', k), x=62, cy=y)
    cv.rect(0, 198, N, 318, 'WHITE')
    digits(cv, '%02d' % H, 86, 'BLACK', 56, 289)
    cv.rect(166, 214, 170, 302, 'BLACK')
    digits(cv, '%02d' % M, 86, 'BLACK', 186, 289)
    cv.text('%02d' % S, 'bold', 28, 'BLACK', x=306, bottom=289)
    icon(cv, 330, 118, 'BLUE', SYM['gnss'])
    cv.text('LOCAL', 'bold', 16, 'GRAY', x=330, y=159)
    cv.text('THU 24 SEP', 'mono', 19, 'WHITE', x=208, y=338)
    cv.text('IST +01:00', 'mono', 14, 'GRAY', x=208, y=364)
    cv.text('AUTO', 'mono', 14, 'GRAY', x=208, y=382)
    return cv


def x4_display():
    """Display face for the time, one line, knocked out of the band.
    Seconds become a stepped rail of 60 cells under it."""
    cv = Canvas()
    cv.ring(230, 232, 'GRAY')
    icon(cv, 217, 96, 'BLUE', SYM['gnss'])
    cv.text('LOCAL', 'bold', 16, 'GRAY', cx=233, y=137)
    cv.text('THU 24 SEP', 'shapiro', 22, 'WHITE', cx=233, y=166)
    cv.rect(0, 198, N, 300, 'WHITE')
    cv.text('13:07', 'shapiro', 76, 'BLACK', cx=233, cy=249)
    # 60-cell seconds rail: 4 px cells on a 6 px pitch
    x = 233 - 30 * 6 + 1
    for k in range(60):
        cv.rect(x + k * 6, 310, x + k * 6 + 4, 322, 'WHITE' if k <= S else fade('GRAY', 0.35))
    cv.text('AUTO  IST  +01:00', 'mono', 14, 'GRAY', cx=233, y=340)
    cv.text('EUROPE/DUBLIN', 'mono', 14, 'GRAY', cx=233, y=356)
    return cv


if __name__ == '__main__':
    ims = []
    for n, fn in [('x1-crop', x1_crop), ('x2-wipe-17s', lambda: x2_wipe(17)), ('x2-wipe-42s', x2_wipe),
                  ('x3-ladder', x3_ladder), ('x4-display', x4_display)]:
        ims.append(fn().save('out/proto-%s.png' % n))
    sh = Image.new('RGB', (486 * 5 - 20, 466), (28, 28, 28))
    for i, im in enumerate(ims):
        sh.paste(im, (i * 486, 0))
    sh.save('out/sheet-prototypes.png')
