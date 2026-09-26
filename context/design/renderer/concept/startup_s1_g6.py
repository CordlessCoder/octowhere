"""G6: the right-hand logo leads a full black wipe, parks, and becomes the card.

Run from renderer: PYTHONPATH=. python3 concept/startup_s1_g6.py OUTPUT_DIR
"""
from functools import lru_cache
from pathlib import Path
import sys

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
from concept import startup_s1_g5 as G5

OUT = Path(sys.argv[1]).resolve() if len(sys.argv)>1 else None
FRAMES = 120
FILL_AT = 2260
WIPE_START = 2500
PARK_AT = 3030
CLEAR_AT = 3500
GROW_START = 3220
GROW_END = 3840
SMALL = .30


def ease(t,a,b):
    return G4.ease((t-a)/(b-a))


@lru_cache(maxsize=36)
def mark_mask(scale):
    return MF.hatched_mask('L1',scale)


def logo(cv,cx,cy,scale):
    mask=ImageChops.offset(mark_mask(round(scale,3)),round((cx-233)*SS),round((cy-233)*SS))
    cv.img.paste(Image.new('RGB',cv.img.size,rgb('LIME')),(0,0),mask)


def frame(n):
    ms=n*FRAME_MS
    cv=Canvas()
    G4.field(cv,ms)
    if ms>=280:
        G2.row(cv,min(99,max(0,int((ms-280)/92)+1)))
    if ms<FILL_AT:G4.outlined_word(cv,ms)
    else:G5.filled_word(cv)

    if ms<WIPE_START:
        cx,cy,scale=404,259,SMALL
    elif ms<PARK_AT:
        travel=ease(ms,WIPE_START,PARK_AT)
        cx=404-(404-233)*travel
        cy=259-(259-233)*travel
        scale=SMALL
        # The dark field starts just behind the mark and erases the entire
        # composition, including the purple columns and microtext.
        cv.rect(cx+30,0,466,466,'BLACK')
    else:
        cx,cy=233,233
        scale=SMALL+(1-SMALL)*ease(ms,GROW_START,GROW_END)
        front=263-(263+40)*ease(ms,PARK_AT,CLEAR_AT)
        cv.rect(front,0,466,466,'BLACK')

    logo(cv,cx,cy,scale)
    return finish(cv)


def sequence():
    frames=[S1.post(n*FRAME_MS) for n in range(S30.LAST+S30.HOLD)]
    frames.extend(frame(n) for n in range(FRAMES))
    frames.extend(LC.card(n) for n in range(LC.F_CLOCK))
    frames.extend(S1.clock_entry(n) for n in range(17))
    frames.extend([frames[-1]]*18)
    return frames


def storyboard(path):
    picks=[
        ('S1 / SELF TEST',S1.post(800)),
        ('G6 / OUTLINE',frame(41)),
        ('G6 / FILLED POP',frame(71)),
        ('G6 / WIPE BEGINS',frame(77)),
        ('G6 / LOGO MOVES',frame(85)),
        ('G6 / LOGO PARKS',frame(91)),
        ('G6 / WIPE CONTINUES',frame(99)),
        ('G6 / LOGO-ONLY',frame(117)),
        ('CARD / INVERSION',LC.card(4)),
        ('CARD / LIME ON BLACK',LC.card(8)),
        ('CARD / IMPACT',LC.card(12)),
        ('CLOCK / F2',S1.clock_entry(16)),
    ]
    cell,gap,cap,margin=233,14,30,20
    sheet=Image.new('RGB',(4*cell+3*gap+2*margin,3*(cell+cap)+2*gap+2*margin),(24,24,27))
    d=ImageDraw.Draw(sheet);f=ImageFont.truetype('fonts/MonoB.otf',14)
    for i,(label,im) in enumerate(picks):
        x=margin+(i%4)*(cell+gap);y=margin+(i//4)*(cell+cap+gap)
        sheet.paste(im.resize((cell,cell),Image.Resampling.LANCZOS),(x,y))
        d.text((x,y+cell+6),label,font=f,fill=(214,220,226))
    sheet.save(path)


if __name__=='__main__':
    OUT.mkdir(parents=True,exist_ok=True)
    storyboard(OUT/'startup-S1-G6-storyboard.png')
    identity=[frame(n) for n in range(FRAMES)]
    save_gif30(identity,str(OUT/'identity-G6-black-logo-wipe.gif'))
    for n,label in ((41,'outline'),(71,'filled'),(85,'moving'),(91,'parked'),(99,'black-continues'),(117,'logo-card')):
        identity[n].save(OUT/f'identity-G6-{label}.png')
    save_gif30(sequence(),str(OUT/'startup-S1-G6-success.gif'))
