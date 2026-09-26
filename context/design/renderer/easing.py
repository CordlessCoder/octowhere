"""Easing options for the clock's entry (and, once chosen, every accent build).

The built entry drives each accent with a linear ramp. Steps on this device are discrete (cell
reveals, icon rows, hatch rows, scatter density), so an easing changes when each step lands, not
how smoothly it moves: front-loaded curves put most steps in the first frames and let the last
ones settle.

E1 linear       as built
E2 out-cubic    1 - (1 - u)^3
E3 out-expo     1 - 2^(-10u): snaps, then a long quiet tail
E4 out-back     overshoots about 10 % and settles: only on things that can pass their target
                (the battery hatch, the scatter's density, the ring's fade toward WHITE); the rest
                use E2
"""
import math
import lib
lib.RECORD = False
from lib import *
import clockface4 as CF4
from anim import save_gif


def e_lin(u): return u
def e_cubic(u): return 1 - (1 - u) ** 3
def e_expo(u): return 1.0 if u >= 1 else 1 - 2 ** (-10 * u)


def e_back(u):
    c1 = 1.70158
    c3 = c1 + 1
    return 1 + c3 * (u - 1) ** 3 + c1 * (u - 1) ** 2


def e_in_cubic(u): return u ** 3
def e_inout_cubic(u): return 4 * u ** 3 if u < 0.5 else 1 - (-2 * u + 2) ** 3 / 2
def e_in_expo(u): return 0.0 if u <= 0 else 2 ** (10 * u - 10)
def e_in_quad(u): return u ** 2


EASE = {'E1': (e_lin, e_lin), 'E2': (e_cubic, e_cubic), 'E3': (e_expo, e_expo), 'E4': (e_cubic, e_back)}
EASE.update({'E5': (e_in_quad, e_in_quad), 'E6': (e_in_cubic, e_in_cubic),
             'E7': (e_inout_cubic, e_inout_cubic), 'E8': (e_in_expo, e_in_expo)})
NAMES = {'E1': 'E1  LINEAR (AS BUILT)', 'E2': 'E2  OUT-CUBIC', 'E3': 'E3  OUT-EXPO', 'E4': 'E4  OUT-BACK, OVERSHOOT',
         'E5': 'E5  IN-QUAD', 'E6': 'E6  IN-CUBIC', 'E7': 'E7  IN-OUT CUBIC', 'E8': 'E8  IN-EXPO: HOLD, THEN SNAP'}


def u(t, start, dur):
    return max(0.0, min(1.0, (t - start) / dur))


def entry(t, key):
    ease, over = EASE[key]
    e = dict(icon_fade=1.0)
    e['ring'] = over(u(t, 0, 110)) if t >= 0 else 0
    e['icon_rows'] = int(round(ease(u(t, 60, 150)) * 5)) if t >= 60 else 0
    e['label'] = ease(u(t, 100, 120))
    e['plate'] = ease(u(t, 160, 120))
    e['zone'] = ease(u(t, 240, 160))
    e['mark'] = ease(u(t, 300, 160))
    e['bat'] = over(u(t, 160, 80)) if t >= 160 else 0
    sk = over(u(t, 0, 120)) if t >= 0 else 0
    return e, sk


def curves(path, keys=('E1', 'E2', 'E3', 'E4')):
    Wd, Hd = 900, 360
    im = Image.new('RGB', (Wd, Hd), (20, 20, 22))
    d = ImageDraw.Draw(im)
    f = ImageFont.truetype('fonts/MonoR.otf', 14)
    cols = {'E1': (136, 142, 152), 'E2': rgb('LIME'), 'E3': rgb('BLUE'), 'E4': rgb('ORANGE'),
            'E5': rgb('LIME'), 'E6': rgb('BLUE'), 'E7': rgb('ORANGE'), 'E8': rgb('#E8337C')}
    fns = {k: EASE[k][1] for k in EASE}
    x0, y0, w, h = 40, 40, 520, 260
    d.rectangle([x0, y0, x0 + w, y0 + h], outline=(60, 60, 66))
    d.line([(x0, y0 + h - h / 1.15), (x0 + w, y0 + h - h / 1.15)], fill=(60, 60, 66))
    for k in keys:
        fn = fns[k]
        pts = [(x0 + w * i / 100, y0 + h - h / 1.15 * fn(i / 100)) for i in range(101)]
        d.line(pts, fill=cols[k], width=3)
        # the steps a 6-step reveal would take at 20 ms frames over 120 ms
        for n in range(6):
            uu = (n + 1) / 6
            d.ellipse([x0 + w * uu - 3, y0 + h - h / 1.15 * fn(uu) - 3, x0 + w * uu + 3, y0 + h - h / 1.15 * fn(uu) + 3], fill=cols[k])
    for i, k in enumerate(keys):
        d.text((600, 60 + i * 28), NAMES[k], font=f, fill=cols[k])
    d.text((600, 250), 'DOTS: WHERE A 6-FRAME STEP LANDS', font=f, fill=(136, 142, 152))
    d.text((600, 270), 'THE LINE: THE TARGET (1.0)', font=f, fill=(136, 142, 152))
    d.text((x0, y0 + h + 10), 'TIME ->', font=f, fill=(136, 142, 152))
    im.save(path)


def compare(keys, gif, slow):
    sc = 0.5
    w = int(466 * sc)
    fnt = ImageFont.truetype('fonts/MonoB.otf', 13)
    frames = []
    cols = 3 if len(keys) > 4 else 2
    rows = (len(keys) + cols - 1) // cols
    for t in list(range(-40, 500, 20)):
        sh = Image.new('RGB', (cols * (w + 10) + 10, rows * (w + 30) + 10), (20, 20, 22))
        d = ImageDraw.Draw(sh)
        for i, k in enumerate(keys):
            if t < 0:
                im = CF4.finish(Canvas())
            else:
                e, sk = entry(t, k)
                im = CF4.face('gnss', e=e, scatter_k=sk)
            x, y = 10 + (i % cols) * (w + 10), 10 + (i // cols) * (w + 30)
            sh.paste(im.resize((w, w), Image.LANCZOS), (x, y))
            d.text((x, y + w + 6), NAMES[k], font=fnt, fill=(210, 211, 214))
        frames.append(sh)
    frames += [frames[-1]] * 40
    save_gif(frames, gif, 20)
    save_gif(frames, slow, 60)


if __name__ == '__main__' and __import__('sys').argv[1:] == ['in']:
    keys = ('E1', 'E5', 'E6', 'E7', 'E8', 'E2')
    curves('out/easing-in-curves.png', keys)
    compare(keys, 'out/easing-in-entry.gif', 'out/easing-in-entry-slow3x.gif')
elif __name__ == '__main__':
    curves('out/easing-curves.png')
    sc = 0.5
    w = int(466 * sc)
    fnt = ImageFont.truetype('fonts/MonoB.otf', 13)
    frames = []
    for t in list(range(-40, 500, 20)):
        sh = Image.new('RGB', (2 * (w + 10) + 10, 2 * (w + 30) + 10), (20, 20, 22))
        d = ImageDraw.Draw(sh)
        for i, k in enumerate(('E1', 'E2', 'E3', 'E4')):
            if t < 0:
                im = CF4.finish(Canvas())
            else:
                e, sk = entry(t, k)
                im = CF4.face('gnss', e=e, scatter_k=sk)
            x, y = 10 + (i % 2) * (w + 10), 10 + (i // 2) * (w + 30)
            sh.paste(im.resize((w, w), Image.LANCZOS), (x, y))
            d.text((x, y + w + 6), NAMES[k], font=fnt, fill=(210, 211, 214))
        frames.append(sh)
    frames += [frames[-1]] * 40
    save_gif(frames, 'out/easing-entry.gif', 20)
    save_gif(frames, 'out/easing-entry-slow3x.gif', 60)


# ---- mixes (owner, 25 Sep: out-cubic feels responsive; try mixing) -------------------------------
C, B, X, OX, Q = e_cubic, e_back, e_in_expo, e_expo, e_in_quad
MIX = {
    'M1': ('M1  OUT-CUBIC THROUGHOUT', dict(ring=C, scatter=C, icon=C, label=C, plate=C, zone=C, mark=C, bat=C)),
    'M2': ('M2  M1, FILLS OVERSHOOT (RING, SCATTER, BATTERY)', dict(ring=B, scatter=B, icon=C, label=C, plate=C, zone=C, mark=C, bat=B)),
    'M3': ('M3  M1, THE WORDMARK HOLDS THEN SNAPS', dict(ring=C, scatter=C, icon=C, label=C, plate=C, zone=C, mark=X, bat=C)),
    'M4': ('M4  SCATTER AND RING OUT-EXPO, REST OUT-CUBIC', dict(ring=OX, scatter=OX, icon=C, label=C, plate=C, zone=C, mark=C, bat=B)),
    'M5': ('M5  TEXT OUT-CUBIC, SCATTER BLOOMS LATE (IN-QUAD)', dict(ring=C, scatter=Q, icon=C, label=C, plate=C, zone=C, mark=C, bat=C)),
    'M6': ('M6  M2 + M3: OVERSHOOT FILLS, SNAPPING WORDMARK', dict(ring=B, scatter=B, icon=C, label=C, plate=C, zone=C, mark=X, bat=B)),
}


def entry_mix(t, key):
    m = MIX[key][1]
    e = dict(icon_fade=1.0)
    e['ring'] = m['ring'](u(t, 0, 110)) if t >= 0 else 0
    e['icon_rows'] = int(round(m['icon'](u(t, 60, 150)) * 5)) if t >= 60 else 0
    e['label'] = m['label'](u(t, 100, 120))
    e['plate'] = m['plate'](u(t, 160, 120))
    e['zone'] = m['zone'](u(t, 240, 160))
    e['mark'] = m['mark'](u(t, 300, 160))
    e['bat'] = m['bat'](u(t, 160, 80)) if t >= 160 else 0
    sk = m['scatter'](u(t, 0, 120)) if t >= 0 else 0
    return e, sk


def compare_mix(keys, gif, slow):
    sc = 0.5
    w = int(466 * sc)
    fnt = ImageFont.truetype('fonts/MonoB.otf', 12)
    frames = []
    cols, rows = 3, 2
    for t in list(range(-40, 500, 20)):
        sh = Image.new('RGB', (cols * (w + 10) + 10, rows * (w + 30) + 10), (20, 20, 22))
        d = ImageDraw.Draw(sh)
        for i, k in enumerate(keys):
            if t < 0:
                im = CF4.finish(Canvas())
            else:
                e, sk = entry_mix(t, k)
                im = CF4.face('gnss', e=e, scatter_k=sk)
            x, y = 10 + (i % cols) * (w + 10), 10 + (i // cols) * (w + 30)
            sh.paste(im.resize((w, w), Image.LANCZOS), (x, y))
            d.text((x, y + w + 6), MIX[k][0], font=fnt, fill=(210, 211, 214))
        frames.append(sh)
    frames += [frames[-1]] * 40
    save_gif(frames, gif, 20)
    save_gif(frames, slow, 60)


if __name__ == '__main__' and __import__('sys').argv[1:] == ['mix']:
    compare_mix(('M1', 'M2', 'M3', 'M4', 'M5', 'M6'), 'out/easing-mix.gif', 'out/easing-mix-slow3x.gif')
