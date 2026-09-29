"""Maratype wordmark placement studies on the selected K1 clock face.

Requires sibling octowhere-design-project and octowhere-clock-battery-fill-v2.
The pixel compositor is illustrative, not a firmware modification.
"""
from pathlib import Path
import math
import os
import sys
from PIL import Image, ImageChops, ImageDraw, ImageFont

HERE = Path(__file__).resolve().parent
WORK = HERE.parent
RENDERER = WORK / 'octowhere-design-project' / 'renderer'
sys.path.insert(0, str(RENDERER))
sys.path.insert(0, str(WORK / 'octowhere-clock-battery-fill-v2'))
os.chdir(RENDERER)

import lib
from lib import Canvas, SS, rgb
import clockface4 as CK
import render_fill as BF
import identity_g as IG
from face import X0, BIG
from industrial import BAND

lib.FONT['maratype'] = str(HERE / 'Maratype.otf')
PANEL = (18, 20, 23)
INK = (215, 218, 222)
FONT = ImageFont.truetype(str(RENDERER / 'fonts/MonoB.otf'), 16)
PURPLE = (52, 16, 112)


def hash32(v):
    v = (v ^ (v >> 16))*0x7feb352d & 0xffffffff
    v = (v ^ (v >> 15))*0x846ca68b & 0xffffffff
    return (v ^ (v >> 16)) & 0xffffffff


def k1_scatter(cv, *args, **kwargs):
    """The selected K1 two-lobe scatter, matching its reference stills."""
    fields = ((304,139,137,-.60,.48,0x8c70),
              (164,363,111,2.55,.28,0x8c71))
    for cx,cy,radius,facing,density,seed in fields:
        for iy in range(58):
            y = -2+8*iy
            if 196 <= y <= 319:
                continue
            for ix in range(58):
                x = 12+8*ix
                if math.hypot(x-233,y-233) > 228:
                    continue
                dx,dy=x-cx,y-cy
                distance=math.hypot(dx,dy)
                if distance > radius:
                    continue
                radial=.45+.55*max(0,min(1,(distance-40)/180))
                turn=.45+.55*((dx*math.cos(facing)+dy*math.sin(facing))/distance if distance else 1)
                h=hash32(seed^hash32(ix*0x9e3779b1&0xffffffff)^hash32(iy*0x85ebca77&0xffffffff))
                if h/2**32 >= max(0,radial*turn*density):
                    continue
                hollow=hash32(h^0x12b591)/2**32 < .6
                if hollow:
                    cv.rect(x-3,y-3,x+3,y+3,PURPLE)
                    cv.rect(x-1,y-1,x+1,y+1,'BLACK')
                else:
                    cv.rect(x-2,y-2,x+2,y+2,PURPLE)


def hour_rail(im, hour, active=True):
    im=im.copy();d=ImageDraw.Draw(im)
    for i in range(24):
        x=92+i*12
        d.rectangle((x,412,x+4,414),fill=(39,47,54))
    if active:
        x=92+(hour%24)*12
        d.rectangle((x-1,408,x+6,418),fill=(192,254,4))
        d.rectangle((x+1,410,x+4,416),fill=(0,0,0))
    return im


def gauge_color(band_col, pct):
    if pct is None:
        return 'GRAY'
    if pct <= 15:
        return 'ORANGE'
    return 'WHITE' if band_col == 'RED' else band_col


def battery(cv, pct, charging=False, phase=27, k=1.0,
            band_col='LIME', horizontal=False):
    """Selected solid/segmented grammar, mapped to either gauge orientation."""
    color = gauge_color(band_col, pct)
    if horizontal:
        x0, y0, x1, y1 = (323, 281, 440, 311)
        a, b, cross0, cross1 = 326, 437, 284, 308
    else:
        x0, y0, x1, y1 = (394, 206, 440, 311)
        a, b, cross0, cross1 = 308, 209, 397, 437  # y grows upward from 308
    cv.rect(x0, y0, x1, y1, 'BLACK')
    # One-pixel state-coloured frame, with 2px or more black around the fill.
    cv.rect(x0 + 1, y0 + 1, x1 - 1, y0 + 2, color)
    cv.rect(x0 + 1, y1 - 2, x1 - 1, y1 - 1, color)
    cv.rect(x0 + 1, y0 + 1, x0 + 2, y1 - 1, color)
    cv.rect(x1 - 2, y0 + 1, x1 - 1, y1 - 1, color)
    if pct is None:
        if horizontal:
            for x in (364, 389):
                cv.rect(x, 294, x + 13, 297, 'GRAY')
        else:
            for x in (402, 422):
                cv.rect(x, 255, x + 11, 258, 'GRAY')
        return
    max_len = (b - a) if horizontal else (a - b)
    length = round(max_len * max(0, min(100, pct)) / 100 * k)
    if length <= 0:
        return
    if not charging:
        if horizontal:
            cv.rect(a, cross0, a + length, cross1, color)
        else:
            cv.rect(cross0, a - length, cross1, a, color)
        return
    if band_col == 'RED':
        phase = 27  # selected NO DATA remains quiet
    sizes, gaps = BF.pattern_for_height(length, phase)
    cursor = a if horizontal else a - length
    for index, size in enumerate(sizes):
        if horizontal:
            cv.rect(cursor, cross0, cursor + size, cross1, color)
        else:
            cv.rect(cross0, cursor, cross1, cursor + size, color)
        cursor += size + (gaps[index] if index < len(gaps) else 0)


def vertical_mark(cv, col):
    """Natural Maratype, turned to read down; OCTO|WHERE crosses band top."""
    source = IG.tm('OCTOWHERE', 'maratype', 44)
    lead = IG.tm('OCTO', 'maratype', 44).width
    mask = source.transpose(Image.Transpose.ROTATE_270)
    x = 28
    y = round(BAND[0] - lead / SS)
    full = Image.new('L', cv.img.size, 0)
    full.paste(mask, (x * SS, y * SS))
    band = Image.new('L', cv.img.size, 0)
    ImageDraw.Draw(band).rectangle((0, BAND[0]*SS, 466*SS-1,
                                    BAND[1]*SS-1), fill=255)
    cv.img.paste(Image.new('RGB', cv.img.size, rgb(col)), (0, 0),
                 ImageChops.subtract(full, ImageChops.multiply(full, band)))
    cv.img.paste(Image.new('RGB', cv.img.size, rgb('BLACK')), (0, 0),
                 ImageChops.multiply(full, band))


def split_mark(cv, col):
    # Both lines are natural width and height. They inhabit the old icon field;
    # a smaller, still 5x5, state icon occupies the upper-right corner.
    cv.text('OCTO', 'maratype', 43, col, x=262, y=93)
    cv.text('WHERE', 'maratype', 43, col, x=262, y=142)


def face(style, state='gnss', pct=87, charging=False, phase=27):
    assert style in ('left', 'split')
    orig = (CK.CC.wordmark, CK.battery_col, CK.digits, CK.icon_sized,
            CK.Canvas, CK.C4.scatter, CK.lower)
    def mark(cv, field_col='LIME', band_col='BLACK'):
        if style == 'left':
            vertical_mark(cv, field_col)
        else:
            split_mark(cv, field_col)
    def bat(cv, p, c=False, ph=0, k=1.0, bc='LIME'):
        battery(cv, p, c, ph, k, bc, horizontal=(style == 'split'))
    def digits(cv, s, px, col, x0, base, name=None, clip=None):
        if style == 'left' and px == BIG and x0 == X0:
            x0 += 30
        return orig[2](cv, s, px, col, x0, base, name=name, clip=clip)
    def icon(cv, x, y, tile, rows, **kw):
        if style == 'split' and x == 262 and y == 90:
            x, y = 365, 110
            kw.update(module=9, pad=5)
        return orig[3](cv, x, y, tile, rows, **kw)
    class ShiftCanvas(orig[4]):
        def text(self, s, face, px, col, **kw):
            if style == 'left' and s == 'NO DATA' and face == 'shapiro' and kw.get('x') == 71:
                kw['x'] = 101
            return super().text(s, face, px, col, **kw)
    def lower(cv, st, f, kd=1.0, kz=1.0):
        if st != 'nodata':
            orig[6](cv, st, f, kd, kz)
    CK.CC.wordmark, CK.battery_col, CK.digits, CK.icon_sized, CK.Canvas, CK.C4.scatter, CK.lower = (
        mark, bat, digits, icon, ShiftCanvas, k1_scatter, lower)
    try:
        fixture = dict(CK.F, bat=pct, charging=charging, phase=phase)
        image=CK.face(state, f=fixture, scatter_k=0 if state=='nodata' else 1)
        if state != 'nodata':
            image=hour_rail(image,fixture['h'],active=state not in ('stopped','nozone'))
        return image
    finally:
        CK.CC.wordmark, CK.battery_col, CK.digits, CK.icon_sized, CK.Canvas, CK.C4.scatter, CK.lower = orig


def board(items, columns, filename):
    rows = (len(items) + columns - 1)//columns
    sh = Image.new('RGB', (columns*486+20, rows*516+20), PANEL)
    d = ImageDraw.Draw(sh)
    for i, (name, image) in enumerate(items):
        x, y = 20+(i%columns)*486, 20+(i//columns)*516
        sh.paste(image, (x, y))
        d.text((x+4, y+478), name, font=FONT, fill=INK)
    sh.save(HERE / filename, optimize=True)


def main():
    board([
        ('K1 / CURRENT 87% SOLID', BF.compose('gnss', charging=False)),
        ('A / LEFT WORDMARK', face('left')),
        ('B / SPLIT RIGHT + HORIZONTAL BAT', face('split')),
        ('K1 / CURRENT 87% CHARGING', BF.compose('gnss', charging=True)),
        ('A / LEFT WORDMARK / CHG', face('left', charging=True)),
        ('B / SPLIT RIGHT / CHG', face('split', charging=True)),
    ], 3, 'clock-maratype-directions.png')
    for style in ('left', 'split'):
        for state in BF.STATES:
            face(style, state).save(HERE / f'clock-{style}-{state}.png', optimize=True)
    board([(f'{style.upper()} / {BF.LABELS[state]}',
            Image.open(HERE / f'clock-{style}-{state}.png'))
           for state in BF.STATES for style in ('left','split')],
          4, 'clock-maratype-all-states.png')
    board([(f'{style.upper()} / {label}', face(style,state,pct=pct,charging=charging))
           for label,state,pct,charging in (
               ('12% SOLID','gnss',12,False),('12% CHG','gnss',12,True),
               ('UNKNOWN','gnss',None,False),('NO DATA CHG','nodata',87,True))
           for style in ('left','split')], 4, 'clock-maratype-battery-edges.png')
    print('A/ B clock study rendered', HERE)

if __name__ == '__main__':
    main()
