"""Three compositions of the normal state, fixture 13:07:42 Thu 24 Sep 2026, Europe/Dublin."""
from lib import *

H, M, S = 13, 7, 42
ICON_GNSS = '00100 01010 10101 01010 00100'


def seconds_track(cv, s, elapsed='WHITE', rest=fade('GRAY', 0.45)):
    for k in range(60):
        on = k <= s
        if k == 0:
            cv.radial_bar(0, 202, 226, 4, 'ORANGE')
        elif k % 5 == 0:
            cv.radial_bar(k * 6, 202, 226, 4, elapsed if on else rest)
        else:
            cv.radial_bar(k * 6, 216, 226, 2, elapsed if on else rest)


def dir_a():
    """Dial: compass sibling. Ring carries elapsed seconds, slab carries HH:MM."""
    cv = Canvas()
    cv.ring(230, 232, 'GRAY')
    seconds_track(cv, S)
    icon(cv, 217, 102, 'BLUE', ICON_GNSS)
    cv.text('LOCAL TIME', 'bold', 16, 'GRAY', cx=233, y=142)
    cv.rect(106, 186, 360, 277, 'WHITE')
    hhmm(cv, '%02d:%02d' % (H, M), 86, 'BLACK', 116, 263, 28)
    cv.text('THU 24 SEP', 'mono', 23, 'GRAY', cx=233, y=293)
    cv.rect(138, 321, 329, 322, 'GRAY')
    cv.text('AUTO  IST  +01:00', 'mono', 14, 'GRAY', cx=233, y=328)
    cv.text('EUROPE/DUBLIN', 'mono', 14, 'GRAY', cx=233, y=345)
    return cv


def dir_b():
    """Editorial: hours on the field, minutes knocked out of a full-chord band."""
    cv = Canvas()
    cv.ring(230, 232, 'GRAY')
    # hours, giant, white on black
    x0 = 62
    a = adv('bold', 136)
    for i, ch in enumerate('%02d' % H):
        pen(cv, ch, 'bold', 136, 'WHITE', x0 + i * a, 184)
    # minutes band, full chord
    cv.rect(0, 198, 466, 318, 'WHITE')
    for i, ch in enumerate('%02d' % M):
        pen(cv, ch, 'bold', 136, 'BLACK', x0 + i * a, 307)
    # right column: icon, seconds, date
    icon(cv, 262, 84, 'BLUE', ICON_GNSS)
    cv.text('SEC', 'mono', 14, 'GRAY', x=262, y=132)
    cv.text('%02d' % S, 'bold', 40, 'WHITE', x=262, bottom=184)
    cv.text('THU', 'bold', 24, 'BLACK', x=262, y=214)
    cv.text('24 SEP', 'bold', 24, 'BLACK', x=262, y=246)
    cv.text('2026', 'mono', 14, 'BLACK', x=262, y=283)
    cv.text('AUTO  IST  +01:00', 'mono', 14, 'GRAY', cx=233, y=334)
    cv.text('EUROPE/DUBLIN', 'mono', 14, 'GRAY', cx=233, y=351)
    return cv


def dir_c():
    """Analogue: fixed dial, rectilinear hands, date aperture at 3."""
    cv = Canvas()
    cv.ring(230, 232, 'GRAY')
    for k in range(60):
        if k == 0:
            cv.radial_bar(0, 196, 226, 6, 'ORANGE')
        elif k % 5 == 0:
            cv.radial_bar(k * 6, 202, 226, 4, 'WHITE')
        else:
            cv.radial_bar(k * 6, 216, 226, 2, 'GRAY')
    icon(cv, 217, 92, 'BLUE', ICON_GNSS)
    cv.text('LOCAL TIME', 'bold', 16, 'GRAY', cx=233, y=132)
    # date aperture at 3 o'clock
    cv.text('THU', 'mono', 14, 'GRAY', cx=348, y=196)
    cv.rect(318, 214, 378, 253, 'WHITE')
    cv.text('24', 'bold', 28, 'BLACK', cx=348, cy=233.5)
    cv.text('AUTO  IST  +01:00', 'mono', 14, 'GRAY', cx=233, y=318)
    cv.text('EUROPE/DUBLIN', 'mono', 14, 'GRAY', cx=233, y=335)
    # seconds as a square pip riding inside the ticks
    import math
    a = math.radians(S * 6)
    px_, py_ = C + math.sin(a) * 188, C - math.cos(a) * 188
    cv.rect(round(px_ - 4), round(py_ - 4), round(px_ + 4), round(py_ + 4), 'WHITE')
    # hands
    hang = (H % 12) * 30 + M * 0.5
    mang = M * 6
    cv.radial_bar(hang, -18, 124, 12, 'WHITE')
    cv.radial_bar(mang, -18, 196, 8, 'WHITE')
    cv.rect(225, 225, 241, 241, 'BLACK')
    cv.rect(229, 229, 237, 237, 'WHITE')
    return cv


if __name__ == '__main__':
    ims = []
    for n, fn in [('a-dial', dir_a), ('b-editorial', dir_b), ('c-analogue', dir_c)]:
        ims.append(fn().save('out/direction-%s.png' % n))
    sheet = Image.new('RGB', (466 * 3 + 40, 466), (28, 28, 28))
    for i, im in enumerate(ims):
        sheet.paste(im, (i * 486, 0))
    sheet.save('out/directions.png')
