"""Always-on face, timeout flow and pixel shift.

AOD face: the clock's layout drawn hollow. Hours and minutes in Fraktion Mono Regular 136 px,
the band as two 1 px rules, the date in 16 px; no ring, seconds, icon, plate or wordmark.
It redraws once a minute, and every redraw moves it one step round the shift pattern.

Pixel shift: everything inside the ring moves by (dx, dy); the ring stays put; bands are
clipped to the fixed circle after the move. Positions: the centre, then eight points round a
3 px circle, one step per move.

Brightness can't be shown in a render. Where a frame is dimmed, the render multiplies its
colours instead, and says so.
"""
import lib
lib.RECORD = False
from lib import *
import math
from PIL import ImageChops
from face import X0, BIG, HOURS_BASE, MIN_BASE

W = 466
BAND = (198, 318)
SHIFTS = [(0, 0), (3, 0), (2, 2), (0, 3), (-2, 2), (-3, 0), (-2, -2), (0, -3), (2, -2)]


def circle_mask(r=232.0, dx=0, dy=0):
    m = Image.new('L', (N * SS, N * SS), 0)
    ImageDraw.Draw(m).ellipse([(C - r + dx) * SS, (C - r + dy) * SS, (C + r + dx) * SS - 1, (C + r + dy) * SS - 1], fill=255)
    return m


def rule(cv, y, col, dx, dy):
    """1 px full-width rule at row y + dy, clipped to the fixed circle."""
    m = circle_mask()
    r = Image.new('L', (N * SS, N * SS), 0)
    ImageDraw.Draw(r).rectangle([0, (y + dy) * SS, N * SS - 1, (y + dy + 1) * SS - 1], fill=255)
    cv.img.paste(Image.new('RGB', cv.img.size, rgb(col)), (0, 0), ImageChops.multiply(m, r))


def mono_digits(cv, s, px, col, x0, base):
    a = adv('mono', px)
    f = font('mono', px)
    for i, ch in enumerate(s):
        cv.d.text(((x0 + i * a) * SS, base * SS), ch, font=f, fill=rgb(col), anchor='ls')


def aod(state='local', shift=(0, 0), h=13, m=7, date='THU 24 SEP'):
    dx, dy = shift
    cv = Canvas()
    if state == 'nodata':
        rule(cv, BAND[0], 'RED', dx, dy)
        rule(cv, BAND[1] - 1, 'RED', dx, dy)
        cv.text('NO DATA', 'shapiro', 28, 'RED', x=71 + dx, cy=257.5 + dy)
        return cv
    known = state == 'local'
    mono_digits(cv, '%02d' % h if known else '--', BIG, 'WHITE', X0 + dx, HOURS_BASE + dy)
    rule(cv, BAND[0], 'WHITE', dx, dy)
    rule(cv, BAND[1] - 1, 'WHITE', dx, dy)
    mono_digits(cv, '%02d' % m if known else '--', BIG, 'WHITE', X0 + dx, MIN_BASE + dy)
    if known:
        cv.text(date, 'mono', 16, 'GRAY', cx=233 + dx, y=334 + dy)
    elif state == 'stopped':
        cv.text('STOPPED', 'mono', 16, 'ORANGE', x=262 + dx, bottom=MIN_BASE + dy)
    elif state == 'nozone':
        cv.text('NO ZONE', 'mono', 16, 'GRAY', x=262 + dx, bottom=MIN_BASE + dy)
    return cv


def lit_fraction(im):
    a = np.array(im.convert('RGB')).astype(int)
    yy, xx = np.mgrid[0:W, 0:W]
    inside = np.hypot(xx + .5 - 233, yy + .5 - 233) < 233
    lum = a.max(axis=2)
    return float(((lum > 16) & inside).sum()) / inside.sum(), float((lum * inside).sum()) / (255 * inside.sum())


def dim(im, k):
    """Simulated brightness for renders only."""
    return Image.eval(im, lambda v: int(v * k))


# ---- shifting a full face: draw bands wider, move, then clip to the fixed circle; ring stays -----
def shifted_page(draw_fn, shift, ring='GRAY'):
    dx, dy = shift
    old = lib.band.__defaults__
    lib.band.__defaults__ = (260.0, None)          # draw bands past the circle, clip after the move
    try:
        cv = draw_fn()
    finally:
        lib.band.__defaults__ = old
    im = cv.img
    moved = Image.new('RGB', im.size, (0, 0, 0))
    moved.paste(im, (dx * SS, dy * SS))
    out = Canvas()
    out.img.paste(moved, (0, 0), circle_mask(229.5))        # content never reaches the ring
    out.ring(230, 232, ring)
    return out


if __name__ == '__main__':
    shots = []
    for st, label in [('local', 'ALWAYS-ON'), ('stopped', 'ALWAYS-ON, CLOCK STOPPED'), ('nozone', 'ALWAYS-ON, NO ZONE'),
                      ('nodata', 'ALWAYS-ON, NO DATA')]:
        im = aod(st).render()
        fr, lum = lit_fraction(im)
        im.save('out/aod-%s.png' % st)
        shots.append((im, '%s  %.1f%% LIT' % (label, fr * 100)))
    # shift pattern on the AOD
    for i in (1, 5):
        im = aod('local', SHIFTS[i]).render()
        shots.append((im, 'ALWAYS-ON SHIFTED %+d,%+d' % SHIFTS[i]))
    # shifted full faces
    import anim
    from anim import page, entry
    def clock_no_ring():
        e = dict(entry('B', 1000)); e['ring'] = 0
        cvx = Canvas()
        cvx.img = page(e)
        return cvx
    # anim.page returns an image; wrap to a canvas-like object at 4x
    def clock_canvas():
        e = dict(entry('B', 1000)); e['ring'] = 0
        im = page(e)                       # 466 px render; upscale for the shift helper
        cvx = Canvas(); cvx.img = im.resize((N * SS, N * SS), Image.NEAREST)
        return cvx
    for sh_ in [(3, 0), (-2, -2)]:
        cvs = shifted_page(clock_canvas, sh_)
        shots.append((cvs.render(), 'CLOCK SHIFTED %+d,%+d' % sh_))
    import compass_trans as ct
    def compass_canvas():
        im = ct.draw(dict(ct.settled('heading'), ring='BLACK'))
        cvx = Canvas(); cvx.img = im.resize((N * SS, N * SS), Image.NEAREST)
        return cvx
    cvs = shifted_page(compass_canvas, (3, 0))
    shots.append((cvs.render(), 'COMPASS SHIFTED +3,+0'))
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
    sh.save('out/aod-overview.png')
    # clock full-face lit fraction for comparison
    e = dict(entry('B', 1000))
    print('clock lit', lit_fraction(page(e)))
