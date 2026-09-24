"""OCTOWHERE as a vertical strip right of the icon, inverting what it crosses.

Rule: where the strip crosses the field (BLACK ground, WHITE ink) it is WHITE with BLACK
letters; where it crosses the band (band colour ground, BLACK ink) it is BLACK with letters in
the band's colour. It is clipped to the page's circle like the band.
"""
import numpy as np
from PIL import ImageChops
import lib
from lib import *
from industrial import v1, FIX, BAND

STRIP = dict(x0=366, w=32)      # 8 px after the icon (x 262..357), two icon modules wide


def _mask_rect_circle(x0, y0, x1, y1, r=BAND_CLIP_R):
    m = Image.new('L', (N * SS, N * SS), 0)
    ImageDraw.Draw(m).ellipse([(C - r) * SS, (C - r) * SS, (C + r) * SS - 1, (C + r) * SS - 1], fill=255)
    rr = Image.new('L', (N * SS, N * SS), 0)
    ImageDraw.Draw(rr).rectangle([x0 * SS, y0 * SS, x1 * SS - 1, y1 * SS - 1], fill=255)
    return ImageChops.multiply(m, rr)


def _text_mask(s, face, px, cx, cy, up=True, tracking=0.0, split=None, boundary=None):
    """Text turned a quarter turn. up=True reads bottom to top.
    Placed with its ink centred on (cx, cy), or, with split=n and boundary=y, so that the gap
    between character n-1 and character n lands on row y."""
    f = font(face, px)
    adv_ = [f.getlength(ch) + tracking * px * SS for ch in s]
    W_ = int(sum(adv_) + 40 * SS)
    H_ = int(px * SS * 2)

    def draw(sub, start=0):
        m = Image.new('L', (W_, H_), 0)
        d = ImageDraw.Draw(m)
        x = 20 * SS
        for i, (ch, a) in enumerate(zip(s, adv_)):
            if start <= i < start + len(sub):
                d.text((x, px * SS * 1.4), ch, font=f, fill=255, anchor='ls')
            x += a
        return m
    m = draw(s)
    bb = m.getbbox()
    off = None
    if split is not None:
        left = draw(s[:split]).getbbox()[2]          # ink right of the first part
        right = draw(s[split:], split).getbbox()[0]   # ink left of the second part
        off = (left + right) / 2 - bb[0]
    m = m.crop(bb)
    w = m.width
    m = m.transpose(Image.ROTATE_90 if up else Image.ROTATE_270)
    full = Image.new('L', (N * SS, N * SS), 0)
    x = int(round(cx * SS - m.width / 2))
    if off is None:
        y = int(round(cy * SS - m.height / 2))
    else:
        y = int(round(boundary * SS - (w - off))) if up else int(round(boundary * SS - off))
    full.paste(m, (x, y))
    return full


def strip(cv, band_col, text='OCTOWHERE', face='shapiro', px=22, cy=233, up=True, tracking=0.0,
          y_span=None, x0=STRIP['x0'], w=STRIP['w'], k=1.0, split=None, boundary=BAND[0]):
    y0, y1 = y_span or (0, N)
    whole = _mask_rect_circle(x0, y0, x0 + w, y1)
    band_rows = _mask_rect_circle(0, BAND[0], N, BAND[1], r=1e4)
    in_band = ImageChops.multiply(whole, band_rows)
    in_field = ImageChops.subtract(whole, in_band)
    solid = lambda c: Image.new('RGB', cv.img.size, rgb(c))
    cv.img.paste(solid('WHITE'), (0, 0), in_field)
    cv.img.paste(solid('BLACK'), (0, 0), in_band)
    if k <= 0:
        return
    t = _text_mask(text, face, px, x0 + w / 2, cy, up, tracking, split, boundary)
    cv.img.paste(solid('BLACK'), (0, 0), ImageChops.multiply(t, in_field))
    cv.img.paste(solid(band_col), (0, 0), ImageChops.multiply(t, in_band))


def face_with_strip(state='gnss', **kw):
    cv = v1(state)
    band = {'stopped': 'ORANGE', 'nodata': 'RED'}.get(state, 'WHITE')
    strip(cv, band, **kw)
    return cv


if __name__ == '__main__':
    lib.RECORD = False
    variants = [
        (dict(split=4), 'SHAPIRO 22, READING UP, OCTO IN THE BAND'),
        (dict(split=4, up=False), 'SHAPIRO 22, READING DOWN, WHERE IN THE BAND'),
        (dict(split=4, face='bold', px=24, tracking=0.12), 'MONO BOLD 24, TRACKED, READING UP'),
        (dict(), 'CENTRED (EARLIER): THE EDGE CUTS LETTERS'),
    ]
    f = ImageFont.truetype('fonts/MonoR.otf', 16)
    sh = Image.new('RGB', (486 * 4 - 20, 500), (28, 28, 28)); d = ImageDraw.Draw(sh)
    for i, (kw, label) in enumerate(variants):
        sh.paste(face_with_strip('gnss', **kw).render(), (i * 486, 0))
        d.text((i * 486, 476), label, font=f, fill=(210, 211, 214))
    sh.save('out/study-wordmark.png')
    sh = Image.new('RGB', (486 * 5 - 20, 466), (28, 28, 28))
    for i, st in enumerate(['gnss', 'rtc', 'stopped', 'nozone', 'nodata']):
        im = face_with_strip(st, split=4).render()
        im.save('out/wordmark-%s.png' % st)
        sh.paste(im, (i * 486, 0))
    sh.save('out/study-wordmark-states.png')
