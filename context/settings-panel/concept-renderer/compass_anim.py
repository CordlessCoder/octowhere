"""Compass entry, exit and heading-arrival animations: the current fades vs the build language.

Layout is the proposed 66 px outlined compass (compass.LAYOUTS['icon66']).
Frames every 20 ms (assumed).
"""
import lib
lib.RECORD = False
from lib import *
from compass import LAYOUTS, ICONS, readout
from industrial import cells_text, pen_for
from anim import composite, drag_profile, save_gif, ramp, clamp, W, DT

L = LAYOUTS['icon66']
HEADING = 47
ROW_MS = 30


def draw(state, e, heading=HEADING):
    """state: heading | calibrating | interference | topedge | nodata.
    e: ring, icon_fade, icon_rows, caption, dial (sweep 0..1, or fade if dial_fade),
       letters_fade, divider, hint. All 0..1 except icon_rows (0..5)."""
    cv = Canvas()
    fault = state == 'nodata'
    if e['ring'] > 0:
        cv.ring(230, 232, fade('RED' if fault else 'GRAY', e['ring']))
    if state in ('heading', 'interference'):
        if e.get('dial_fade'):
            k = e['dial']
            if k > 0:
                for t in range(36):
                    a = t * 10 - heading
                    if t % 3 == 0:
                        cv.radial_bar(a, 202, 226, 4, fade('WHITE', k))
                    else:
                        cv.radial_bar(a, 216, 226, 2, fade('GRAY', k))
            kl = e['letters_fade']
            if kl > 0:
                for t, Lt in enumerate('NESW'):
                    rtext(cv, Lt, 'shapiro', 40, fade('ORANGE' if Lt == 'N' else 'WHITE', kl), t * 90 - heading, 172)
        else:
            sweep = e['dial'] * 360          # bearings already swept, clockwise from N
            for t in range(36):
                if t * 10 < sweep or (sweep > 0 and t == 0):
                    a = t * 10 - heading
                    if t % 3 == 0:
                        cv.radial_bar(a, 202, 226, 4, 'WHITE')
                    else:
                        cv.radial_bar(a, 216, 226, 2, 'GRAY')
            for t, Lt in enumerate('NESW'):
                if t * 90 < sweep or (sweep > 0 and t == 0):
                    rtext(cv, Lt, 'shapiro', 40, 'ORANGE' if Lt == 'N' else 'WHITE', t * 90 - heading, 172)
    tile, rows = ICONS[state]
    size = 5 * L['module'] + 2 * L['pad']
    if e['icon_fade'] > 0:
        icon_sized(cv, round(233 - size / 2), L['icon_top'], fade(tile, e['icon_fade']), rows,
                   module=L['module'], pad=L['pad'], rows_shown=e['icon_rows'])
    cap, capc = {'calibrating': ('CALIBRATION', 'ORANGE'), 'nodata': ('COMPASS', 'GRAY')}.get(state, ('MAGNETIC', 'GRAY'))
    cap_base = L['caption'] + cap_height('bold', 16)
    cells_text(cv, cap, 'bold', 16, capc, pen_for(cap, 'bold', 16, cx=233), cap_base, e['caption'])
    st = {'interference': ('INTERFERENCE', 'bold', 24, 'ORANGE'), 'calibrating': ('TURN ALL WAYS', 'mono', 19, 'GRAY'),
          'topedge': ('TOP EDGE UP', 'mono', 20, 'GRAY')}.get(state)
    if st:
        cv.text(st[0], st[1], st[2], st[3], cx=233, cy=L['state_mid'])       # never animated
    slab = {'heading': 'WHITE', 'topedge': 'WHITE', 'nodata': 'RED'}.get(state, 'ORANGE')
    s0 = L['slab']
    cv.rect(135, s0, 331, s0 + 91, slab)
    if fault:
        cv.text('NO DATA', 'shapiro', 28, 'BLACK', cx=233, cy=s0 + 45.5)
        return cv.render(offpanel=False)
    if state == 'topedge':
        cv.text('---', 'bold', 86, 'BLACK', cx=233, cy=s0 + 45.5)
    elif state == 'calibrating':
        readout(cv, '054', '%', s0)
    else:
        readout(cv, '047', '°', s0)
    cv.text('P +05  R -12', 'mono', 23, 'GRAY', cx=233, y=L['tilt'])
    k = e['divider']
    if k > 0:
        half = 95.5 * k
        cv.rect(round(233.5 - half), L['div'], round(233.5 + half), L['div'] + 1, 'GRAY')
    hint = 'COVER SCREEN TO RECAL'
    cells_text(cv, hint, 'mono', 14, 'GRAY', pen_for(hint, 'mono', 14, cx=233), L['hint'] + cap_height('mono', 14), e['hint'])
    return cv.render(offpanel=False)


FULL = dict(ring=1, icon_fade=1, icon_rows=5, caption=1, dial=1, letters_fade=1, divider=1, hint=1)


# ---- current behaviour, from the brief --------------------------------------------------------
def cur_entry(t):
    return dict(ring=ramp(t, 0, 110), icon_fade=ramp(t, 95, 110), icon_rows=5, caption=1, dial_fade=True,
                dial=ramp(t, 190, 110), letters_fade=ramp(t, 285, 110), divider=1, hint=1)


def cur_exit(p):
    k = 1 - p
    return dict(ring=k, icon_fade=k, icon_rows=5, caption=1, dial_fade=True, dial=k, letters_fade=k, divider=1, hint=1)


# ---- proposed ---------------------------------------------------------------------------------
def new_entry(t):
    return dict(ring=ramp(t, 0, 110),
                icon_fade=1, icon_rows=min(5, int((t - 95) // ROW_MS + 1)) if t >= 95 else 0,
                caption=ramp(t, 120, 120),
                dial=ramp(t, 190, 170),
                letters_fade=1,
                divider=ramp(t, 240, 100),
                hint=ramp(t, 280, 160))


def new_exit(p):
    return dict(ring=1 - ramp(p, 0.6, 0.4),
                icon_fade=1, icon_rows=round(5 * (1 - ramp(p, 0.3, 0.5))),
                caption=1 - ramp(p, 0.2, 0.3),
                dial=1 - ramp(p, 0.2, 0.4), letters_fade=1,
                divider=1 - ramp(p, 0.1, 0.2),
                hint=1 - ramp(p, 0.0, 0.2))


def sequence(entry, exit_, state='heading', settle_ms=460):
    frames = []
    black = composite(Image.new('RGB', (W, W)), W)
    frames += [black] * 10
    prof = drag_profile()
    for off in prof:
        dx = round(W - off)
        frames.append(composite(draw(state, entry(-1)), dx) if dx > 0 else composite(draw(state, entry(0)), 0))
    for t in range(0, settle_ms, DT):
        frames.append(composite(draw(state, entry(t)), 0))
    frames += [frames[-1]] * 35
    for off in prof:
        p = clamp(off / (0.35 * W))
        frames.append(composite(draw(state, exit_(p)), -round(off)))
    frames += [black] * 15
    return frames


def heading_arrives():
    """Calibration completes: calibrating -> heading, with the page settled."""
    frames = [composite(draw('calibrating', FULL), 0)] * 30
    for t in range(0, 300, DT):
        e = dict(FULL, icon_rows=min(5, t // ROW_MS + 1), caption=ramp(t, 0, 120), dial=ramp(t, 0, 170))
        frames.append(composite(draw('heading', e), 0))
    frames += [frames[-1]] * 45
    return frames


def strip(entry, path, ts):
    cols, sc = 6, 0.5
    w = int(W * sc)
    rows = (len(ts) + cols - 1) // cols
    sh = Image.new('RGB', (cols * (w + 10) + 10, rows * (w + 30) + 10), (28, 28, 28))
    d = ImageDraw.Draw(sh)
    f = ImageFont.truetype('fonts/MonoR.otf', 13)
    for i, t in enumerate(ts):
        im = composite(draw('heading', entry(t)), 0).resize((w, w), Image.LANCZOS)
        x, y = 10 + (i % cols) * (w + 10), 10 + (i // cols) * (w + 30)
        sh.paste(im, (x, y))
        d.text((x, y + w + 6), '%d MS' % t, font=f, fill=(136, 142, 152))
    sh.save(path)


if __name__ == '__main__':
    save_gif(sequence(cur_entry, cur_exit), 'out/compass-anim-current.gif', DT)
    fr = sequence(new_entry, new_exit)
    save_gif(fr, 'out/compass-anim-build.gif', DT)
    save_gif(fr, 'out/compass-anim-build-slow4x.gif', DT * 4)
    save_gif(heading_arrives(), 'out/compass-anim-heading-arrives.gif', DT)
    strip(new_entry, 'out/compass-anim-build-entry-frames.png', list(range(0, 480, 20)))
    print(len(fr))
