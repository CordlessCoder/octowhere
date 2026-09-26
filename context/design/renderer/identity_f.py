"""Identity F: hairlines -> blocks -> stretched columns -> tall title, after the owner's reference.

Principles taken, not the reference's letterforms: a centre row as the axis of growth; hairlines
first; PURPLE blocks that stretch into full-height columns; a lime title that resolves tall,
with a microtext row, a halftone figure, registration marks, and a flicker before it hands
over.

OCTOWHERE-specific: the hairlines sit on the final title's stem edges, so the opening lines
are the skeleton of the word; the halftone figure is an octagon; the microtext row is real
data (version barcode, the device's own 5x5 glyphs, UTC in pixel digits, two text lines).
"""
import math
import random
import lib
lib.RECORD = False
from lib import *
from PIL import ImageChops
from identity import circle, text_mask, place, rect_mask, finish, band_word
from anim import save_gif, DT

W = 466
CY = 233
TITLE_PX = 46
TITLE_SY = 2.2          # the resting title is Shapiro stretched 2.2x tall
WORD = 'OCTOWHERE'


# ---- geometry of the final title, per letter -----------------------------------------------------------
def letter_boxes(px=TITLE_PX):
    """Ink x-ranges (panel px) of each letter of the centred title, and stem edges from column coverage."""
    f = font('shapiro', px)
    adv_ = [f.getlength(ch) for ch in WORD]
    Wd, Hd = int(sum(adv_) + 40 * SS), int(px * SS * 2)
    whole = Image.new('L', (Wd, Hd), 0)
    d = ImageDraw.Draw(whole)
    x = 20 * SS
    for ch, a in zip(WORD, adv_):
        d.text((x, px * SS * 1.4), ch, font=f, fill=255, anchor='ls')
        x += a
    bb = whole.getbbox()
    width = (bb[2] - bb[0]) / SS
    left0 = 233 - width / 2
    out = []
    x = 20 * SS
    for ch, a in zip(WORD, adv_):
        m = Image.new('L', (Wd, Hd), 0)
        ImageDraw.Draw(m).text((x, px * SS * 1.4), ch, font=f, fill=255, anchor='ls')
        b = m.getbbox()
        a_arr = np.array(m.crop((b[0], b[1], b[2], b[3])))
        cov = (a_arr > 128).mean(axis=0)
        stems = []
        on = False
        for i, v in enumerate(cov):
            if v > 0.8 and not on:
                start, on = i, True
            elif v <= 0.8 and on:
                stems.append((start, i)); on = False
        if on:
            stems.append((start, len(cov)))
        lx = left0 + (b[0] - bb[0]) / SS
        rx = left0 + (b[2] - bb[0]) / SS
        edges = []
        for s0, s1 in stems:
            edges += [lx + s0 / SS, lx + s1 / SS]
        if not edges:
            edges = [lx, rx]
        out.append((ch, lx, rx, edges))
        x += a
    return out


LETTERS = letter_boxes()


# ---- pieces ---------------------------------------------------------------------------------------------
def hairlines(cv, length_k, gap=0):
    """1 px LIME lines on every stem edge; outer letters longer, as in the reference."""
    clip = circle()
    for ch, lx, rx, edges in LETTERS:
        dist = abs((lx + rx) / 2 - 233) / 200            # 0 centre .. ~1 edge
        full = 40 + 380 * dist                            # outer groups reach furthest
        h = max(4, full * length_k)
        for e in edges:
            x = round(e)
            if gap and h / 2 <= gap + 1:
                continue
            if gap:
                m =ImageChops.multiply(clip, rect_mask(x, round(CY - h / 2), x + 1, CY - gap))
                cv.img.paste(Image.new('RGB', cv.img.size, rgb('LIME')), (0, 0), m)
                m = ImageChops.multiply(clip, rect_mask(x, CY + gap, x + 1, round(CY + h / 2)))
                cv.img.paste(Image.new('RGB', cv.img.size, rgb('LIME')), (0, 0), m)
            else:
                m = ImageChops.multiply(clip, rect_mask(x, round(CY - h / 2), x + 1, round(CY + h / 2)))
                cv.img.paste(Image.new('RGB', cv.img.size, rgb('LIME')), (0, 0), m)


def centre_dots(cv):
    for x in range(150, 318, 10):
        cv.rect(x, CY - 1, x + 2, CY + 1, 'LIME')


def blocks(cv, h_k, outer_only=False):
    """PURPLE blocks on the centre row, one per letter, h_k of the letter's own width tall."""
    clip = circle()
    for i, (ch, lx, rx, edges) in enumerate(LETTERS):
        if outer_only and i not in (0, 1, 7, 8):
            continue
        h = max(3, (rx - lx) * h_k)
        m = ImageChops.multiply(clip, rect_mask(round(lx), round(CY - h / 2), round(rx), round(CY + h / 2)))
        cv.img.paste(Image.new('RGB', cv.img.size, rgb('PURPLE')), (0, 0), m)


def arrow_ticks(cv):
    """Small LIME wedges on the centre row at the stem edges, pointing inward."""
    for ch, lx, rx, edges in LETTERS:
        for j, e in enumerate(edges[:2]):
            x = round(e)
            s = 1 if j == 0 else -1
            cv.poly([(x, CY - 5), (x + 6 * s, CY), (x, CY + 5)], 'LIME')


def title(cv, sy, col):
    m = text_mask(WORD, 'shapiro', TITLE_PX, sy=sy)
    place(cv, m, col, 233, CY, clip=circle())


# ---- the microtext row and ornaments -------------------------------------------------------------------------
PIX = {  # 3 x 5 pixel digits
    '0': '111 101 101 101 111', '1': '010 110 010 010 111', '2': '111 001 111 100 111',
    '3': '111 001 111 001 111', '4': '101 101 111 001 001', '5': '111 100 111 001 111',
    '6': '111 100 111 101 111', '7': '111 001 010 010 010', '8': '111 101 111 101 111',
    '9': '111 101 111 001 111',
}


def pix_digits(cv, s, x, y, m=3, col='LIME'):
    for ch in s:
        if ch == ' ':
            x += 2 * m
            continue
        for j, row in enumerate(PIX[ch].split()):
            for i, b in enumerate(row):
                if b == '1':
                    cv.rect(x + i * m, y + j * m, x + (i + 1) * m, y + (j + 1) * m, col)
        x += 4 * m
    return x


def glyph(cv, rows, x, y, m=3, col='LIME'):
    for j, row in enumerate(rows.split()):
        for i, b in enumerate(row):
            if b == '1':
                cv.rect(x + i * m, y + j * m, x + (i + 1) * m, y + (j + 1) * m, col)
    return x + 5 * m


def barcode(cv, data, x, y, h=15, col='LIME'):
    """Bars from the bits of `data` (the version string): 2 px bar for 1, 1 px for 0, 1 px gaps."""
    for byte in data.encode():
        for k in range(4):
            bit = (byte >> k) & 1
            w = 2 if bit else 1
            cv.rect(x, y, x + w, y + h, col)
            x += w + 2
    return x


def micro_row(cv, top, x=None):
    if x is None:                                   # dry run to centre the row
        end = micro_row(Canvas(), top, 0)
        x = round(233 - end / 2)
    x = barcode(cv, '0.1.0', x, top) + 10
    x = glyph(cv, '10001 01010 00100 01010 10001', x, top) + 10          # cross
    x = glyph(cv, '01110 11011 10001 11011 01110', x, top) + 12          # octagon
    x = pix_digits(cv, '12 07', x, top) + 10                             # UTC hh mm, pixel digits
    x = glyph(cv, '00100 01010 10101 01010 00100', x, top) + 12          # the GNSS glyph
    cv.text('OCTOWHERE 0.1.0', 'mono', 14, 'LIME', x=x, y=top - 2)
    cv.text('SELF TEST 6/6 OK', 'mono', 14, 'LIME', x=x, y=top + 14)
    return x + font('mono', 14).getlength('SELF TEST 6/6 OK') / SS


def halftone_octagon(cv, k=1.0, seed=4):
    """o and . marks in PURPLE on a halftone octagon ring, denser toward one side."""
    rnd = random.Random(seed)
    R = 172
    for y in range(20, 446, 8):
        for x in range(20, 446, 8):
            dx, dy = x - 233, y - 233
            # octagon 'radius' (L-inf and rotated L-inf combined)
            r8 = max(abs(dx), abs(dy), (abs(dx) + abs(dy)) / math.sqrt(2))
            band = abs(r8 - R)
            if band > 30 or math.hypot(dx, dy) > 222:
                continue
            if 40 <= x <= 426 and (184 <= y <= 286 or 292 <= y <= 334):   # keep the title and row clear
                continue
            ang = math.atan2(dy, dx)
            dens = (1.1 - band / 30) * (0.5 + 0.5 * math.cos(ang - 0.8)) * k
            if rnd.random() < dens:
                if rnd.random() < 0.6:
                    cv.rect(x, y, x + 6, y + 6, 'PURPLE'); cv.rect(x + 2, y + 2, x + 4, y + 4, 'BLACK')
                else:
                    cv.rect(x + 1, y + 1, x + 5, y + 5, 'PURPLE')


def registration(cv):
    for sx in (-1, 1):
        for sy in (-1, 1):
            x, y = 233 + sx * 148, 233 + sy * 148
            cv.rect(x - 14, y, x + 15, y + 1, 'GRAY')
            cv.rect(x, y - 14, x + 1, y + 15, 'GRAY')
            qx, qy = x - sx * 22, y - sy * 22
            cv.rect(qx - 4, qy - 4, qx + 5, qy + 5, fade('GRAY', 0.6))
    for x in (38, 428):
        cv.rect(x - 9, CY, x + 10, CY + 1, 'LIME')
        cv.rect(x, CY - 9, x + 1, CY + 10, 'LIME')


# ---- frames -----------------------------------------------------------------------------------------------
def frame(t):
    cv = Canvas()
    if t < 100:                          # hairlines grow from the centre row, purple dashes on the outer letters
        k = 0.25 + 0.75 * t / 100
        hairlines(cv, k * 0.6, gap=6)
        centre_dots(cv)
        blocks(cv, 0.12, outer_only=True)
    elif t < 200:                        # hairlines to full height; blocks on every letter
        k = (t - 100) / 100
        hairlines(cv, 0.6 + 0.8 * k, gap=6)
        blocks(cv, 0.2 + 0.5 * k)
    elif t < 300:                        # blocks stretch into columns, hairlines inside, arrow ticks
        k = (t - 200) / 100
        blocks(cv, 0.7 + 5 * k)
        hairlines(cv, 1.4)
        arrow_ticks(cv)
    elif t < 360:                        # the stretched title: every letter a full-height PURPLE column
        title(cv, 14, 'PURPLE')
        hairlines(cv, 1.6)
    elif t < 460:                        # compress in steps: 14 -> 7 -> 3.5 -> 2.2
        step = [7, 3.5, 2.2][min(2, (t - 360) // 34)]
        title(cv, step, 'PURPLE' if step > 3 else 'LIME')
    else:                                # rest: tall LIME title and its ornaments; flicker 520-600
        halftone_octagon(cv)
        registration(cv)
        on = not (520 <= t < 600 and ((t - 520) // 20) % 2 == 0)
        if on:
            title(cv, TITLE_SY, 'LIME')
        micro_row(cv, 300)
    return finish(cv)


def handover(t):
    """900 ms on: the ornaments cut; the band builds in two steps; the clock's entry follows."""
    cv = Canvas()
    if t < 40:
        title(cv, TITLE_SY, 'LIME')
    else:
        band_word(cv)
    return finish(cv)


TIMES = [0, 60, 120, 200, 260, 320, 380, 430, 480, 540, 700, 940]
NOTES = {
    0: 'Short LIME hairlines on the stem edges of the letters-to-be; PURPLE dashes on the outer ones',
    60: 'Hairlines grow from the centre row; the outer groups reach furthest',
    120: 'Every letter gets a PURPLE block on the centre row',
    200: 'Blocks stretch; hairlines run full height',
    260: 'Columns; arrow ticks at the centre row',
    320: 'The stretched title: the word at 14x height',
    380: 'Compresses in hard steps, 7x',
    430: '2.2x, turns LIME',
    480: 'Rest: halftone octagon, registration marks, microtext row',
    540: 'The title flickers, two frames off, 520-600 ms',
    700: 'Holds',
    940: 'Handover: the clock band, then the clock entry',
}


def board(path):
    sc = 0.5
    w = int(W * sc)
    f = ImageFont.truetype('fonts/MonoR.otf', 13)
    fb = ImageFont.truetype('fonts/MonoB.otf', 18)
    cols = 6
    rows = (len(TIMES) + cols - 1) // cols
    cell_h = w + 84
    sh = Image.new('RGB', (cols * (w + 12) + 12, rows * cell_h + 50), (20, 20, 22))
    d = ImageDraw.Draw(sh)
    d.text((12, 14), 'F  STRETCHED TITLE, AFTER THE REFERENCE', font=fb, fill=(210, 211, 214))
    for i, t in enumerate(TIMES):
        im = frame(t) if t < 900 else handover(t - 900)
        x, y = 12 + (i % cols) * (w + 12), 48 + (i // cols) * cell_h
        sh.paste(im.resize((w, w), Image.LANCZOS), (x, y))
        d.text((x, y + w + 6), '%d MS' % t, font=f, fill=(210, 211, 214))
        words, line, yy = NOTES[t].split(), '', y + w + 24
        for wd in words:
            if len(line + ' ' + wd) > 31:
                d.text((x, yy), line, font=f, fill=(136, 142, 152)); yy += 16; line = wd
            else:
                line = (line + ' ' + wd).strip()
        d.text((x, yy), line, font=f, fill=(136, 142, 152))
    sh.save(path)


if __name__ == '__main__':
    board('out/identity-F.png')
    frame(700).save('out/identity-F-hero.png')
    fr = [finish(Canvas())] * 8
    for t in range(0, 900, DT):
        fr.append(frame(t))
    for t in range(0, 120, DT):
        fr.append(handover(t))
    fr += [fr[-1]] * 25
    save_gif(fr, 'out/identity-F.gif', DT)
    save_gif(fr, 'out/identity-F-slow3x.gif', DT * 3)
