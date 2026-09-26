"""Start-up: power-on self-test, then the identity sequence, then the clock.

Self-test: six subsystems in a 3 x 2 registration grid. Each cell's frame is there from the
first frame; its glyph builds by rows when that driver answers, and its status reads OK.
A driver that does not answer by its deadline fails: RED frame, the no-data glyph, FAIL.
The fixture's answer times are illustrative.

Identity (only after every cell passes): the clock's band builds in eight segments, left to
right; OCTOWHERE types into it in Shapiro; the version types under it; then the wordmark
untypes and the clock's time types in around the band, and the face's own entry runs.
"""
import lib
lib.RECORD = False
from lib import *
from industrial import cells_text, pen_for
from anim import page as clock_page, entry as clock_entry, save_gif, ramp, DT

W = 466
BAND = (198, 318)

CELLS = [   # name, glyph, answer time (ms), passes
    ('POWER', '01110 11111 10001 11111 11111', 40, True),
    ('CLOCK', '11111 10001 10101 10001 11111', 90, True),
    ('TOUCH', '00100 00100 11011 00100 00100', 140, True),
    ('MOTION', '10000 10000 10000 10000 11111', 230, True),
    ('MAGNET', '11011 11011 11011 11111 01110', 300, True),
    ('GNSS', '00100 01010 10101 01010 00100', 620, True),
]
NODATA = '11100 11000 00100 00011 00111'

# grid: 3 columns x 2 rows, cells 116 x 104, centred on the panel
CW, CH = 116, 104
GX0 = 233 - 3 * CW // 2          # 59
GY0 = 233 - CH                   # 129
ROW_MS = 30


def post(t, cells=CELLS, bright=1.0):
    """The self-test t ms after the first frame."""
    cv = Canvas()
    cv.ring(230, 232, 'GRAY')
    cv.text('SELF TEST', 'shapiro', 16, 'WHITE', cx=233, y=92)
    # rules
    for r in range(3):
        y = GY0 + r * CH
        cv.rect(GX0, y, GX0 + 3 * CW + 1, y + 1, 'GRAY')
    for c in range(4):
        x = GX0 + c * CW
        cv.rect(x, GY0, x + 1, GY0 + 2 * CH + 1, 'GRAY')
    done = 0
    for i, (name, glyph, at, ok) in enumerate(cells):
        c, r = i % 3, i // 3
        left, top = GX0 + c * CW, GY0 + r * CH + 1
        cells_text(cv, '%02d' % (i + 1), 'mono', 14, 'GRAY', left + 9, top + 9 + cap_height('mono', 14), 1.0)
        cv.text(name, 'bold', 14, 'GRAY', x=left + 9, y=top + 62)
        ix, iy = left + CW - 9 - 48, top + 8
        if t < at:
            icon_sized(cv, ix, iy, 'GRAY', glyph, module=8, pad=4, rows_shown=0)
            cells_text(cv, '--', 'mono', 16, 'GRAY', left + 9, top + 90, 1.0)
        elif ok:
            rows = min(5, (t - at) // ROW_MS + 1)
            icon_sized(cv, ix, iy, 'WHITE', glyph, module=8, pad=4, rows_shown=rows)
            cells_text(cv, 'OK', 'mono', 16, 'WHITE', left + 9, top + 90, 1.0)
            done += 1
        else:
            icon_sized(cv, ix, iy, 'RED', NODATA, module=8, pad=4)
            cells_text(cv, 'FAIL', 'mono', 16, 'RED', left + 9, top + 90, 1.0)
            done += 1
    cv.text('%d/%d' % (done, len(cells)), 'mono', 14, 'GRAY', cx=233, y=GY0 + 2 * CH + 16)
    cv.text('0.1.0', 'mono', 14, 'GRAY', cx=233, y=GY0 + 2 * CH + 36)
    return cv.render()


def identity(t):
    """t ms after the self-test's last cell passed and its hold ended."""
    cv = Canvas()
    # band: eight segments, one every 20 ms, left to right, clipped to the circle
    seg = W / 8
    n = min(8, max(0, t // 20 + 1)) if t >= 0 else 0
    for k in range(n):
        band_seg(cv, round(k * seg), round((k + 1) * seg))
    # wordmark types in, 30 ms a character, from 200 ms
    word = 'OCTOWHERE'
    kw = ramp(t, 200, 30 * (len(word) + 1))
    if t >= 200:
        cells_text(cv, word, 'shapiro', 40, 'BLACK', pen_for(word, 'shapiro', 40, cx=233),
                   257.5 + cap_height('shapiro', 40) / 2, kw)
    # version under the band
    if t >= 380:
        cells_text(cv, '0.1.0', 'mono', 14, 'GRAY', pen_for('0.1.0', 'mono', 14, cx=233),
                   BAND[1] + 22 + cap_height('mono', 14), ramp(t, 380, 90))
    return cv.render()


def band_seg(cv, x0, x1):
    m = Image.new('L', (N * SS, N * SS), 0)
    ImageDraw.Draw(m).ellipse([(C - 232) * SS, (C - 232) * SS, (C + 232) * SS - 1, (C + 232) * SS - 1], fill=255)
    r = Image.new('L', (N * SS, N * SS), 0)
    ImageDraw.Draw(r).rectangle([x0 * SS, BAND[0] * SS, x1 * SS - 1, BAND[1] * SS - 1], fill=255)
    from PIL import ImageChops
    cv.img.paste(Image.new('RGB', cv.img.size, rgb('WHITE')), (0, 0), ImageChops.multiply(m, r))


def handover(t):
    """The wordmark untypes (0-100 ms), then the clock's time and date type in and the face's
    own entry runs, with its ring fade dropped (the ring arrives with the entry)."""
    word = 'OCTOWHERE'
    if t < 100:
        cv = Canvas()
        clipped_band(cv, *BAND, 'WHITE')
        cells_text(cv, word, 'shapiro', 40, 'BLACK', pen_for(word, 'shapiro', 40, cx=233),
                   257.5 + cap_height('shapiro', 40) / 2, 1 - ramp(t, 0, 100))
        cells_text(cv, '0.1.0', 'mono', 14, 'GRAY', pen_for('0.1.0', 'mono', 14, cx=233),
                   BAND[1] + 22 + cap_height('mono', 14), 1 - ramp(t, 0, 60))
        return cv.render()
    u = t - 100
    e = clock_entry('B', u)
    e['time'] = ramp(u, 0, 180)
    e['date'] = ramp(u, 120, 160)
    im = clock_page(e)
    return still(im)


def still(im):
    m = Image.new('L', (W * 4, W * 4), 0)
    ImageDraw.Draw(m).ellipse([0, 0, W * 4 - 1, W * 4 - 1], fill=255)
    bg = Image.new('RGB', (W, W), (28, 28, 28))
    bg.paste(im, (0, 0), m.resize((W, W), Image.BOX))
    return bg


def sequence():
    frames = [still(Image.new('RGB', (W, W)))] * 10
    last = max(c[2] for c in CELLS) + 150            # the last glyph built
    for t in range(0, last + 200, DT):                 # plus a 200 ms hold with every cell passed
        frames.append(post(t))
    for t in range(0, 900, DT):                        # identity, including a 380 ms hold
        frames.append(identity(t))
    for t in range(0, 700, DT):
        frames.append(handover(t))
    frames += [frames[-1]] * 40
    return frames


def failed_sequence():
    cells = [c if c[0] != 'MAGNET' else (c[0], c[1], 1000, False) for c in CELLS]
    frames = [still(Image.new('RGB', (W, W)))] * 10
    for t in range(0, 1000 + 2000, DT * 2):
        frames.append(post(t, cells))
    return frames, cells


if __name__ == '__main__':
    shots = []
    for t in (0, 100, 260, 400, 800):
        shots.append((post(t), 'SELF TEST %d MS' % t))
    fcells = [c if c[0] != 'MAGNET' else (c[0], c[1], 1000, False) for c in CELLS]
    shots.append((post(1100, fcells), 'SELF TEST, MAGNETOMETER FAILED'))
    for t in (60, 140, 260, 380, 560):
        shots.append((identity(t), 'IDENTITY %d MS' % t))
    for t in (40, 160, 260, 420):
        shots.append((handover(t), 'HANDOVER %d MS' % t))
    f = ImageFont.truetype('fonts/MonoR.otf', 16)
    cols = 4
    Wc, Hh = 466 + 24, 466 + 44
    rows = (len(shots) + cols - 1) // cols
    sh = Image.new('RGB', (cols * Wc + 24, rows * Hh + 24), (28, 28, 28))
    d = ImageDraw.Draw(sh)
    for i, (im, label) in enumerate(shots):
        x, y = 24 + (i % cols) * Wc, 24 + (i // cols) * Hh
        sh.paste(im, (x, y))
        d.text((x, y + 474), label, font=f, fill=(136, 142, 152))
    sh.save('out/startup-overview.png')
    fr = sequence()
    save_gif(fr, 'out/startup.gif', DT)
    save_gif(fr, 'out/startup-slow2x.gif', DT * 2)
    print(len(fr))
