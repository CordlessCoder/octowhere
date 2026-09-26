"""Clock face, round 4, second pass: the built face keeps its own identity (hours over the band,
minutes and seconds knocked out of it, the icon, the plate, the wordmark), and takes on the
identity's language: high chromatic contrast, bright microtext, a complex background under
everything, blocky elements, and some living motion.

Layers, bottom up: the PURPLE scatter over the whole circle; a BLACK halo round every element on
the field; the elements. Fills (the band, the icon tile's black inside, the plate's cells) are
solid ground, so knockout text stays black.
"""
import math
import lib
lib.RECORD = False
from lib import *
from PIL import ImageFilter
import numpy as np
from face import SYM, X0, BIG, HOURS_BASE, MIN_BASE, SEC_PEN, SEC_PX
from industrial import digits, cells_text, pen_for, plate, BAND, FIX
from identity import finish
from identity_g import hatch, frame_rect
import clockcolor as CC
import clock4 as C4

W = 466
F = dict(FIX, utc='12:07')
SEC_COL = (394, 440, 206, 311)      # the band's right end, past the wordmark


def band_seconds(cv, s, ink='BLACK'):
    """A hatch column in the band's right end, filling top down with the seconds (60 steps)."""
    x0, x1, y0, y1 = SEC_COL
    frame_rect(cv, x0, y0, x1, y1, ink, 1)
    h = round((y1 - y0) * s / 60)
    if h > 0:
        hatch(cv, x0, y0, x1, y0 + h, ink, pitch=8, w=4)


def fg(o, f=F):
    ground = [(0, BAND[0], W, BAND[1]), (262, 90, 358, 186)]

    def draw(cv):
        digits(cv, '%02d' % f['h'], BIG, o.get('hours', 'WHITE'), X0, HOURS_BASE)
        icon_sized(cv, 262, 90, o.get('icon', 'BLUE'), SYM['gnss'], module=16, pad=8)
        band(cv, BAND[0], BAND[1], o.get('band', 'LIME'))
        cells_text(cv, 'LOCAL', 'bold', 16, 'BLACK', pen_for('LOCAL', 'bold', 16, x=262), 225)
        digits(cv, '%02d' % f['m'], BIG, 'BLACK', X0, MIN_BASE)
        digits(cv, '%02d' % f['s'], SEC_PX, 'BLACK', SEC_PEN, MIN_BASE)
        cv.text(f['date'], 'mono', 23, o.get('date', 'WHITE'), cx=233, y=334)
        plate(cv, [('tag', f['mode']), ('cell', f['abbr']), ('cell', f['off'])], 360)
        cells_text(cv, f['zone'], 'mono', 14, o.get('zone', 'GRAY'), pen_for(f['zone'], 'mono', 14, cx=233), 398)
        CC.wordmark(cv, o.get('mark_field', 'LIME'), 'BLACK')
        if o.get('micro_top'):
            CC.micro_top(cv, f, 'LIME')
        if o.get('micro_band'):
            CC.micro_band(cv, f, 'BLACK')
        if o.get('band_seconds'):
            band_seconds(cv, f['s'])
    # plate cells are ground too
    px, padx, gap = 14, 6, 4
    cells = [f['mode'], f['abbr'], f['off']]
    ws = [round(adv('mono', px) * len(t) + 2 * padx) for t in cells]
    x = round(233 - (sum(ws) + gap * (len(ws) - 1)) / 2)
    for w in ws:
        ground.append((x, 360, x + w, 380))
        x += w + gap
    return draw, ground


def compose(o, f=F):
    draw, ground = fg(o, f)
    base = Canvas()
    if o.get('bg', True):
        ang = C4.hour_angle(f['h']) if o.get('hand') else 0.8
        C4.scatter(base, ang, col=o.get('bg_col', 'PURPLE'), zone=(0, -1),
                   density=o.get('density', 0.8), centre=o.get('centre', 0.15))
    top = Canvas()
    draw(top)
    arr = np.array(top.img)
    m = Image.fromarray(((arr.sum(axis=2) > 0) * 255).astype('uint8'))
    d = ImageDraw.Draw(m)
    for (a, b, c, e) in ground:
        d.rectangle([a * SS, b * SS, c * SS - 1, e * SS - 1], fill=255)
    halo = o.get('halo', 2)
    if halo:
        base.img.paste((0, 0, 0), (0, 0), m.filter(ImageFilter.MaxFilter(int(halo * SS) * 2 + 1)))
    base.img.paste(top.img, (0, 0), m)
    base.ring(230, 232, o.get('ring', 'GRAY'))
    return finish(base)


VARIANTS = [
    ('A', 'AS BUILT, FOR REFERENCE', None),
    ('B', 'LIME BAND, SCATTER UNDER EVERYTHING', dict()),
    ('C', 'B, WITH MICROTEXT ABOVE AND IN THE BAND', dict(micro_top=True, micro_band=True)),
    ('D', 'C, WITH A SECONDS COLUMN IN THE BAND', dict(micro_top=True, micro_band=True, band_seconds=True)),
    ('E', 'D, THE SCATTER AS A 24-HOUR HAND', dict(micro_top=True, micro_band=True, band_seconds=True, hand=True)),
    ('F', 'D ON A WHITE BAND, LIME ACCENTS', dict(micro_top=True, micro_band=True, band_seconds=True, band='WHITE')),
]


if __name__ == '__main__':
    fnt = ImageFont.truetype('fonts/MonoB.otf', 16)
    Wc, Hh = W + 24, W + 48
    sh = Image.new('RGB', (3 * Wc + 24, 2 * Hh + 24), (20, 20, 22))
    dr = ImageDraw.Draw(sh)
    for i, (k, name, o) in enumerate(VARIANTS):
        im = Image.open('caps/screen-captures/clock-gnss.png').convert('RGB') if o is None else compose(o)
        x, y = 24 + (i % 3) * Wc, 24 + (i // 3) * Hh
        sh.paste(im, (x, y))
        im.save('out/clock5-%s.png' % k)
        dr.text((x, y + W + 8), '%s  %s' % (k, name), font=fnt, fill=(210, 211, 214))
    sh.save('out/clock5.png')
