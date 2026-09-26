"""OCTOWHERE in Marathon Shapiro, turned so the letter tops face right (reads top to bottom),
right of the icon, with no ground of its own: each letter pixel takes the inverse of what is
under it. WHITE over the BLACK field; BLACK over the band, whatever the band's colour."""
import lib
from lib import *
from PIL import ImageChops
from industrial import v1, FIX, BAND
from wordmark import _text_mask

X0 = 366   # ink left, 8 px after the icon


def mark(cv, px=24, cy=None, split=None, boundary=BAND[0], x0=X0, text='OCTOWHERE', tracking=0.0):
    f = font('shapiro', px)
    cap = cap_height('shapiro', px)
    cx = x0 + cap / 2
    t = _text_mask(text, 'shapiro', px, cx, cy if cy is not None else 233, up=False, tracking=tracking,
                   split=split, boundary=boundary)
    rows = Image.new('L', t.size, 0)
    ImageDraw.Draw(rows).rectangle([0, BAND[0] * SS, N * SS - 1, BAND[1] * SS - 1], fill=255)
    on_band = ImageChops.multiply(t, rows)
    on_field = ImageChops.subtract(t, on_band)
    cv.img.paste(Image.new('RGB', cv.img.size, rgb('WHITE')), (0, 0), on_field)
    cv.img.paste(Image.new('RGB', cv.img.size, rgb('BLACK')), (0, 0), on_band)
    bb = t.getbbox()
    return tuple(v / SS for v in bb)


if __name__ == '__main__':
    lib.RECORD = False
    vs = [
        (dict(px=24, split=4), 'SHAPIRO 24, BAND EDGE BETWEEN OCTO AND WHERE'),
        (dict(px=24), 'SHAPIRO 24, CENTRED ON THE PANEL'),
        (dict(px=30), 'SHAPIRO 30, CENTRED ON THE PANEL'),
        (dict(px=30, cy=None, split=4, boundary=BAND[1]), 'SHAPIRO 30, OCTO+W.. ENDS: EDGE AT BAND FOOT'),
    ]
    f = ImageFont.truetype('fonts/MonoR.otf', 15)
    sh = Image.new('RGB', (486 * 4 - 20, 500), (28, 28, 28)); d = ImageDraw.Draw(sh)
    for i, (kw, l) in enumerate(vs):
        cv = v1('gnss')
        bb = mark(cv, **kw)
        sh.paste(cv.render(), (i * 486, 0))
        d.text((i * 486, 476), l, font=f, fill=(210, 211, 214))
        print(l, [round(v) for v in bb])
    sh.save('out/study-wordmark2.png')


# ---- chosen placement: 26 px, the O|W gap on the band's top edge ----------------------------
MARK_PX = 26
MARK_TEXT = 'OCTOWHERE'
MARK_SPLIT = 4


def _letters(px=MARK_PX, x0=X0, boundary=BAND[0]):
    """Per-letter masks and advance cells, placed as the whole word is placed."""
    s = MARK_TEXT
    f = font('shapiro', px)
    adv_ = [f.getlength(ch) for ch in s]
    W_ = int(sum(adv_) + 40 * SS)
    H_ = int(px * SS * 2)

    def draw(idx):
        m = Image.new('L', (W_, H_), 0)
        d = ImageDraw.Draw(m)
        x = 20 * SS
        for i, (ch, a) in enumerate(zip(s, adv_)):
            if i in idx:
                d.text((x, px * SS * 1.4), ch, font=f, fill=255, anchor='ls')
            x += a
        return m
    whole = draw(range(len(s)))
    bb = whole.getbbox()
    left = draw(range(MARK_SPLIT)).getbbox()[2]
    right = draw(range(MARK_SPLIT, len(s))).getbbox()[0]
    off = (left + right) / 2 - bb[0]
    cap = cap_height('shapiro', px)
    cx = x0 + cap / 2
    out = []
    pens = [20 * SS + sum(adv_[:i]) for i in range(len(s))]
    for i in range(len(s)):
        m = draw([i]).crop(bb).transpose(Image.ROTATE_270)
        full = Image.new('L', (N * SS, N * SS), 0)
        X = int(round(cx * SS - m.width / 2))
        Y = int(round(boundary * SS - off))
        full.paste(m, (X, Y))
        # advance cell in panel rows: down-reading, so x in the upright raster maps to y
        y0 = (Y + (pens[i] - bb[0])) / SS
        y1 = (Y + (pens[i] + adv_[i] - bb[0])) / SS
        out.append((full, (x0, y0 + 1, x0 + cap, y1 - 1)))
    return out


_LCACHE = {}


def mark_reveal(cv, k=1.0, band_col='WHITE'):
    """The chosen mark, cell-revealed top to bottom at progress k."""
    if 'l' not in _LCACHE:
        _LCACHE['l'] = _letters()
    L = _LCACHE['l']
    n = len(L)
    shown = k * (n + 1)
    rows = Image.new('L', (N * SS, N * SS), 0)
    ImageDraw.Draw(rows).rectangle([0, BAND[0] * SS, N * SS - 1, BAND[1] * SS - 1], fill=255)
    white = Image.new('RGB', cv.img.size, rgb('WHITE'))
    black = Image.new('RGB', cv.img.size, rgb('BLACK'))
    for i, (m, cell) in enumerate(L):
        if i < shown - 1:
            t = m
        elif i < shown:
            t = Image.new('L', (N * SS, N * SS), 0)
            x0, y0, x1, y1 = cell
            ImageDraw.Draw(t).rectangle([round(x0) * SS, round(y0) * SS, round(x1) * SS - 1, round(y1) * SS - 1], fill=255)
        else:
            continue
        on_band = ImageChops.multiply(t, rows)
        cv.img.paste(white, (0, 0), ImageChops.subtract(t, on_band))
        cv.img.paste(black, (0, 0), on_band)


def mark_geometry():
    L = _letters()
    union = None
    for m, _ in L:
        union = m if union is None else ImageChops.lighter(union, m)
    bb = union.getbbox()
    return tuple(v / SS for v in bb), [tuple(round(v, 1) for v in c) for _, c in L]
