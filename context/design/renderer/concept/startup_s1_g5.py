"""G5 identity: outline type-on, logo-led word wipe, and spatially continuous logo card.

Run from renderer: PYTHONPATH=. python3 concept/startup_s1_g5.py OUTPUT_DIR
"""
import sys
from pathlib import Path

from PIL import Image, ImageChops, ImageDraw, ImageFont

import lib
lib.RECORD = False
from lib import Canvas, SS, rgb
from identity import finish
from anim import FRAME_MS, save_gif30
import identity_g2 as G2
import marks_frame as MF
import logo_card as LC
import startup30 as S30
from concept import startup_s1 as S1
from concept import startup_s1_g4 as G4

OUT = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else None
FPS_FRAMES = 120
SWEEP_START, SWEEP_END = 2190, 2940
SLIDE_END = 3260
START_X, LEFT_X, CENTER_X = 404, 47, 233
MARK_Y, CENTER_Y = 259, 233
SMALL_MARK = .30


def ramp(t, start, end):
    return G4.ease((t-start)/(end-start))


def paint_mark(cv, cx, cy, scale):
    mask = MF.hatched_mask('L1', scale)
    mask = ImageChops.offset(mask, round((cx-233)*SS), round((cy-233)*SS))
    cv.img.paste(Image.new('RGB', cv.img.size, rgb('LIME')), (0, 0), mask)


def filled_word(cv, threshold=None):
    _, _, mask, cx, cy = G2.word_geom()
    left = round((cx-mask.width/SS/2)*SS)
    top = round((cy-mask.height/SS/2)*SS)
    if threshold is not None:
        mask = mask.copy()
        cut = max(0, min(mask.width, round((threshold*SS)-left)))
        if cut:
            ImageDraw.Draw(mask).rectangle((0, 0, cut-1, mask.height), fill=0)
    cv.img.paste(Image.new('RGB', mask.size, rgb('LIME')), (left, top), mask)


def identity_frame(n, backdrop='scatter'):
    ms = n*FRAME_MS
    cv = Canvas()
    if backdrop == 'scatter':
        # The original G2 scatter is intentionally preserved, including its open
        # horizontal band; only the final card transition clears it.
        G2.scatter(cv, n, appear=min(1, max(.1, ms/490)))
    elif backdrop == 'columns':
        G4.field(cv, ms)
    else:
        raise ValueError(backdrop)

    if ms >= 280:
        G2.row(cv, min(99, max(0, int((ms-280)/92)+1)))

    if ms < SWEEP_START:
        G4.outlined_word(cv, ms)
        cx, cy, scale = START_X, MARK_Y, SMALL_MARK
    elif ms < SWEEP_END:
        k = ramp(ms, SWEEP_START, SWEEP_END)
        cx, cy, scale = START_X+(LEFT_X-START_X)*k, MARK_Y, SMALL_MARK
        G4.outlined_word(cv, ms)
        # The mark travels right to left: the type becomes solid in its wake.
        filled_word(cv, threshold=cx-35)
    else:
        k = ramp(ms, SWEEP_END, SLIDE_END)
        cx = LEFT_X+(CENTER_X-LEFT_X)*k
        cy = MARK_Y+(CENTER_Y-MARK_Y)*k
        scale = SMALL_MARK+(1-SMALL_MARK)*k
        filled_word(cv)
        # The word, microtext, and field clear together beneath the persistent
        # moving mark. At 3.26 s this is the logo-only inverted card.
        cv.img = Image.blend(cv.img, Image.new('RGB', cv.img.size, rgb('BLACK')), k)

    paint_mark(cv, cx, cy, scale)
    return finish(cv)


def sequence(backdrop='scatter'):
    frames = [S1.post(n*FRAME_MS) for n in range(S30.LAST+S30.HOLD)]
    frames.extend(identity_frame(n, backdrop) for n in range(FPS_FRAMES))
    # One-frame inversion is the impact leading into the clock reveal.
    frames.append(LC.card_l1h(12))
    frames.extend(S1.clock_entry(n) for n in range(17))
    frames.extend([frames[-1]]*18)
    return frames


def storyboard(path):
    picks = [
        ('S1 / SELF TEST', S1.post(800)),
        ('SCATTER / OUTLINE', identity_frame(41)),
        ('COLUMNS / OUTLINE', identity_frame(41, 'columns')),
        ('SCATTER / WIPE BEGINS', identity_frame(67)),
        ('SCATTER / MID WIPE', identity_frame(77)),
        ('SCATTER / FULL WORD', identity_frame(88)),
        ('SCATTER / RETURN', identity_frame(93)),
        ('LOGO / CENTERED CARD', identity_frame(109)),
        ('CLOCK / F2', S1.clock_entry(16)),
    ]
    cell, gap, cap, margin = 233, 14, 30, 20
    sheet = Image.new('RGB', (3*cell+2*gap+2*margin, 3*(cell+cap)+2*gap+2*margin), (24,24,27))
    draw = ImageDraw.Draw(sheet)
    font = ImageFont.truetype('fonts/MonoB.otf', 14)
    for i, (label, im) in enumerate(picks):
        x = margin+(i%3)*(cell+gap)
        y = margin+(i//3)*(cell+cap+gap)
        sheet.paste(im.resize((cell,cell), Image.Resampling.LANCZOS), (x,y))
        draw.text((x,y+cell+6), label, font=font, fill=(214,220,226))
    sheet.save(path)


if __name__ == '__main__':
    OUT.mkdir(parents=True, exist_ok=True)
    storyboard(OUT/'startup-S1-G5-storyboard.png')
    for variant in ('scatter', 'columns'):
        frames = [identity_frame(n, variant) for n in range(FPS_FRAMES)]
        save_gif30(frames, str(OUT/f'identity-G5-{variant}-logo-wipe.gif'))
        for n, label in ((41,'outline'),(77,'wipe'),(109,'logo-card')):
            frames[n].save(OUT/f'identity-G5-{variant}-{label}.png')
    save_gif30(sequence(), str(OUT/'startup-S1-G5-scatter-success.gif'))
