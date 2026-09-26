"""Panel motion: pull-down from the clock, entry build, horizontal scroll, drag-up close,
and the clock's re-entry. Frames every 20 ms (assumed)."""
import lib
lib.RECORD = False
from lib import *
from panel import panel, FULL, NCOL
from anim import page as face_page, entry as face_entry, exit_ as face_exit, save_gif, ramp, clamp, drag_profile, DT

W = 466
NCELL = 6
STAGGER = 30


def p_entry(t):
    """Panel accents t ms after it settles."""
    return dict(
        ring=ramp(t, 0, 110),
        title=ramp(t, 0, 120),
        rules=ramp(t, 40, 160),
        rows=[max(0, min(5, int((t - 80 - STAGGER * i) // 30 + 1))) if t >= 80 + STAGGER * i else 0 for i in range(NCELL)],
        index=[ramp(t, 100 + STAGGER * i, 60) for i in range(NCELL)],
        name=[ramp(t, 120 + STAGGER * i, 90) for i in range(NCELL)],
        pips=1.0 if t >= 200 else 0.0,
        hint=ramp(t, 260, 160),
    )


def p_exit(p):
    """Panel accents at offset fraction p (0 at rest, 1 at 35 % of the height)."""
    return dict(
        ring=1 - ramp(p, 0.6, 0.4),
        title=1 - ramp(p, 0.5, 0.3),
        rules=1 - ramp(p, 0.4, 0.4),
        rows=[round(5 * (1 - ramp(p, 0.3, 0.4)))] * NCELL,
        index=[1 - ramp(p, 0.15, 0.25)] * NCELL,
        name=[1 - ramp(p, 0.1, 0.3)] * NCELL,
        pips=0.0 if p > 0.1 else 1.0,
        hint=1 - ramp(p, 0.0, 0.2),
    )


HIDDEN = p_entry(-1)


def mask_circle():
    m = Image.new('L', (W * 4, W * 4), 0)
    ImageDraw.Draw(m).ellipse([0, 0, W * 4 - 1, W * 4 - 1], fill=255)
    return m.resize((W, W), Image.BOX)


MASK = mask_circle()


def vcomposite(top_img, bottom_img, edge):
    """top page drawn with its bottom edge at row `edge`, bottom page with its top edge there."""
    fr = Image.new('RGB', (W, W), (0, 0, 0))
    if top_img is not None and edge > 0:
        fr.paste(top_img.crop((0, W - edge, W, W)), (0, 0))
    if bottom_img is not None and edge < W:
        fr.paste(bottom_img.crop((0, 0, W, W - edge)), (0, edge))
    bg = Image.new('RGB', (W, W), (28, 28, 28))
    bg.paste(fr, (0, 0), MASK)
    return bg


def still(img):
    return vcomposite(None, img, 0)


def pimg(scroll=0.0, e=FULL):
    return panel(scroll=scroll, e=e).render(offpanel=False)


def sequence():
    frames = []
    face_rest = face_page(face_entry('B', 1000))
    frames += [still(face_rest)] * 20
    # pull down: the edge between panel (above) and face (below) moves down with the finger
    for off in drag_profile():
        off = round(off)
        p = clamp(off / (0.35 * W))
        top = pimg(0, HIDDEN) if off < W else pimg(0, p_entry(0))
        frames.append(vcomposite(top, face_page(face_exit('B', p)), off))
    # panel entry
    for t in range(0, 460, DT):
        frames.append(still(pimg(0, p_entry(t))))
    frames += [frames[-1]] * 25
    # scroll to the end and back: follow 0 -> 0.6 over 200 ms, settle to 1 over 160 ms
    def scroll_run(a, b):
        out = []
        for t in range(0, 200, DT):
            out.append(still(pimg(a + (b - a) * 0.6 * t / 200)))
        for t in range(0, 180, DT):
            u = min(1, t / 160)
            out.append(still(pimg(a + (b - a) * (0.6 + 0.4 * (1 - (1 - u) ** 3)))))
        out.append(still(pimg(b)))
        return out
    frames += scroll_run(0, 1)
    frames += [frames[-1]] * 20
    frames += scroll_run(1, 0)
    frames += [frames[-1]] * 15
    # drag up to close: the edge moves up; the face comes back from below with its accents hidden
    for off in drag_profile():
        off = round(off)
        p = clamp(off / (0.35 * W))
        bottom = face_page(face_entry('B', -1)) if off < W else face_page(face_entry('B', 0))
        frames.append(vcomposite(pimg(0, p_exit(p)), bottom, W - off))
    for t in range(0, 500, DT):
        frames.append(still(face_page(face_entry('B', t))))
    frames += [frames[-1]] * 25
    return frames


def strip(path):
    ts = list(range(0, 460, 20))
    cols, sc = 6, 0.5
    w = int(W * sc)
    rows = (len(ts) + cols - 1) // cols
    sh = Image.new('RGB', (cols * (w + 10) + 10, rows * (w + 30) + 10), (28, 28, 28))
    d = ImageDraw.Draw(sh)
    f = ImageFont.truetype('fonts/MonoR.otf', 13)
    for i, t in enumerate(ts):
        im = still(pimg(0, p_entry(t))).resize((w, w), Image.LANCZOS)
        x, y = 10 + (i % cols) * (w + 10), 10 + (i // cols) * (w + 30)
        sh.paste(im, (x, y))
        d.text((x, y + w + 6), '%d MS' % t, font=f, fill=(136, 142, 152))
    sh.save(path)


if __name__ == '__main__':
    fr = sequence()
    save_gif(fr, 'out/panel-anim.gif', DT)
    save_gif(fr, 'out/panel-anim-slow4x.gif', DT * 4)
    strip('out/panel-anim-entry-frames.png')
    # key frames for the spec
    prof = drag_profile()
    still_pull = vcomposite(pimg(0, HIDDEN), face_page(face_exit('B', clamp(200 / (0.35 * W)))), 200)
    still_pull.save('out/panel-pulling.png')
    still_close = vcomposite(pimg(0, p_exit(clamp(160 / (0.35 * W)))), face_page(face_entry('B', -1)), W - 160)
    still_close.save('out/panel-closing.png')
    print(len(fr))
