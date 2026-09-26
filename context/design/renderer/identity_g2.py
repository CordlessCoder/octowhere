"""Identity G2, the chosen direction: STAND BY on a black field with PURPLE halftone scatter,
F's microtext row inside the band, no checker squares.

Two variants for the band's edges:
  RULES  the band's WHITE rules (2 px) at rows 198-199 and 316-317
  BARE   no rules; the band is where the scatter stops
"""
import math
import random
import lib
lib.RECORD = False
from lib import *
from identity import finish, band_word
from identity_g import tm, reveal, putc, g_frow, hatch
from identity_f import barcode, glyph, pix_digits
import identity_g as IG
from anim import save_gif, DT

W = 466
FPS = 30                 # timing is in frames; 24 fps stretches every step by 1.25x
PX, SY = 36, 1.5
WORD_CY = 275            # cap-centred; the word sits under the microtext row
ROW_TOP = 211
RULES = False
LINES = ('VERSION 0.1.0', 'SELF TEST 6/6 OK')   # the version lives in the band's row
TURN = 0.055             # radians per frame, the scatter's slow turn

# the timeline, in frames
F_OFF = (1,)             # the scatter's one dark frame
F_FILL = (2, 8)          # scatter density 50% -> 100%
F_RULES = (4, 6)         # rules draw out over three frames
F_ROW = 6                # F's row: one element a frame from here (6 elements)
F_WORD = 8               # the word: one letter a frame from here, whole at 17
FLICKER = (20, 22)       # frames with the word off
REST = 58                # the handover starts at this frame: ~1.2 s steady after the flicker

# the scatter grid: 8 px pitch, rows placed so marks stop 2 px short of row 198 and start at row 318
GRID_Y0 = 14 - 8 * 2
GRID_X0 = 12


def scatter(cv, t, appear=1.0):
    rnd = random.Random(4)
    for y0 in range(GRID_Y0, 466, 8):
        for x in range(GRID_X0, 460, 8):
            y = y0
            v, kind = rnd.random(), rnd.random()
            if 196 < y + 6 and y < 318:
                continue                                   # the band
            if y >= 318:
                y += 2                                     # 2 px clear of the band on both sides
            dx, dy = x + 3 - 233, y + 3 - 233
            r = math.hypot(dx, dy)
            if r > 228:
                continue
            ang = math.atan2(dy, dx)
            radial = 0.45 + 0.55 * min(1.0, max(0.0, (r - 40) / 180))
            turn = 0.45 + 0.55 * math.cos(ang - 0.8 - t * TURN)
            if v < radial * turn * 1.15 * appear:
                if kind < 0.6:
                    cv.rect(x, y, x + 6, y + 6, 'PURPLE'); cv.rect(x + 2, y + 2, x + 4, y + 4, 'BLACK')
                else:
                    cv.rect(x + 1, y + 1, x + 5, y + 5, 'PURPLE')


LAYOUT = 'hatch'         # chosen 25 Sep. Others: 'centre', or the word left with a weight on the right: 'hatch', 'glyph', 'barcode'
BIG = dict(centre=(44, 1.7), hatch=(40, 1.8), glyph=(40, 1.8), barcode=(40, 1.8))
LEFT = 34                # the word's and the row's left edge in the left layouts
RIGHT = (372, 436)       # the weight's x span
OCT = '10101 00000 10001 00000 10101'   # pattern 40, no longer used (the mark is L1, below)
ROW_ITEMS = [('bar',), ('g', '10001 01010 00100 01010 10001'), ('mark',), ('p', '12 07'),
             ('g', '00100 01010 10101 01010 00100'), ('t',) + LINES]


def row(cv, n):
    items = [i for i in ROW_ITEMS if not (LAYOUT == 'barcode' and i[0] == 'bar')
             and not (LAYOUT == 'hatch' and i[0] == 'g' and i[1].startswith('10001'))]

    def run(cv, x, n):
        for it in items[:n]:
            if it[0] == 'bar':
                x = barcode(cv, '0.1.0', x, ROW_TOP, col='LIME') + 10
            elif it[0] == 'g':
                x = glyph(cv, it[1], x, ROW_TOP, col='LIME') + 11
            elif it[0] == 'mark':                          # L1's 15 x 15 row bitmap, 1 px strokes
                import marks_frame as MF
                for j, r in enumerate(MF.ROW['L1']):
                    for i, b in enumerate(r):
                        if b == '1':
                            cv.rect(x + i, ROW_TOP + j, x + i + 1, ROW_TOP + j + 1, 'LIME')
                x += 15 + 11
            elif it[0] == 'p':
                x = pix_digits(cv, it[1], x, ROW_TOP, col='LIME') + 10
            else:
                cv.text(it[1], 'mono', 14, 'LIME', x=x, y=ROW_TOP - 2)
                cv.text(it[2], 'mono', 14, 'LIME', x=x, y=ROW_TOP + 14)
                x += font('mono', 14).getlength(max(it[1:], key=len)) / SS
        return x
    x0 = LEFT if LAYOUT != 'centre' else round(233 - run(Canvas(), 0, 99) / 2)
    run(cv, x0, n)


def word_geom():
    px, sy = BIG[LAYOUT]
    m = tm('OCTOWHERE', 'shapiro', px, sy=sy)
    w, h = m.width / SS, m.height / SS
    cx = 233 if LAYOUT == 'centre' else LEFT + w / 2
    cy = 311 - h / 2 - 4          # bottom of the ink 4 px above the band's bottom row
    return px, sy, m, cx, cy


def weight(cv, k):
    """The right-hand weight, k = 0..1 built."""
    if k <= 0 or LAYOUT == 'centre':
        return
    x0, x1 = RIGHT
    _, _, m, cx, cy = word_geom()
    wt, wb = cy - m.height / SS / 2, cy + m.height / SS / 2
    if LAYOUT == 'hatch':                         # full band height, drawn down over the build
        y0, y1 = 206, 206 + (311 - 206) * k
        hatch(cv, x0, y0, x1, max(y0 + 1, y1), 'LIME', pitch=10, w=5)
    elif LAYOUT == 'glyph':                       # a solid LIME octagon, the word's height plus a little
        s = wb - wt + 8
        c = s * (1 - 1 / 2 ** 0.5) / 2 * 1.2
        x0_, y0_ = x1 - s, wb - s
        if k < 1:                                 # a block first, as the cell reveal does
            cv.rect(x0_, y0_, x1, wb, 'PURPLE')
        else:
            cv.poly([(x0_ + c, y0_), (x1 - c, y0_), (x1, y0_ + c), (x1, wb - c), (x1 - c, wb),
                     (x0_ + c, wb), (x0_, wb - c), (x0_, y0_ + c)], 'LIME')
    elif LAYOUT == 'barcode':                     # the version barcode, full band height
        bits = [(b >> i) & 1 for b in '0.1.0'.encode() for i in range(4)]
        x = x1 + 6 - sum((3 if b else 1) + 2 for b in bits) + 2   # right-aligned, 6 px past the span
        shown = bits[:max(1, int(len(bits) * k))]
        for bit in shown:
            w = 3 if bit else 1
            cv.rect(x, 206, x + w, 311, 'LIME')
            x += w + 2


def frame(n):
    cv = Canvas()
    if n < F_FILL[0]:
        if n not in F_OFF:
            scatter(cv, n, appear=0.5)
        return finish(cv)
    k = min(1.0, (n - F_FILL[0]) / (F_FILL[1] - F_FILL[0]))
    scatter(cv, n, appear=0.5 + 0.5 * k)
    if RULES and n >= F_RULES[0]:
        k = min(1.0, (n - F_RULES[0] + 1) / (F_RULES[1] - F_RULES[0] + 1))
        cv.rect(233 - 233 * k, 198, 233 + 233 * k, 200, 'WHITE')
        cv.rect(233 - 233 * k, 316, 233 + 233 * k, 318, 'WHITE')
    if n >= F_ROW:
        row(cv, n - F_ROW + 1)
        weight(cv, min(1.0, (n - F_ROW + 1) / 4))
    px, sy, m, cx, cy = word_geom()
    if n >= F_WORD and n not in FLICKER:
        reveal(cv, 'OCTOWHERE', 'shapiro', px, 'LIME', cx, cy, n - F_WORD, sy=sy)
    return finish(cv)


HANDOVER = 'logo'        # 'logo' (the card, chosen 25 Sep) or 'band' (the first version)


def handover(n):
    if HANDOVER == 'logo':
        import logo_card as LC
        im = LC.card(n)
        return im if im is not None else clock_in(n - LC.F_CLOCK)
    return handover_band(n)


def handover_band(n):
    """Frames from REST. 0: the scatter, row and weight cut. 1: the band fills WHITE, the word
    turns BLACK where it is. 2-4: the word untypes in place, three letters a frame. From 5 the
    clock's time and date type in around the band while the face's entry runs (see clock_in)."""
    cv = Canvas()
    px, sy, m, cx, cy = word_geom()
    if n == 0:
        row(cv, 99)
        weight(cv, 1.0)
        putc(cv, m, 'LIME', cx, cy)
    elif n <= 4:
        cv.rect(0, 198, W, 318, 'WHITE')
        shown = 9 if n == 1 else 9 - 3 * (n - 1)
        reveal(cv, 'OCTOWHERE', 'shapiro', px, 'BLACK', cx, cy, shown, sy=sy)
    else:
        return clock_in(n - 5)
    return finish(cv)


def clock_in(m):
    """The clock after the card, m frames of the 30 fps render in: the clock's own motion is
    functional and keeps its ms timings (the built entry; time 0-180 ms, date 120-280 ms)."""
    from anim import page, entry, ramp, FRAME_MS
    from startup import still
    t = m * FRAME_MS
    e = entry('B', t)
    e['time'] = ramp(t, 0, 180)
    e['date'] = ramp(t, 120, 160)
    return still(page(e))


def render(n):
    return frame(n) if n < REST else handover(n - REST)


TIMES = [0, 1, 3, 6, 9, 13, 17, 20, 40, 57, 58, 61, 65, 69, 70, 71, 77]
NOTES = {0: 'The scatter cuts on at half density', 1: 'One frame dark (brightness, free)',
         3: 'The scatter fills in over 6 frames; its dense side turns slowly all through',
         6: "F's row starts: one element a frame. The hatch column draws down over 4 frames",
         9: 'The word types in, one letter a frame', 13: 'Row and hatch whole',
         17: 'Word whole', 20: 'The word flickers: frames 20 and 22 dark',
         40: 'Holds, steady', 57: 'The last frame of the hold',
         58: 'The card cuts on: the page LIME, the hatched tile knocked out alone, 3 frames',
         61: 'The whole mark: corners and edge lines join the tile, 4 frames', 65: 'Inverted: the mark alone, LIME on black, 4 frames',
         69: 'The mark 1.9x for one frame; the corners touch the edge',
         70: 'Impact: inverted again, one frame', 71: "The clock cuts in; its own entry runs in ms, as built",
         77: 'Entry continuing; it ends at 460 ms'}


def board(path, title):
    sc = 0.5
    w = int(W * sc)
    f = ImageFont.truetype('fonts/MonoR.otf', 13)
    fb = ImageFont.truetype('fonts/MonoB.otf', 18)
    cols = 6
    rows = (len(TIMES) + cols - 1) // cols
    cell_h = w + 84
    sh = Image.new('RGB', (cols * (w + 12) + 12, rows * cell_h + 50), (20, 20, 22))
    d = ImageDraw.Draw(sh)
    d.text((12, 14), title, font=fb, fill=(210, 211, 214))
    for i, t in enumerate(TIMES):
        x, y = 12 + (i % cols) * (w + 12), 48 + (i // cols) * cell_h
        sh.paste(render(t).resize((w, w), Image.LANCZOS), (x, y))
        d.text((x, y + w + 6), 'F%d  %d MS' % (t, round(t * 1000 / FPS)), font=f, fill=(210, 211, 214))
        words, line, yy = NOTES[t].split(), '', y + w + 24
        for wd in words:
            if len(line + ' ' + wd) > 31:
                d.text((x, yy), line, font=f, fill=(136, 142, 152)); yy += 16; line = wd
            else:
                line = (line + ' ' + wd).strip()
        d.text((x, yy), line, font=f, fill=(136, 142, 152))
    sh.save(path)


def gif(path):
    """At 30 fps: GIF delays are in 10 ms units, so frames alternate 30, 30, 40 ms."""
    fr = [finish(Canvas())] * 6
    for n in range(REST + 13 + 16):
        fr.append(render(n))
    fr += [fr[-1]] * 25
    pal = [f.convert('P', palette=Image.ADAPTIVE, colors=64) for f in fr]
    dur = [40 if i % 3 == 2 else 30 for i in range(len(pal))]
    pal[0].save(path, save_all=True, append_images=pal[1:], duration=dur, loop=0, disposal=1, optimize=False)


if __name__ == '__main__':
    import sys
    names = dict(centre='CENTRED, LARGER', hatch='LEFT, HATCH COLUMN ON THE RIGHT',
                 glyph='LEFT, OCTAGON CELL ON THE RIGHT', barcode='LEFT, VERSION BARCODE ON THE RIGHT')
    for lay in sys.argv[1:] or names:
        LAYOUT = lay
        board('out/identity-G2-%s.png' % lay, 'G2  %s' % names[lay])
        render(40).save('out/identity-G2-%s-hero.png' % lay)
        gif('out/identity-G2-%s.gif' % lay)
