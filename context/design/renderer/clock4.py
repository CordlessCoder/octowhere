"""Clock face, round 4: the identity's language applied to the face (owner, 25 Sep: the identity
and fault screens are the direction for the whole UI).

Composition, from the identity capture: black field, PURPLE halftone scatter hard against a
black zone that carries everything; LIME for OCTOWHERE's own voice; a microtext row of glyphs,
pixel digits and two text lines; the time in stretched Shapiro; a hatch column as the weight on
the right, here filling with the seconds. No ring. Faults take the fault screen's language.

Fixture: LOCAL, GNSS, 13:07:42, Thu 24 Sep 2026, Europe/Dublin, UTC 12:07, 7 of 12 satellites,
battery 87 %.
"""
import math
import lib
lib.RECORD = False
from lib import *
from PIL import ImageChops
from identity import finish, text_mask, circle
from identity_f import pix_digits, glyph, barcode
from identity_g import hatch, frame_rect, tm, put, putc
import marks_frame as MF

W = 466
LEFT = 40
ZONE = (136, 342)            # the black zone: nothing of the scatter inside these rows
TIME_BOTTOM, TIME_PX, TIME_SY = 294, 64, 2.1
ROW_TOP = 148
PLATE_TOP = 308
HATCH = (372, 436, 190, 295)
GNSS_G = '00100 01010 10101 01010 00100'
RTC_G = '11111 10001 10101 10001 11111'
NODATA_G = '11100 11000 00100 00011 00111'
STOP_G = '01010 01010 01010 01010 01010'
FIX = dict(h=13, m=7, s=42, utc='12 07', date='THU 24 SEP 2026', zone='EUROPE/DUBLIN',
           mode='AUTO', abbr='IST', off='+01:00', sats=(7, 12), bat=87)


def _hash(i, salt):
    x = (i * 2654435761 + salt * 40503) & 0xFFFFFFFF
    x ^= x >> 15
    x = (x * 2246822519) & 0xFFFFFFFF
    x ^= x >> 13
    return (x & 0xFFFF) / 65536.0


def scatter(cv, angle, col='PURPLE', zone=ZONE, density=1.15, centre=0.45):
    """The identity's scatter (same marks, grid and density law), static, turned by `angle`."""
    i = 0
    for y in range(-2, W, 8):
        for x in range(12, W - 6, 8):
            i += 1
            v, kind = _hash(i, 1), _hash(i, 2)
            yy = y if y < zone[1] else y + 2
            if zone[0] - 2 < yy + 6 and yy < zone[1] + 2:
                continue
            dx, dy = x + 3 - 233, yy + 3 - 233
            r = math.hypot(dx, dy)
            if r > 228:
                continue
            th = math.atan2(dy, dx)
            radial = centre + (1 - centre) * min(1.0, max(0.0, (r - 40) / 180))
            turn = 0.6 + 0.4 * math.cos(th - angle)
            if v < radial * turn * density:
                if kind < 0.6:
                    cv.rect(x, yy, x + 6, yy + 6, col); cv.rect(x + 2, yy + 2, x + 4, yy + 4, 'BLACK')
                else:
                    cv.rect(x + 1, yy + 1, x + 5, yy + 5, col)


def hour_angle(h):
    """24-hour dial, noon at the top, one step an hour."""
    return math.radians((h - 12) * 15 - 90)


def mark15(cv, x, y, col):
    for j, r in enumerate(MF.ROW['L1']):
        for i, b in enumerate(r):
            if b == '1':
                cv.rect(x + i, y + j, x + i + 1, y + j + 1, col)
    return x + 15


def row(cv, f, status_glyph, status_col, lines, col='LIME', utc=True):
    x = mark15(cv, LEFT, ROW_TOP, col) + 11
    x = glyph(cv, status_glyph, x, ROW_TOP, col=status_col) + 11
    if utc:
        x = pix_digits(cv, f['utc'], x, ROW_TOP, col=col) + 10
    cv.text(lines[0], 'mono', 14, col, x=x, y=ROW_TOP - 2)
    cv.text(lines[1], 'mono', 14, col, x=x, y=ROW_TOP + 14)


CELL = None


def time_cells(px=TIME_PX, sy=TIME_SY):
    """Tabular cells for stretched Shapiro digits: every digit centred in the widest digit's cell."""
    f = font('shapiro', px)
    cw = max(f.getlength(c) for c in '0123456789') / SS
    colon = f.getlength(':') / SS
    return cw, colon


def big_time(cv, s, col, px=TIME_PX, sy=TIME_SY, x0=LEFT, bottom=TIME_BOTTOM):
    """'HH:MM' on fixed cells; '-' draws as a bar across the middle of its cell."""
    cw, colon = time_cells(px, sy)
    cap = tm('0', 'shapiro', px, sy=sy).height / SS
    x = x0
    for ch in s:
        w = colon if ch == ':' else cw
        if ch == '-':
            cv.rect(x + 6, bottom - cap / 2 - 6, x + w - 6, bottom - cap / 2 + 6, col)
        elif ch != ' ':
            m = tm(ch, 'shapiro', px, sy=sy)
            put(cv, m, col, x + (w - m.width / SS) / 2, bottom - m.height / SS)
        x += w + 4
    return x


def seconds_col(cv, s, col='LIME'):
    """The hatch column fills top down with the seconds: s/60 of its height, in 60 steps."""
    x0, x1, y0, y1 = HATCH
    frame_rect(cv, x0, y0, x1, y1, fade(col, 0.35) if isinstance(col, str) else col, 1)
    h = round((y1 - y0) * s / 60)
    if h > 0:
        hatch(cv, x0, y0, x1, y0 + h, col, pitch=10, w=5)


GROUND = []     # rectangles that are solid ground (fills and cells with a black inside)


def plate_row(cv, f, top=PLATE_TOP, x=LEFT, tag_col='GRAY', cell_col='GRAY', text_col='WHITE', extra=None,
              tag_text='BLACK'):
    px, padx, gap, h = 14, 6, 4, 20
    base = top + (h + cap_height('mono', px)) / 2
    for kind, t in (('tag', f['mode']), ('cell', f['abbr']), ('cell', f['off'])):
        w = round(adv('mono', px) * len(t) + 2 * padx)
        GROUND.append((x, top, x + w, top + h))
        if kind == 'tag':
            cv.rect(x, top, x + w, top + h, tag_col)
            cv.text(t, 'bold', px, tag_text, x=x + padx, bottom=base)
        else:
            frame_rect(cv, x, top, x + w, top + h, cell_col, 1)
            cv.text(t, 'mono', px, text_col, x=x + padx, bottom=base)
        x += w + gap
    if extra:
        cv.text(extra, 'mono', 14, text_col, x=x + 8, bottom=base)


def layered(fg, angle, halo=0, zone=(0, -1), density=1.15, centre=0.45):
    """The scatter under everything. fg(cv) draws the elements on a black canvas; everything it
    draws, and every GROUND rectangle whole, covers the scatter, so knockout text stays black.
    halo > 0: a BLACK ring that many px wide round every element (the renderer's outlined text,
    used as a halo), so the scatter keeps clear of edges by exactly that much."""
    from PIL import ImageFilter
    import numpy as np
    GROUND.clear()
    base = Canvas()
    scatter(base, angle, zone=zone, density=density, centre=centre)
    top = Canvas()
    fg(top)
    arr = np.array(top.img)
    m = Image.fromarray(((arr.sum(axis=2) > 0) * 255).astype('uint8'))
    d = ImageDraw.Draw(m)
    for (a, b, c, e) in GROUND:
        d.rectangle([a * SS, b * SS, c * SS - 1, e * SS - 1], fill=255)
    if halo:
        k = int(halo * SS) * 2 + 1
        ring = m.filter(ImageFilter.MaxFilter(k))
        base.img.paste((0, 0, 0), (0, 0), ring)
    base.img.paste(top.img, (0, 0), m)
    return finish(base)


def local_fg(f=FIX, rtc=False, seconds=True, time_face='shapiro'):
    def fg(cv):
        row(cv, f, RTC_G if rtc else GNSS_G, 'GRAY' if rtc else 'BLUE', (f['date'], f['zone']))
        if time_face == 'shapiro':
            big_time(cv, '%02d:%02d' % (f['h'], f['m']), 'LIME')
            m = tm('%02d' % f['s'], 'shapiro', 36, sy=1.95)
            put(cv, m, 'LIME', 312, TIME_BOTTOM - m.height / SS)
        else:
            from face import digits as fdigits
            a = adv('bold', 120)
            fdigits(cv, '%02d' % f['h'], 120, 'LIME', LEFT, TIME_BOTTOM)
            fdigits(cv, '%02d' % f['m'], 120, 'LIME', LEFT + 2 * a + 14, TIME_BOTTOM)
        if seconds:
            seconds_col(cv, f['s'])
        plate_row(cv, f, extra='SAT %02d/%02d  BAT %d%%' % (f['sats'] + (f['bat'],)))
    return fg


def local(f=FIX, rtc=False, seconds=True, time_face='shapiro'):
    cv = Canvas()
    scatter(cv, hour_angle(f['h']))
    row(cv, f, RTC_G if rtc else GNSS_G, 'GRAY' if rtc else 'BLUE', (f['date'], f['zone']))
    if time_face == 'shapiro':
        big_time(cv, '%02d:%02d' % (f['h'], f['m']), 'LIME')
        m = tm('%02d' % f['s'], 'shapiro', 36, sy=1.95)
        put(cv, m, 'LIME', 312, TIME_BOTTOM - m.height / SS)
    else:
        from face import digits as fdigits
        a = adv('bold', 120)
        fdigits(cv, '%02d' % f['h'], 120, 'LIME', LEFT, TIME_BOTTOM)
        fdigits(cv, '%02d' % f['m'], 120, 'LIME', LEFT + 2 * a + 14, TIME_BOTTOM)
    if seconds:
        seconds_col(cv, f['s'])
    plate_row(cv, f, extra='SAT %02d/%02d  BAT %d%%' % (f['sats'] + (f['bat'],)))
    return finish(cv)


def stopped(f=FIX):
    cv = Canvas()
    scatter(cv, hour_angle(f['h']))
    row(cv, f, STOP_G, 'ORANGE', ('WAITING FOR GNSS', f['zone']), utc=False)
    big_time(cv, '--:--', 'ORANGE')
    x0, x1, y0, y1 = HATCH
    hatch(cv, x0, y0, x1, y1, 'ORANGE', pitch=10, w=5)
    cv.text('STOPPED', 'bold', 16, 'ORANGE', right=HATCH[0] - 10, y=HATCH[2] + 2)
    plate_row(cv, f)
    return finish(cv)


def nodata(f=FIX, t=0):
    """The fault screen's language: RED field to the glass, BLACK band, the no-data glyph four
    times, a running line, microtext. The plate stays: the zone does not come from the clock."""
    import identity_g as IG
    cv = Canvas()
    cv.img.paste(Image.new('RGB', cv.img.size, rgb('RED')), (0, 0), circle(233))
    for y in (86, 150, 366, 430):
        for x in range(20, 450, 46):
            cv.rect(x, y, x + 9, y + 2, 'BLACK')
    for gx, gy in ((110, 118), (356, 118), (110, 348), (356, 348)):
        cv.rect(gx - 32, gy - 38, gx + 32, gy - 30, 'BLACK')
        cv.rect(gx - 32, gy + 30, gx + 32, gy + 38, 'BLACK')
        glyph(cv, NODATA_G, gx - 22.5, gy - 22.5, m=9, col='BLACK')
    cv.rect(0, 198, W, 318, 'BLACK')
    giant = tm('CLOCK', 'shapiro', 200)
    gm = Image.new('L', cv.img.size, 0)
    gm.paste(giant, (int((233 - giant.width / SS / 2 + 40 - t) * SS), int((258 - giant.height / SS / 2) * SS)))
    band = Image.new('L', cv.img.size, 0)
    ImageDraw.Draw(band).rectangle([0, 198 * SS, W * SS, 318 * SS - 1], fill=255)
    cv.img.paste(Image.new('RGB', cv.img.size, rgb('RED')), (0, 0), ImageChops.multiply(gm, band))
    cv.rect(0, 234, W, 282, 'BLACK')
    run = tm('CLOCK NO DATA_', 'shapiro', 32, sy=1.3)
    period = run.width / SS
    x = -((t * 4) % period) - period
    while x < W:
        put(cv, run, 'WHITE', x, 258 - run.height / SS / 2)
        x += period
    cv.text('CLOCK', 'bold', 12, 'BLACK', x=162, y=164)
    cv.text('NOT READABLE', 'mono', 12, 'BLACK', x=162, y=178)
    hatch(cv, 290, 164, 318, 190, 'BLACK', phase=t * 2)
    plate_row(cv, f, top=326, x=162, tag_col='BLACK', cell_col='BLACK', text_col='BLACK', tag_text='RED')
    cv.text(f['zone'], 'mono', 12, 'BLACK', x=162, y=352)
    return finish(cv)


def under_sheet(path):
    shots = [
        (layered(local_fg(), hour_angle(13), 0), 'U1  UNDER EVERYTHING, IDENTITY DENSITY'),
        (layered(local_fg(), hour_angle(13), 2), 'U2  AS U1, 2 PX BLACK HALO'),
        (layered(local_fg(), hour_angle(13), 2, density=0.8, centre=0.15), 'U3  HALO, THINNER TOWARD THE CENTRE'),
        (layered(local_fg(time_face='fraktion'), hour_angle(13), 2, density=0.8, centre=0.15), 'U4  AS U3, FRAKTION DIGITS'),
        (layered(local_fg(), hour_angle(13), 1, density=0.8, centre=0.15), 'U5  AS U3, 1 PX HALO'),
        (layered(local_fg(), hour_angle(13), 0, density=0.8, centre=0.15), 'U6  AS U3, NO HALO'),
    ]
    f = ImageFont.truetype('fonts/MonoB.otf', 16)
    Wc, Hh = W + 24, W + 48
    sh = Image.new('RGB', (3 * Wc + 24, 2 * Hh + 24), (20, 20, 22))
    d = ImageDraw.Draw(sh)
    for i, (im, name) in enumerate(shots):
        x, y = 24 + (i % 3) * Wc, 24 + (i // 3) * Hh
        sh.paste(im, (x, y))
        im.save('out/clock4-under-%d.png' % i)
        d.text((x, y + W + 8), name, font=f, fill=(210, 211, 214))
    sh.save(path)


if __name__ == '__main__' and __import__('sys').argv[1:] == ['under']:
    under_sheet('out/clock4-under.png')
elif __name__ == '__main__':
    shots = [(local(), 'LOCAL, GNSS'), (local(rtc=True), 'LOCAL, RTC'),
             (local(time_face='fraktion'), 'LOCAL, FRAKTION DIGITS INSTEAD'),
             (stopped(), 'STOPPED'), (nodata(), 'NO DATA, IN THE FAULT LANGUAGE')]
    f = ImageFont.truetype('fonts/MonoB.otf', 16)
    cols = 3
    Wc, Hh = W + 24, W + 48
    rows = (len(shots) + cols - 1) // cols
    sh = Image.new('RGB', (cols * Wc + 24, rows * Hh + 24), (20, 20, 22))
    d = ImageDraw.Draw(sh)
    for i, (im, name) in enumerate(shots):
        x, y = 24 + (i % cols) * Wc, 24 + (i // cols) * Hh
        sh.paste(im, (x, y))
        d.text((x, y + W + 8), name, font=f, fill=(210, 211, 214))
    sh.save('out/clock4.png')
