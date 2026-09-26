"""Start-up at 30 fps (round 3, after the owner's choices of 25 Sep):
self-test -> hold -> identity G2 (layout B) -> handover to the clock, and the failed path
self-test -> the failing cell RED -> the fault screen K for 4 s -> the clock.
Frames at 30 fps. The self-test's answer times are illustrative."""
import lib
lib.RECORD = False
from lib import *
import startup as S
import identity_g2 as G2
import identity_g as IG
from anim import save_gif30, FRAME_MS

S.ROW_MS = 30                          # the self-test is functional: ms timings, rendered at 30 fps
HOLD = 6                               # 200 ms held after the last cell passes
LAST = round((max(c[2] for c in S.CELLS)) / FRAME_MS) + 5   # the last glyph built
FAIL_HOLD, K_FRAMES = 9, 120           # 300 ms, then the fault screen's 120 frames (4 s)


def ok_frames():
    fr = []
    for n in range(LAST + HOLD):
        fr.append(S.post(n * FRAME_MS))
    for n in range(G2.REST + 13 + 16):
        fr.append(G2.render(n))
    return fr


FCELLS = [c if c[0] != 'MAGNET' else (c[0], c[1], 300, False) for c in S.CELLS]


def fail_frames():
    """K cuts on once every cell has been decided (the last glyph built), plus the 9-frame hold."""
    fr = []
    for n in range(LAST + FAIL_HOLD):
        fr.append(S.post(n * FRAME_MS, FCELLS))
    for n in range(K_FRAMES):
        fr.append(IG.k_frame(n * FRAME_MS))
    for n in range(16):
        fr.append(G2.clock_in(n))
    return fr


if __name__ == '__main__':
    black = S.still(Image.new('RGB', (466, 466)))
    ok = [black] * 8 + ok_frames() + [None]
    ok[-1] = ok[-2]
    ok += [ok[-1]] * 40
    save_gif30(ok, 'out/startup.gif')
    save_gif30(ok, 'out/startup-slow2x.gif', 2)
    fail = [black] * 8 + fail_frames()
    fail += [fail[-1]] * 40
    save_gif30(fail, 'out/startup-failed.gif')
    # overview: self-test stages, the failure, K
    shots = []
    for n in (0, 3, 8, LAST - 1):
        shots.append((S.post(n * FRAME_MS), 'SELF TEST F%d' % n))
    fail_at = round(300 / FRAME_MS)
    shots.append((S.post((fail_at + 1) * FRAME_MS, FCELLS), 'MAGNETOMETER FAILS AT ITS DEADLINE'))
    shots.append((S.post((LAST + 2) * FRAME_MS, FCELLS), 'ALL DECIDED, HELD 9 FRAMES'))
    shots.append((IG.k_frame(20 * FRAME_MS), 'FAULT SCREEN K'))
    shots.append((G2.clock_in(15), 'AFTER 120 FRAMES: THE CLOCK'))
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
