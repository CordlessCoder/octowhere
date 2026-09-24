"""Concept renderer for the 466x466 round panel.

Draws at 4x and box-filters down, which approximates the firmware's
antialiased coverage. Every element is also rendered into its own mask so
its ink bounds in panel pixels can be reported for the specification.
"""
from PIL import Image, ImageDraw, ImageFont
import numpy as np
import math

SS = 4
RECORD = True  # set False to skip ink measurement when rendering animation frames
N = 466
C = 233.0  # panel centre is the pixel corner (233, 233)

TOK = {
    'LIME': '#C0FE04', 'RED': '#F24723', 'ORANGE': '#F1710D', 'PURPLE': '#5500E4',
    'BLUE': '#409DE4', 'GRAY': '#888E98', 'WHITE': '#D2D3D6', 'BLACK': '#000000',
    'YELLOW': '#ECDB0B',  # proposed, from the reference board
}
FONT = {
    'shapiro': 'fonts/Shapiro.ttf',
    'mono': 'fonts/MonoR.otf',
    'bold': 'fonts/MonoB.otf',
}


def rgb(c):
    if isinstance(c, tuple):
        return c
    c = TOK.get(c, c)
    return tuple(int(c[i:i + 2], 16) for i in (1, 3, 5))


def fade(c, k):
    """Colour interpolated toward BLACK; k=1 full colour."""
    r = rgb(c)
    return tuple(int(round(v * k)) for v in r)


_fcache = {}
_icache = {}


def stroke(face, px):
    return 0


def ink_box(s, face, px):
    key = (s, face, px)
    if key not in _icache:
        f = font(face, px)
        sw = stroke(face, px)
        bb = f.getbbox(s, stroke_width=sw)
        W, H = bb[2] - bb[0] + 40, bb[3] - bb[1] + 40
        m = Image.new('L', (int(W), int(H)), 0)
        ImageDraw.Draw(m).text((20 - bb[0], 20 - bb[1]), s, font=f, fill=255, stroke_width=sw, stroke_fill=255)
        a = np.array(m)
        ys, xs = np.where(a > 100)
        _icache[key] = ((xs.min() - 20 + bb[0], ys.min() - 20 + bb[1], xs.max() + 1 - 20 + bb[0], ys.max() + 1 - 20 + bb[1]), sw)
    return _icache[key]


def font(face, px):
    key = (face, px)
    if key not in _fcache:
        _fcache[key] = ImageFont.truetype(FONT[face], int(round(px * SS)))
    return _fcache[key]


class Canvas:
    def __init__(self):
        self.img = Image.new('RGB', (N * SS, N * SS), (0, 0, 0))
        self.d = ImageDraw.Draw(self.img)
        self.log = []  # (name, bbox) in panel px, inclusive

    # -- bookkeeping -------------------------------------------------------
    def _record(self, name, mask_draw_fn):
        if name is None or not RECORD:
            return None
        m = Image.new('L', (N * SS, N * SS), 0)
        mask_draw_fn(ImageDraw.Draw(m))
        a = np.array(m.resize((N, N), Image.BOX))
        ys, xs = np.where(a > 25)
        bb = None if len(ys) == 0 else (int(xs.min()), int(ys.min()), int(xs.max()), int(ys.max()))
        self.log.append((name, bb))
        return bb

    # -- primitives --------------------------------------------------------
    def rect(self, x0, y0, x1, y1, col, name=None):
        """Axis-aligned fill covering pixels x0..x1-1, y0..y1-1."""
        box = [x0 * SS, y0 * SS, x1 * SS - 1, y1 * SS - 1]
        self.d.rectangle(box, fill=rgb(col))
        self._record(name, lambda dd: dd.rectangle(box, fill=255))

    def ring(self, r0, r1, col, name=None):
        def f(dd, fill):
            dd.ellipse([(C - r1) * SS, (C - r1) * SS, (C + r1) * SS, (C + r1) * SS], fill=fill)
            dd.ellipse([(C - r0) * SS, (C - r0) * SS, (C + r0) * SS, (C + r0) * SS], fill=0 if fill == 255 else None)
        # draw annulus via mask so the inside keeps what is there
        m = Image.new('L', (N * SS, N * SS), 0)
        md = ImageDraw.Draw(m)
        md.ellipse([(C - r1) * SS, (C - r1) * SS, (C + r1) * SS, (C + r1) * SS], fill=255)
        md.ellipse([(C - r0) * SS, (C - r0) * SS, (C + r0) * SS, (C + r0) * SS], fill=0)
        self.img.paste(Image.new('RGB', self.img.size, rgb(col)), (0, 0), m)
        if name:
            a = np.array(m.resize((N, N), Image.BOX))
            ys, xs = np.where(a > 25)
            self.log.append((name, (int(xs.min()), int(ys.min()), int(xs.max()), int(ys.max()))))

    def poly(self, pts, col, name=None):
        p = [(x * SS, y * SS) for x, y in pts]
        self.d.polygon(p, fill=rgb(col))
        self._record(name, lambda dd: dd.polygon(p, fill=255))

    def radial_bar(self, ang_deg, r0, r1, w, col, name=None):
        """Quadrilateral from radius r0 to r1, width w, angle clockwise from 12."""
        a = math.radians(ang_deg)
        ux, uy = math.sin(a), -math.cos(a)
        vx, vy = -uy, ux
        h = w / 2
        pts = [(C + ux * r0 + vx * h, C + uy * r0 + vy * h), (C + ux * r1 + vx * h, C + uy * r1 + vy * h),
               (C + ux * r1 - vx * h, C + uy * r1 - vy * h), (C + ux * r0 - vx * h, C + uy * r0 - vy * h)]
        self.poly(pts, col, name)

    # -- text --------------------------------------------------------------
    def ink(self, s, face, px):
        """Ink box of s drawn at origin, in 4x units, as (l, t, r, b)."""
        return ink_box(s, face, px)

    def ink_size(self, s, face, px):
        (l, t, r, b), _ = self.ink(s, face, px)
        return (r - l) / SS, (b - t) / SS

    def text(self, s, face, px, col, x=None, y=None, cx=None, cy=None, right=None, bottom=None, name=None):
        """Place text by its ink box. x/y = ink left/top, cx/cy = ink centre, right/bottom = ink edge."""
        f = font(face, px)
        (l, t, r, b), sw = self.ink(s, face, px)
        w, h = r - l, b - t
        if cx is not None:
            X = cx * SS - w / 2
        elif right is not None:
            X = right * SS - w
        else:
            X = x * SS
        if cy is not None:
            Y = cy * SS - h / 2
        elif bottom is not None:
            Y = bottom * SS - h
        else:
            Y = y * SS
        ox, oy = X - l, Y - t
        self.d.text((ox, oy), s, font=f, fill=rgb(col), stroke_width=sw, stroke_fill=rgb(col))
        return self._record(name, lambda dd: dd.text((ox, oy), s, font=f, fill=255, stroke_width=sw, stroke_fill=255))

    # -- output ------------------------------------------------------------
    def render(self, offpanel=True):
        im = self.img.resize((N, N), Image.BOX)
        if offpanel:
            m = Image.new('L', (N * SS, N * SS), 0)
            ImageDraw.Draw(m).ellipse([0, 0, N * SS - 1, N * SS - 1], fill=255)
            m = m.resize((N, N), Image.BOX)
            bg = Image.new('RGB', (N, N), (28, 28, 28))
            bg.paste(im, (0, 0), m)
            im = bg
        return im

    def save(self, path, offpanel=True):
        im = self.render(offpanel)
        im.save(path)
        return im


# -- shared symbol system: 5x5 grid, 5 px modules, 4 px padding, 33x33 tile --
def icon(cv, x, y, tile, rows, name='icon'):
    cv.rect(x, y, x + 33, y + 33, tile, name=name)
    for j, row in enumerate(rows.split()):
        for i, bit in enumerate(row):
            if bit == '1':
                cv.rect(x + 4 + 5 * i, y + 4 + 5 * j, x + 9 + 5 * i, y + 9 + 5 * j, 'BLACK')


def chord_half(y, r=231.0):
    """Half-width of the circle of radius r at pixel row y (row centre)."""
    dy = abs(y + 0.5 - C)
    return math.sqrt(max(0.0, r * r - dy * dy))


def rtext(cv, s, face, px, col, ang_deg, r):
    """Text centred on the point at radius r, turned so its top faces outward."""
    f = font(face, px)
    (l, t, rr, b), sw = ink_box(s, face, px)
    w, h = rr - l, b - t
    pad = 8
    m = Image.new('L', (int(w + 2 * pad), int(h + 2 * pad)), 0)
    ImageDraw.Draw(m).text((pad - l, pad - t), s, font=f, fill=255, stroke_width=sw, stroke_fill=255)
    m = m.rotate(-ang_deg, resample=Image.BICUBIC, expand=True)
    a = math.radians(ang_deg)
    px_, py_ = C + math.sin(a) * r, C - math.cos(a) * r
    ox, oy = int(round(px_ * SS - m.width / 2)), int(round(py_ * SS - m.height / 2))
    cv.img.paste(Image.new('RGB', m.size, rgb(col)), (ox, oy), m)


def pen(cv, s, face, px, col, x, baseline, name=None):
    """Draw s with its pen at (x, baseline), as the firmware's baseline mode does."""
    f = font(face, px)
    sw = stroke(face, px)
    X, Y = x * SS, baseline * SS
    cv.d.text((X, Y), s, font=f, fill=rgb(col), anchor='ls', stroke_width=sw, stroke_fill=rgb(col))
    return cv._record(name, lambda dd: dd.text((X, Y), s, font=f, fill=255, anchor='ls', stroke_width=sw, stroke_fill=255))


def adv(face, px, s='0'):
    return font(face, px).getlength(s) / SS


def hhmm(cv, text, px, col, x0, baseline, colon_cell, name='time'):
    """Tabular HH:MM on fixed columns; the colon gets a narrower cell.
    x0 is the pen of the first digit. Returns the column pens."""
    a = adv('bold', px)
    pens = [x0, x0 + a, x0 + 2 * a - (a - colon_cell) / 2, x0 + 2 * a + colon_cell, x0 + 3 * a + colon_cell]
    f = font('bold', px)
    sw = stroke('bold', px)

    def draw(dd, fill):
        for ch, p in zip(text, pens):
            dd.text((p * SS, baseline * SS), ch, font=f, fill=fill, anchor='ls', stroke_width=sw, stroke_fill=fill)
    draw(cv.d, rgb(col))
    cv._record(name, lambda dd: draw(dd, 255))
    return pens


def frame_for(module):
    """Outline thickness: a quarter of the module, rounded half up, never below 2 px.
    5 -> 2, 8 -> 2, 10 -> 3, 16 -> 4."""
    return max(2, int(module / 4 + 0.5))


def icon_sized(cv, x, y, tile, rows, module=5, pad=4, shown=None, name='icon', outlined=True,
               rows_shown=5, col=None):
    """The 5x5 symbol system at any integer module size.
    Outlined (the standard since 24 Sep): a frame in the state colour, BLACK inside, and the
    modules in the state colour. Filled: a solid tile with BLACK modules.
    `shown` limits which modules draw by index; `rows_shown` limits by row, top down."""
    size = 5 * module + 2 * pad
    c = col or tile
    cv.rect(x, y, x + size, y + size, c, name=name)
    mod = 'BLACK'
    if outlined:
        f = frame_for(module)
        cv.rect(x + f, y + f, x + size - f, y + size - f, 'BLACK')
        mod = c
    k = 0
    for j, row in enumerate(rows.split()):
        for i, bit in enumerate(row):
            if bit == '1':
                if (shown is None or k in shown) and j < rows_shown:
                    cv.rect(x + pad + module * i, y + pad + module * j,
                            x + pad + module * (i + 1), y + pad + module * (j + 1), mod)
                k += 1
    return size


def cap_height(face, px):
    (l, t, r, b), _ = ink_box('H', face, px)
    return (b - t) / SS


def lsb(s, face, px):
    (l, t, r, b), _ = ink_box(s, face, px)
    return l / SS


BAND_CLIP_R = 232.0   # the ring's outer radius: a band ends exactly where the page's ring does


def band(cv, y0, y1, col, r=BAND_CLIP_R, name=None):
    """Full-width band, rows y0..y1-1, clipped to the page's own circle.
    In page coordinates the clip never changes, so during a swipe the band's ends are arcs
    that travel with the page. Each row is one span with an antialiased pixel at each end."""
    m = Image.new('L', (N * SS, N * SS), 0)
    md = ImageDraw.Draw(m)
    md.ellipse([(C - r) * SS, (C - r) * SS, (C + r) * SS - 1, (C + r) * SS - 1], fill=255)
    rows = Image.new('L', (N * SS, N * SS), 0)
    ImageDraw.Draw(rows).rectangle([0, y0 * SS, N * SS - 1, y1 * SS - 1], fill=255)
    from PIL import ImageChops
    m = ImageChops.multiply(m, rows)
    cv.img.paste(Image.new('RGB', cv.img.size, rgb(col)), (0, 0), m)
    if name and RECORD:
        a = np.array(m.resize((N, N), Image.BOX))
        ys, xs = np.where(a > 25)
        cv.log.append((name, (int(xs.min()), int(ys.min()), int(xs.max()), int(ys.max()))))
clipped_band = band
