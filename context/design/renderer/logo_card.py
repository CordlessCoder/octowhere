"""The logo card: the identity's handover to the clock, replacing the band handover.

After the identity's hold: the page fills LIME with the mark knocked out in stripes, then solid,
then inverted (a LIME mark on black), then the mark scaled up for one frame, then the clock.
The motion follows the owner's reference; the mark is OCTOWHERE's own. Three candidates:

  M1  an octagon ring with an octagonal dot at the centre: the octagon, and a position in it
  M2  an octagon ring with a north pointer on its inner top edge
  M3  M1 drawn on the 5 x 5 module grid, as the icons are: 01110 10001 10101 10001 01110
  M4  M1 with the dot off centre, up and right: a position, not a centre
  M40 the chosen mark: 10101 00000 10001 00000 10101, eight modules on the octagon's points
"""
import math
import lib
lib.RECORD = False
from lib import *
from PIL import ImageChops
from identity import finish, circle

W = 466
MARK = 'L1H'             # chosen 25 Sep: L1 with a hatched centre (marks_frame.py). Was M40
APO_OUT, APO_IN, APO_DOT = 96, 64, 19      # octagon apothems, px (M1, M2)
GRID = {'M3': '01110 10001 10101 10001 01110',
        'M40': '10101 00000 10001 00000 10101'}   # eight modules on the octagon's points
M3_MOD = 38
M4_DOT = (24, -24)                          # M4: the dot off centre, up and right
STRIPE_PITCH, STRIPE_W = 5, 1
POP_BY_MARK = {'L1H': 1.9}                  # the scale-up; 2.3 for the block marks
POP = 2.3

# frames from the card's start
F_STRIPES = (0, 2)
F_SOLID = (3, 6)
F_INVERT = (7, 10)
F_POP = 11
F_IMPACT = 12                                # the scaled mark inverted: the impact frame
F_CLOCK = 13


def octagon(apo, cx=233.0, cy=233.0, s=1.0):
    R = apo * s / math.cos(math.radians(22.5))
    return [((cx + R * math.cos(math.radians(22.5 + 45 * k))) * SS,
             (cy + R * math.sin(math.radians(22.5 + 45 * k))) * SS) for k in range(8)]


def mark_mask(s=1.0, mark=None):
    mark = mark or MARK
    m = Image.new('L', (W * SS, W * SS), 0)
    d = ImageDraw.Draw(m)
    if mark in ('M1', 'M2', 'M4'):
        d.polygon(octagon(APO_OUT, s=s), fill=255)
        d.polygon(octagon(APO_IN, s=s), fill=0)
        if mark == 'M1':
            d.polygon(octagon(APO_DOT, s=s), fill=255)
        elif mark == 'M4':
            d.polygon(octagon(APO_DOT, 233 + M4_DOT[0] * s, 233 + M4_DOT[1] * s, s=s), fill=255)
        else:
            # north pointer: a triangle on the inner top edge, pointing at the centre
            top = 233 - APO_IN * s
            w = 22 * s
            d.polygon([((233 - w) * SS, top * SS), ((233 + w) * SS, top * SS),
                       (233 * SS, (top + 34 * s) * SS)], fill=255)
    else:
        mod = M3_MOD * s
        x0 = 233 - 2.5 * mod
        for j, row in enumerate(GRID[mark].split()):
            for i, b in enumerate(row):
                if b == '1':
                    d.rectangle([(x0 + i * mod) * SS, (x0 + j * mod) * SS,
                                 (x0 + (i + 1) * mod) * SS - 1, (x0 + (j + 1) * mod) * SS - 1], fill=255)
    return m


def stripes():
    m = Image.new('L', (W * SS, W * SS), 0)
    d = ImageDraw.Draw(m)
    x = 233 % STRIPE_PITCH
    while x < W:
        d.rectangle([x * SS, 0, (x + STRIPE_W) * SS - 1, W * SS - 1], fill=255)
        x += STRIPE_PITCH
    return m


_ST = None


def paint(cv, mask, col):
    cv.img.paste(Image.new('RGB', cv.img.size, rgb(col)), (0, 0), ImageChops.multiply(mask, circle(232)))


def card(n, mark=None):
    """Frame n of the card. Returns None from F_CLOCK on (the clock takes over)."""
    global _ST
    mark = mark or MARK
    if mark == 'L1H':
        return card_l1h(n)
    cv = Canvas()
    if n <= F_STRIPES[1]:
        if _ST is None:
            _ST = stripes()
        paint(cv, Image.new('L', cv.img.size, 255), 'LIME')
        paint(cv, ImageChops.multiply(mark_mask(mark=mark), _ST), 'BLACK')
    elif n <= F_SOLID[1]:
        paint(cv, Image.new('L', cv.img.size, 255), 'LIME')
        paint(cv, mark_mask(mark=mark), 'BLACK')
    elif n <= F_INVERT[1]:
        paint(cv, mark_mask(mark=mark), 'LIME')
    elif n == F_POP:
        paint(cv, mark_mask(POP, mark=mark), 'LIME')
    elif n == F_IMPACT:
        paint(cv, Image.new('L', cv.img.size, 255), 'LIME')
        paint(cv, mark_mask(POP, mark=mark), 'BLACK')
    else:
        return None
    return finish(cv)


def card_l1h(n):
    """The chosen mark. First stage: the hatched tile alone (the stripes of the reference, as the
    hatch); then the full mark; inverted; 1.9x; impact."""
    import marks_frame as MF
    pop = POP_BY_MARK['L1H']
    cv = Canvas()
    full = Image.new('L', cv.img.size, 255)
    if n <= F_STRIPES[1]:
        paint(cv, full, 'LIME'); paint(cv, MF.hatched_mask('L1', parts=('tile',)), 'BLACK')
    elif n <= F_SOLID[1]:
        paint(cv, full, 'LIME'); paint(cv, MF.hatched_mask('L1'), 'BLACK')
    elif n <= F_INVERT[1]:
        paint(cv, MF.hatched_mask('L1'), 'LIME')
    elif n == F_POP:
        paint(cv, MF.hatched_mask('L1', pop), 'LIME')
    elif n == F_IMPACT:
        paint(cv, full, 'LIME'); paint(cv, MF.hatched_mask('L1', pop), 'BLACK')
    else:
        return None
    return finish(cv)


if __name__ == '__main__':
    # the three marks at each stage
    f = ImageFont.truetype('fonts/MonoB.otf', 16)
    stages = [(1, 'THE HATCHED TILE'), (4, 'SOLID'), (8, 'INVERTED'), (11, 'SCALED UP 1.9X'), (12, 'IMPACT: INVERTED AGAIN')]
    sc = 0.5
    w = int(W * sc)
    marks = ['L1H']
    sh = Image.new('RGB', (len(stages) * (w + 12) + 130, len(marks) * (w + 12) + 50), (20, 20, 22))
    d = ImageDraw.Draw(sh)
    for i, (n, name) in enumerate(stages):
        d.text((120 + i * (w + 12), 14), name, font=f, fill=(210, 211, 214))
    for j, mk in enumerate(marks):
        d.text((12, 44 + j * (w + 12) + w // 2 - 8), mk, font=f, fill=(210, 211, 214))
        for i, (n, name) in enumerate(stages):
            sh.paste(card(n, mk).resize((w, w), Image.LANCZOS), (120 + i * (w + 12), 44 + j * (w + 12)))
    sh.save('out/logo-mark.png')
