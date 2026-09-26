"""Identity sequence: five directions, as storyboards of key frames.

Colour: the identity may spend the two unused tokens, LIME and PURPLE, which nothing else on the
device uses, so they can mean "OCTOWHERE" and nothing else. RED stays a fault colour.
Microtext is real data only: version, self-test result, zone data releases, UTC time, date,
zone, battery, and the last fix if there is one. The values here are the fixture's.
"""
import random
import lib
lib.RECORD = False
from lib import *
from PIL import ImageChops, ImageOps

W = 466
BAND = (198, 318)
MICRO = ['OCTOWHERE 0.1.0', 'SELF TEST 6/6 OK', 'TZDATA 2026D', 'TZBB 2026D', 'UTC 12:07:42',
         '24 SEP 2026', 'EUROPE/DUBLIN IST', 'LAST FIX 53.35N 006.26W', 'BAT 87% USB']


# ---- helpers ------------------------------------------------------------------------------------
def circle(r=232.0):
    m = Image.new('L', (N * SS, N * SS), 0)
    ImageDraw.Draw(m).ellipse([(C - r) * SS, (C - r) * SS, (C + r) * SS - 1, (C + r) * SS - 1], fill=255)
    return m


def fill(cv, col, mask=None):
    cv.img.paste(Image.new('RGB', cv.img.size, rgb(col)), (0, 0), mask if mask is not None else circle())


def text_mask(s, face, px, sx=1.0, sy=1.0, tracking=0.0):
    """Upright text raster at 4x, stretched by sx, sy (a stretched raster, not a new glyph)."""
    f = font(face, px)
    adv_ = [f.getlength(ch) + tracking * px * SS for ch in s]
    Wd = int(sum(adv_) + 40 * SS)
    Hd = int(px * SS * 2)
    m = Image.new('L', (Wd, Hd), 0)
    d = ImageDraw.Draw(m)
    x = 20 * SS
    for ch, a in zip(s, adv_):
        d.text((x, px * SS * 1.4), ch, font=f, fill=255, anchor='ls')
        x += a
    m = m.crop(m.getbbox())
    if sx != 1 or sy != 1:
        m = m.resize((max(1, int(m.width * sx)), max(1, int(m.height * sy))), Image.BICUBIC)
    return m


def place(cv, m, col, cx, cy, clip=None):
    full = Image.new('L', cv.img.size, 0)
    full.paste(m, (int(cx * SS - m.width / 2), int(cy * SS - m.height / 2)))
    if clip is not None:
        full = ImageChops.multiply(full, clip)
    cv.img.paste(Image.new('RGB', cv.img.size, rgb(col)), (0, 0), full)
    return full


def rect_mask(x0, y0, x1, y1):
    m = Image.new('L', (N * SS, N * SS), 0)
    ImageDraw.Draw(m).rectangle([x0 * SS, y0 * SS, x1 * SS - 1, y1 * SS - 1], fill=255)
    return m


def noise(cv, density, cell, seed, col, region=None, skip=None):
    """High-contrast block noise: cells of `cell` px, lit with probability `density`.
    Built from rectangles by a seeded generator, not a texture."""
    rnd = random.Random(seed)
    d = ImageDraw.Draw(cv.img)
    for y in range(0, W, cell):
        for x in range(0, W, cell):
            if rnd.random() < density:
                if region and not (region[0] <= x < region[2] and region[1] <= y < region[3]):
                    continue
                d.rectangle([x * SS, y * SS, (x + cell) * SS - 1, (y + cell) * SS - 1], fill=rgb(col))


def slices(im, seed, n=6, maxshift=40):
    """Row-band displacement, as the transfer's row offset would produce."""
    rnd = random.Random(seed)
    out = im.copy()
    for _ in range(n):
        y = rnd.randrange(40, W - 60)
        h = rnd.randrange(4, 26)
        dx = rnd.randrange(-maxshift, maxshift)
        band = im.crop((0, y, W, y + h))
        out.paste((0, 0, 0), (0, y, W, y + h))
        out.paste(band, (dx, y))
    return out


def micro_rails(cv, col='GRAY', k=1.0, seed=0):
    """Microtext along the top and bottom chords, and two registration marks."""
    items = MICRO[:max(1, int(len(MICRO) * k))]
    ys = [70, 88, 378, 396]
    for i, s in enumerate(items[:4]):
        cv.text(s, 'mono', 14, col, cx=233, y=ys[i])
    for x, y in ((64, 233), (402, 233)):
        cv.rect(x - 8, y, x + 9, y + 1, col)
        cv.rect(x, y - 8, x + 1, y + 9, col)


def finish(cv, glitch=None, invert=False):
    im = cv.render(offpanel=False)
    if glitch is not None:
        im = slices(im, glitch)
    if invert:
        im = ImageOps.invert(im)
    m = Image.new('L', (W * 4, W * 4), 0)
    ImageDraw.Draw(m).ellipse([0, 0, W * 4 - 1, W * 4 - 1], fill=255)
    bg = Image.new('RGB', (W, W), (28, 28, 28))
    bg.paste(im, (0, 0), m.resize((W, W), Image.BOX))
    return bg


def band_word(cv, band_col='WHITE', word_col='BLACK', px=40):
    cv.img.paste(Image.new('RGB', cv.img.size, rgb(band_col)), (0, 0),
                 ImageChops.multiply(circle(), rect_mask(0, BAND[0], W, BAND[1])))
    place(cv, text_mask('OCTOWHERE', 'shapiro', px), word_col, 233, 257.5)


# ---- A. STRETCH: the title starts stretched vertically and settles ------------------------------------
def stretch(sy, bg='BLACK', col='WHITE', band=None, micro=0.0, glitch=None):
    cv = Canvas()
    if bg != 'BLACK':
        fill(cv, bg)
    if band:
        cv.img.paste(Image.new('RGB', cv.img.size, rgb(band)), (0, 0),
                     ImageChops.multiply(circle(), rect_mask(0, BAND[0], W, BAND[1])))
    m = text_mask('OCTOWHERE', 'shapiro', 40, sx=1.0, sy=sy)
    place(cv, m, col, 233, 257.5 if sy < 3 else 233, clip=circle())
    if micro:
        micro_rails(cv, 'GRAY' if bg == 'BLACK' else 'BLACK', micro)
    return finish(cv, glitch)


SEQ_A = [
    (lambda: stretch(12, micro=0.3), 0, 'Title arrives stretched 12x, a wall of vertical bars cropped by the circle'),
    (lambda: stretch(6, micro=0.6, glitch=3), 100, 'Halves each step; row slices jump'),
    (lambda: stretch(3, bg='LIME', col='BLACK', micro=1.0), 200, 'Field cuts to LIME, type knocks out black'),
    (lambda: stretch(1.6, bg='LIME', col='BLACK', micro=1.0, glitch=8), 300, 'Nearly settled, one more glitch frame'),
    (lambda: stretch(1.0, band='LIME', col='BLACK', micro=1.0), 400, 'Settles into a LIME band, microtext rails hold'),
    (lambda: stretch(1.0, band='WHITE', col='BLACK'), 700, 'Band turns WHITE: it is the clock band. Handover'),
]


# ---- B. COUNTDOWN ------------------------------------------------------------------------------------
def count(n, bg, col, micro_col, glitch=None):
    cv = Canvas()
    if bg != 'BLACK':
        fill(cv, bg)
    m = text_mask('%02d' % n, 'bold', 300)
    place(cv, m, col, 233, 240, clip=circle())
    cv.text('T-%02d' % n, 'bold', 16, micro_col, x=96, y=96)
    cv.text('OCTOWHERE 0.1.0', 'mono', 14, micro_col, right=370, y=96)
    for k in range(24):                                  # a tick rail counting down
        x = 113 + k * 10
        on = k < n * 8
        cv.rect(x, 372, x + 4, 372 + (14 if k % 8 == 0 else 8), micro_col if on else fade(micro_col if micro_col != 'BLACK' else 'GRAY', 0.3))
    return finish(cv, glitch)


def count_flash():
    cv = Canvas()
    noise(cv, 0.5, 12, 7, 'WHITE')
    return finish(cv)


def count_word():
    cv = Canvas()
    m = text_mask('OCTOWHERE', 'shapiro', 60)
    place(cv, m, 'WHITE', 233, 233, clip=circle())
    cv.rect(40, 180, 426, 184, 'LIME')
    cv.rect(40, 283, 426, 287, 'LIME')
    micro_rails(cv, 'GRAY', 1.0)
    return finish(cv)


SEQ_B = [
    (lambda: count(3, 'LIME', 'BLACK', 'BLACK'), 0, '03 on a LIME field, digits 300 px cropped by the circle'),
    (lambda: count(2, 'BLACK', 'WHITE', 'GRAY'), 250, '02, field cut to black'),
    (lambda: count(1, 'WHITE', 'BLACK', 'BLACK', glitch=5), 500, '01 on WHITE, slices tearing'),
    (count_flash, 740, 'Two frames of block noise at full brightness'),
    (count_word, 780, 'OCTOWHERE slams in at 60 px between LIME rules'),
    (lambda: (lambda cv: (band_word(cv), finish(cv))[1])(Canvas()), 1100, 'Collapses to the band. Handover'),
]


# ---- C. NOISE / SIGNAL ----------------------------------------------------------------------------
def signal(density, resolve, glitch=None, clean=False, lime=False):
    cv = Canvas()
    word = text_mask('OCTOWHERE', 'shapiro', 48)
    # the word, quantised to 8 px blocks, emerging from the noise
    cell = 8
    wm = Image.new('L', cv.img.size, 0)
    wm.paste(word, (int(233 * SS - word.width / 2), int(233 * SS - word.height / 2)))
    small = wm.resize((W // cell, W // cell), Image.BOX)
    rnd = random.Random(11)
    d = ImageDraw.Draw(cv.img)
    if not clean:
        noise(cv, density, cell, 3, 'WHITE')
        for by in range(small.height):
            for bx in range(small.width):
                v = small.getpixel((bx, by))
                if v > 60 and rnd.random() < resolve:
                    d.rectangle([bx * cell * SS, by * cell * SS, (bx + 1) * cell * SS - 1, (by + 1) * cell * SS - 1],
                                fill=rgb('LIME' if lime else 'WHITE'))
                elif v <= 60 and resolve > 0.6:
                    # the word's surroundings clear first
                    if abs(by * cell - 233) < 40:
                        d.rectangle([bx * cell * SS, by * cell * SS, (bx + 1) * cell * SS - 1, (by + 1) * cell * SS - 1], fill=(0, 0, 0))
    else:
        place(cv, word, 'WHITE', 233, 233, clip=circle())
        # residue: a barcode strip of noise under the word
        noise(cv, 0.5, 4, 9, 'GRAY', region=(120, 270, 346, 282))
        cv.text('SELF TEST 6/6 OK', 'mono', 14, 'GRAY', cx=233, y=300)
    return finish(cv, glitch)


SEQ_C = [
    (lambda: signal(0.55, 0.0), 0, 'Full-screen block noise, 8 px cells, white on black'),
    (lambda: signal(0.35, 0.5), 120, 'Noise thins; the word appears as blocks'),
    (lambda: signal(0.18, 0.9, glitch=4, lime=True), 240, 'Blocks lock in LIME; slices tear'),
    (lambda: signal(0.06, 1.0, lime=True), 340, 'Almost clear'),
    (lambda: signal(0, 1.0, clean=True), 440, 'Sharp wordmark; noise survives as a barcode strip'),
    (lambda: (lambda cv: (band_word(cv), finish(cv))[1])(Canvas()), 740, 'Band. Handover'),
]


# ---- D. OCTO BLOCKS: eight blocks, the only shapes -------------------------------------------------------
def blocks(heights, cols, word=False, band=False, micro=0.0):
    cv = Canvas()
    bw = 44
    gap = 8
    x0 = 233 - (8 * bw + 7 * gap) / 2
    for i in range(8):
        h = heights[i]
        x = round(x0 + i * (bw + gap))
        top = round(233 - h / 2)
        m = ImageChops.multiply(circle(), rect_mask(x, top, x + bw, top + h))
        cv.img.paste(Image.new('RGB', cv.img.size, rgb(cols[i % len(cols)])), (0, 0), m)
    if band:
        cv.img.paste(Image.new('RGB', cv.img.size, rgb('LIME')), (0, 0),
                     ImageChops.multiply(circle(), rect_mask(0, BAND[0], W, BAND[1])))
    if word:
        place(cv, text_mask('OCTOWHERE', 'shapiro', 40), 'BLACK', 233, 257.5)
    if micro:
        micro_rails(cv, 'GRAY', micro)
    return finish(cv)


SEQ_D = [
    (lambda: blocks([44] * 8, ['LIME']), 0, 'Eight 44 px squares cut in, one per frame'),
    (lambda: blocks([44, 120, 300, 200, 420, 90, 260, 160], ['LIME', 'WHITE']), 160, 'They stretch to bars of different heights, stepped'),
    (lambda: blocks([420, 60, 420, 60, 420, 60, 420, 60], ['PURPLE', 'LIME']), 280, 'Alternate, full height and stub; PURPLE and LIME'),
    (lambda: blocks([120] * 8, ['LIME'], micro=0.5), 400, 'All eight level to the band height'),
    (lambda: blocks([120] * 8, ['LIME'], band=True, word=True, micro=1.0), 480, 'Gaps close into one LIME band; the word knocks out'),
    (lambda: (lambda cv: (band_word(cv), finish(cv))[1])(Canvas()), 800, 'Band turns WHITE. Handover'),
]


# ---- E. HUGE LETTERS: one letter per cut, full bleed -------------------------------------------------------
def huge(ch, bg, col, dx=0, micro_col=None, idx=None):
    cv = Canvas()
    if bg != 'BLACK':
        fill(cv, bg)
    m = text_mask(ch, 'shapiro', 420)
    place(cv, m, col, 233 + dx, 240, clip=circle())
    if micro_col:
        cv.text('%02d/09' % idx, 'bold', 16, micro_col, x=96, y=100)
        cv.text('OCTOWHERE', 'mono', 14, micro_col, right=370, y=100)
    return finish(cv)


def huge_word():
    cv = Canvas()
    m = text_mask('OCTOWHERE', 'shapiro', 60, sx=1.0)
    place(cv, m, 'BLACK', 233, 233)
    return finish(cv)


def huge_word_lime():
    cv = Canvas()
    fill(cv, 'LIME')
    place(cv, text_mask('OCTOWHERE', 'shapiro', 60), 'BLACK', 233, 233)
    micro_rails(cv, 'BLACK', 1.0)
    return finish(cv)


SEQ_E = [
    (lambda: huge('O', 'LIME', 'BLACK', micro_col='BLACK', idx=1), 0, 'O, 420 px, on LIME. One letter per 60 ms'),
    (lambda: huge('C', 'BLACK', 'WHITE', micro_col='GRAY', idx=2), 60, 'C, white on black'),
    (lambda: huge('T', 'PURPLE', 'BLACK', micro_col='BLACK', idx=3), 120, 'T, on PURPLE'),
    (lambda: huge('W', 'WHITE', 'BLACK', dx=-30, micro_col='BLACK', idx=5), 240, 'W, cropped hard; the sequence speeds up'),
    (huge_word_lime, 540, 'The whole word at 60 px on LIME, microtext rails'),
    (lambda: (lambda cv: (band_word(cv), finish(cv))[1])(Canvas()), 800, 'Field drops to black around the band. Handover'),
]

DIRS = [('A', 'STRETCH', SEQ_A), ('B', 'COUNTDOWN', SEQ_B), ('C', 'NOISE / SIGNAL', SEQ_C), ('D', 'OCTO BLOCKS', SEQ_D),
        ('E', 'HUGE LETTERS', SEQ_E)]


def board(key, name, frames, path):
    sc = 0.55
    w = int(W * sc)
    f = ImageFont.truetype('fonts/MonoR.otf', 13)
    fb = ImageFont.truetype('fonts/MonoB.otf', 18)
    cap_h = 64
    sh = Image.new('RGB', (len(frames) * (w + 12) + 12, w + cap_h + 56), (20, 20, 22))
    d = ImageDraw.Draw(sh)
    d.text((12, 14), '%s  %s' % (key, name), font=fb, fill=(210, 211, 214))
    ims = []
    for i, (fn, t, note) in enumerate(frames):
        im = fn()
        ims.append(im)
        x = 12 + i * (w + 12)
        sh.paste(im.resize((w, w), Image.LANCZOS), (x, 48))
        d.text((x, 48 + w + 8), '%d MS' % t, font=f, fill=(210, 211, 214))
        # wrap the note
        words, line, y = note.split(), '', 48 + w + 26
        for wd in words:
            if len(line + ' ' + wd) > 34:
                d.text((x, y), line, font=f, fill=(136, 142, 152)); y += 16; line = wd
            else:
                line = (line + ' ' + wd).strip()
        d.text((x, y), line, font=f, fill=(136, 142, 152))
    sh.save(path)
    return ims


if __name__ == '__main__':
    heroes = []
    hero_idx = {'A': 0, 'B': 0, 'C': 2, 'D': 2, 'E': 0}
    for key, name, frames in DIRS:
        ims = board(key, name, frames, 'out/identity-%s.png' % key)
        heroes.append((ims[hero_idx[key]], '%s  %s' % (key, name)))
    f = ImageFont.truetype('fonts/MonoR.otf', 16)
    sh = Image.new('RGB', (5 * 490 + 24, 466 + 68), (20, 20, 22))
    d = ImageDraw.Draw(sh)
    for i, (im, label) in enumerate(heroes):
        sh.paste(im, (24 + i * 490, 24))
        d.text((24 + i * 490, 24 + 474), label, font=f, fill=(210, 211, 214))
    sh.save('out/identity-heroes.png')
