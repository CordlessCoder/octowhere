"""Swipe-in and swipe-out prototypes for the industrial face (V1), two options.

Frames are 20 ms apart. That is an assumption: the compass draws a full frame in
13-23 ms, and this face has not been measured.

A  fade:  the compass's rule. Ring, icon, plate and zone name fade in after the page
          settles, staggered; on the way out they fade with the page's offset.
B  build: the icon's tile travels with the page, and its black modules print one row
          every 30 ms after the page settles, top down. The band label and the zone name reveal cell by
          cell, a solid block standing in each character's cell for one frame before its
          glyph. The plate's cells land one at a time. Going out, the same steps run in
          reverse, driven by the offset, so reversing a drag restores them.
"""
import lib
from lib import *
from face import SYM, X0, BIG, HOURS_BASE, MIN_BASE, SEC_PEN, SEC_PX
from industrial import digits, cells_text, pen_for, plate, ICON96, BAND, FIX

lib.RECORD = False
W = 466
DT = 20
GLYPH = SYM['gnss']
ROW_MS = 30        # icon builds one row of modules every 30 ms: 150 ms for any glyph


def clamp(x):
    return max(0.0, min(1.0, x))


def page(e, f=FIX):
    """The face with every accent under control of e:
    ring, icon_fade, label, plate, zone: 0..1; icon_rows: rows of modules shown, 0..5."""
    cv = Canvas()
    if e['ring'] > 0:
        cv.ring(230, 232, fade('GRAY', e['ring']))
    digits(cv, '%02d' % f['h'], BIG, 'WHITE', X0, HOURS_BASE)
    if e['icon_fade'] > 0:
        icon_sized(cv, 262, 90, fade(e.get('tile', 'BLUE'), e['icon_fade']), e.get('sym', GLYPH),
                   module=16, pad=8, rows_shown=e['icon_rows'])
    clipped_band(cv, BAND[0], BAND[1], 'WHITE')
    cells_text(cv, 'LOCAL', 'bold', 16, 'BLACK', pen_for('LOCAL', 'bold', 16, x=262), 225, e['label'])
    digits(cv, '%02d' % f['m'], BIG, 'BLACK', X0, MIN_BASE)
    digits(cv, '%02d' % f['s'], SEC_PX, 'BLACK', SEC_PEN, MIN_BASE)
    cv.text(f['date'], 'mono', 23, 'WHITE', cx=233, y=334)
    if e.get('fade_mode'):
        # plate and zone fade as whole elements
        if e['plate'] > 0:
            k = e['plate']
            cells = [('tag', f['mode']), ('cell', f['abbr']), ('cell', f['off'])]
            _plate_faded(cv, cells, 360, k)
        if e['zone'] > 0:
            cv.text(f['zone'], 'mono', 14, fade('GRAY', e['zone']), cx=233, y=388)
    else:
        plate(cv, [('tag', f['mode']), ('cell', f['abbr']), ('cell', f['off'])], 360, e['plate'])
        cells_text(cv, f['zone'], 'mono', 14, 'GRAY', pen_for(f['zone'], 'mono', 14, cx=233), 398, e['zone'])
    return cv.render(offpanel=False)


def _plate_faded(cv, cells, top, k, h=20):
    px, padx, gap = 14, 6, 4
    ws = [adv('mono', px) * len(t) + 2 * padx for _, t in cells]
    x = round(233 - (sum(ws) + gap * (len(ws) - 1)) / 2)
    base = top + (h + cap_height('mono', px)) / 2
    for (kind, t), w in zip(cells, ws):
        w = round(w)
        cv.rect(x, top, x + w, top + h, fade('GRAY', k))
        if kind == 'tag':
            cells_text(cv, t, 'bold', px, 'BLACK', x + padx, base)
        else:
            cv.rect(x + 1, top + 1, x + w - 1, top + h - 1, 'BLACK')
            cells_text(cv, t, 'mono', px, fade('WHITE', k), x + padx, base)
        x += w + gap


# ---- accent state over time ------------------------------------------------------------
def ramp(t, start, dur):
    return clamp((t - start) / dur)


def entry(opt, t):
    """Accents t ms after the page settles."""
    if opt == 'A':
        return dict(fade_mode=True, ring=ramp(t, 0, 110), icon_fade=ramp(t, 95, 110), icon_rows=5,
                    label=1.0, plate=ramp(t, 190, 110), zone=ramp(t, 190, 110))
    return dict(ring=ramp(t, 0, 110), icon_fade=1.0, icon_rows=min(5, int((t - 60) // ROW_MS + 1)) if t >= 60 else 0,
                label=ramp(t, 100, 120), plate=ramp(t, 160, 120), zone=ramp(t, 240, 160))


def hidden(opt):
    """Accents while the page is off its settled position on the way in."""
    return entry(opt, -1)


def exit_(opt, p):
    """Accents at offset fraction p of 35 % of the width (0 settled, 1 gone)."""
    if opt == 'A':
        k = 1 - p
        return dict(fade_mode=True, ring=k, icon_fade=k, icon_rows=5, label=1.0, plate=k, zone=k)
    return dict(ring=1 - ramp(p, 0.6, 0.4), icon_fade=1.0, icon_rows=round(5 * (1 - ramp(p, 0.3, 0.5))),
                label=1 - ramp(p, 0.2, 0.3), plate=1 - ramp(p, 0.1, 0.3), zone=1 - ramp(p, 0.0, 0.2))


# ---- page motion --------------------------------------------------------------------------
def drag_profile():
    """Offsets in px, one per frame: a drag of 170 px over 220 ms, release, settle over 180 ms."""
    out = []
    for t in range(0, 220, DT):
        out.append(170 * (t / 220))
    for t in range(0, 200, DT):
        u = min(1, t / 180)
        out.append(170 + (W - 170) * (1 - (1 - u) ** 3))
    out.append(W)
    return out


def composite(img, dx):
    """Page image drawn with its left edge at dx; the neighbouring page is left black."""
    fr = Image.new('RGB', (W, W), (0, 0, 0))
    x0, x1 = max(0, dx), min(W, W + dx)
    if x1 > x0:
        fr.paste(img.crop((x0 - dx, 0, x1 - dx, W)), (x0, 0))
    m = Image.new('L', (W * 4, W * 4), 0)
    ImageDraw.Draw(m).ellipse([0, 0, W * 4 - 1, W * 4 - 1], fill=255)
    bg = Image.new('RGB', (W, W), (28, 28, 28))
    bg.paste(fr, (0, 0), m.resize((W, W), Image.BOX))
    return bg


def sequence(opt):
    frames = []
    black = composite(Image.new('RGB', (W, W)), W)
    frames += [black] * 10
    # swipe in from the right: page offset goes W -> 0
    prof = drag_profile()
    for off in prof:
        dx = round(W - off)
        frames.append(composite(page(hidden(opt)), dx) if dx > 0 else composite(page(entry(opt, 0)), 0))
    # entry sequence after settle
    for t in range(0, 460, DT):
        frames.append(composite(page(entry(opt, t)), 0))
    frames += [frames[-1]] * 35
    # swipe out to the left: page offset goes 0 -> -W
    for off in prof:
        p = clamp(off / (0.35 * W))
        frames.append(composite(page(exit_(opt, p)), -round(off)))
    frames += [black] * 15
    return frames


def save_gif(frames, path, dt):
    pal = [f.convert('P', palette=Image.ADAPTIVE, colors=64) for f in frames]
    pal[0].save(path, save_all=True, append_images=pal[1:], duration=dt, loop=0, disposal=1, optimize=False)


def strip(opt, path):
    """Entry frames 0..440 ms, every 20 ms, as a contact sheet."""
    ts = list(range(0, 460, DT))
    cols = 6
    sc = 0.5
    w = int(W * sc)
    rows = (len(ts) + cols - 1) // cols
    sh = Image.new('RGB', (cols * (w + 10) + 10, rows * (w + 30) + 10), (28, 28, 28))
    d = ImageDraw.Draw(sh)
    f = ImageFont.truetype('fonts/MonoR.otf', 13)
    for i, t in enumerate(ts):
        im = composite(page(entry(opt, t)), 0).resize((w, w), Image.LANCZOS)
        x, y = 10 + (i % cols) * (w + 10), 10 + (i // cols) * (w + 30)
        sh.paste(im, (x, y))
        d.text((x, y + w + 6), '%d MS' % t, font=f, fill=(136, 142, 152))
    sh.save(path)


def resync():
    """GNSS sets the clock while the face shows: GRAY RTC icon, then the tile turns BLUE
    in one frame and the GNSS glyph builds by rows."""
    base = entry('B', 1000)
    frames = [composite(page(dict(base, tile='GRAY', sym=SYM['rtc'])), 0)] * 30
    for t in range(0, 200, DT):
        rows = min(5, t // ROW_MS + 1)
        frames.append(composite(page(dict(base, icon_rows=rows)), 0))
    frames += [frames[-1]] * 40
    return frames


if __name__ == '__main__':
    save_gif(resync(), 'out/anim-B-resync.gif', DT)
    for opt in ('A', 'B'):
        fr = sequence(opt)
        save_gif(fr, 'out/anim-%s.gif' % opt, DT)
        save_gif(fr, 'out/anim-%s-slow4x.gif' % opt, DT * 4)
        strip(opt, 'out/anim-%s-entry-frames.png' % opt)
        print(opt, len(fr), 'frames')
