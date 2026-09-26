"""Compass state transitions, round 3.

Built layout: 66 px outlined icon, no hint and no divider; the tilt line ends the stack.
Every change on the settled page is described as parameters over time t (ms), so the same
drawing makes the frame strips and the GIF. Frames every 20 ms (assumed). Functional motion
keeps ms timings; only the start-up's non-functional animations are timed at 30 fps.

Primitives: slab wipe (new colour from the top, in 4 steps over 80 ms), icon row rebuild
(tile recolours in one frame, rows every 30 ms), cell reveal (and reverse) for caption and
state line. Digits are redrawn on the new colour but never change value mid-transition.
"""
import lib
lib.RECORD = False
from lib import *
from compass import LAYOUTS, ICONS, readout
from industrial import cells_text, pen_for
from anim import save_gif, ramp, DT

L = LAYOUTS['icon66']
HEAD = 47
ROW_MS = 30
WIPE_STEPS, WIPE_MS = 4, 80
W = 466

STATE_LINE = {
    'interference': ('INTERFERENCE', 'bold', 24, 'ORANGE'),
    'calibrating': ('TURN ALL WAYS', 'mono', 19, 'GRAY'),
    'topedge': ('TOP EDGE UP', 'mono', 20, 'GRAY'),
}
CAPTION = {'calibrating': ('CALIBRATION', 'ORANGE'), 'nodata': ('COMPASS', 'GRAY')}
SLAB = {'heading': 'WHITE', 'topedge': 'WHITE', 'nodata': 'RED', 'interference': 'ORANGE', 'calibrating': 'ORANGE'}
READ = {'heading': ('047', '°'), 'interference': ('047', '°'), 'calibrating': ('054', '%'),
        'topedge': '---', 'nodata': 'NODATA'}


def caption_of(s):
    return CAPTION.get(s, ('MAGNETIC', 'GRAY'))


def draw(p):
    """p keys: ring, dial (None or sweep 0..1), icon (colour, glyph, rows), caption (text, colour, k),
    line (None or (text, face, px, colour, k)), slab (old, new, k), read, tilt."""
    cv = Canvas()
    cv.ring(230, 232, p['ring'])
    if p['dial'] is not None:
        sweep = p['dial'] * 360
        for t in range(36):
            if t * 10 < sweep or (sweep > 0 and t == 0):
                a = t * 10 - HEAD
                if t % 3 == 0:
                    cv.radial_bar(a, 202, 226, 4, 'WHITE')
                else:
                    cv.radial_bar(a, 216, 226, 2, 'GRAY')
        for t, Lt in enumerate('NESW'):
            if t * 90 < sweep or (sweep > 0 and t == 0):
                rtext(cv, Lt, 'shapiro', 40, 'ORANGE' if Lt == 'N' else 'WHITE', t * 90 - HEAD, 172)
    col, glyph, rows = p['icon']
    size = 5 * L['module'] + 2 * L['pad']
    icon_sized(cv, round(233 - size / 2), L['icon_top'], col, glyph, module=L['module'], pad=L['pad'], rows_shown=rows)
    ctext, ccol, ck = p['caption']
    cells_text(cv, ctext, 'bold', 16, ccol, pen_for(ctext, 'bold', 16, cx=233), L['caption'] + cap_height('bold', 16), ck)
    if p['line']:
        text, face, px, lcol, lk = p['line']
        base = L['state_mid'] + cap_height(face, px) / 2
        cells_text(cv, text, face, px, lcol, pen_for(text, face, px, cx=233), base, lk)
    old, new, k = p['slab']
    s0 = L['slab']
    cut = s0 + round(91 * k)
    cv.rect(135, s0, 331, s0 + 91, old)
    if cut > s0:
        cv.rect(135, s0, 331, cut, new)
    r = p['read']
    if r == 'NODATA':
        cv.text('NO DATA', 'shapiro', 28, 'BLACK', cx=233, cy=s0 + 45.5)
    elif r == '---':
        cv.text('---', 'bold', 86, 'BLACK', cx=233, cy=s0 + 45.5)
    else:
        readout(cv, r[0], r[1], s0)
    if p['tilt']:
        cv.text('P +05  R -12', 'mono', 23, 'GRAY', cx=233, y=L['tilt'])
    return cv.render()


def settled(s):
    tile, glyph = ICONS[s]
    line = STATE_LINE.get(s)
    return dict(ring='RED' if s == 'nodata' else 'GRAY',
                dial=1.0 if s in ('heading', 'interference') else None,
                icon=(tile, glyph, 5), caption=(*caption_of(s), 1.0),
                line=(*line, 1.0) if line else None,
                slab=(SLAB[s], SLAB[s], 0.0), read=READ[s], tilt=s != 'nodata')


def wipe_k(t):
    """Stepped wipe: 4 steps, one per 20 ms frame."""
    if t < 0:
        return 0.0
    return min(1.0, (t // (WIPE_MS // WIPE_STEPS) + 1) / WIPE_STEPS)


def rows_at(t):
    return max(0, min(5, t // ROW_MS + 1)) if t >= 0 else 0


def transition(a, b, t):
    """Parameters t ms after the change from state a to state b is shown."""
    if b == 'nodata':
        return settled('nodata')                 # a fault shows at once
    p = settled(b)
    tile, glyph = ICONS[b]
    # icon: rebuilt unless the glyph and colour are unchanged
    if ICONS[a] != ICONS[b]:
        p['icon'] = (tile, glyph, rows_at(t))
    # slab: wipes when its colour changes, except that a fault's RED leaves at once
    if SLAB[a] != SLAB[b] and a != 'nodata':
        p['slab'] = (SLAB[a], SLAB[b], wipe_k(t))
    # caption: retyped when its text changes (colour changes with it)
    if caption_of(a) != caption_of(b):
        p['caption'] = (*caption_of(b), ramp(t, 0, 120))
    # state line: the old one untypes over 60 ms, then the new one types over 120 ms
    la, lb = STATE_LINE.get(a), STATE_LINE.get(b)
    if la != lb:
        if la and t < 60:
            p['line'] = (*la, 1 - ramp(t, 0, 60))
        elif lb:
            p['line'] = (*lb, ramp(t, 60 if la else 0, 120))
        else:
            p['line'] = None
    # dial: sweeps in when a heading arrives from a state without one; goes at once otherwise
    had = a in ('heading', 'interference')
    has = b in ('heading', 'interference')
    if has and not had:
        p['dial'] = ramp(t, 0, 170)
    return p


PAIRS = [
    ('heading', 'interference', 'HEADING TO INTERFERENCE (AFTER 200 MS DISTURBED)'),
    ('interference', 'heading', 'INTERFERENCE TO HEADING (AFTER 1 S CLEAN)'),
    ('calibrating', 'topedge', 'CALIBRATION COMPLETES WHILE TILTED'),
    ('calibrating', 'heading', 'CALIBRATION COMPLETES: HEADING ARRIVES'),
    ('nodata', 'calibrating', 'LEAVING NO DATA'),
    ('heading', 'topedge', 'HEADING TO TOP EDGE UP: DIAL GOES AT ONCE'),
]


def strip(path, ts=(-20, 0, 20, 40, 60, 80, 100, 120, 140, 160, 180)):
    sc = 0.4
    w = int(W * sc)
    f = ImageFont.truetype('fonts/MonoR.otf', 14)
    lab = 30
    sh = Image.new('RGB', (len(ts) * (w + 6) + 10, len(PAIRS) * (w + lab + 16) + 10), (28, 28, 28))
    d = ImageDraw.Draw(sh)
    for r, (a, b, label) in enumerate(PAIRS):
        y = 10 + r * (w + lab + 16)
        d.text((10, y), label, font=f, fill=(210, 211, 214))
        for i, t in enumerate(ts):
            im = draw(settled(a) if t < 0 else transition(a, b, t)).resize((w, w), Image.LANCZOS)
            sh.paste(im, (10 + i * (w + 6), y + lab))
            d.text((10 + i * (w + 6), y + lab + w - 2), 'BEFORE' if t < 0 else '%d MS' % t, font=f,
                   fill=(136, 142, 152))
    sh.save(path)


def gif(path, slow=1):
    seq = ['heading', 'interference', 'heading', 'topedge', 'heading']
    frames = []
    for i in range(len(seq)):
        a = seq[i - 1] if i else None
        b = seq[i]
        if a is None:
            frames += [draw(settled(b))] * 45
            continue
        for t in range(0, 200, DT):
            frames.append(draw(transition(a, b, t)))
        frames += [frames[-1]] * 60
    # calibration completing while tilted, then leaving NO DATA
    frames += [draw(settled('calibrating'))] * 45
    for t in range(0, 200, DT):
        frames.append(draw(transition('calibrating', 'topedge', t)))
    frames += [frames[-1]] * 60
    frames += [draw(settled('nodata'))] * 45
    for t in range(0, 200, DT):
        frames.append(draw(transition('nodata', 'calibrating', t)))
    frames += [frames[-1]] * 60
    save_gif(frames, path, DT * slow)


if __name__ == '__main__':
    strip('out/compass-trans-frames.png')
    gif('out/compass-trans.gif')
    gif('out/compass-trans-slow4x.gif', 4)
