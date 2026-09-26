"""G7 identity: hard-stepped black wipe leads a later, snapping logo.

Run from renderer: PYTHONPATH=. python3 concept/startup_s1_g7.py OUTPUT_DIR
"""
from pathlib import Path
import sys

from PIL import Image, ImageDraw, ImageFont

import lib
lib.RECORD=False
from lib import Canvas
from identity import finish
from anim import FRAME_MS, save_gif30
import identity_g2 as G2
import logo_card as LC
import startup30 as S30
from concept import startup_s1 as S1
from concept import startup_s1_g4 as G4
from concept import startup_s1_g5 as G5
from concept import startup_s1_g6 as G6

OUT=Path(sys.argv[1]).resolve() if len(sys.argv)>1 else None
FRAMES=120
FILL_AT=2260
WIPE_FRAME=75              # 2.5 seconds

# Positions are deliberate holds followed by jumps, not interpolated keyframes.
# The black boundary stays left of the mark and advances farther on each hit.
BLACK_EDGE=(406,352,352,263,263,149,149,63,63,0)
LOGO_X=(404,404,365,365,299,299,233,233,233,233)
LOGO_Y=(259,259,253,253,244,244,233,233,233,233)


def wipe_state(n):
    i=max(0,min(len(BLACK_EDGE)-1,n-WIPE_FRAME))
    front=BLACK_EDGE[i]
    x,y=LOGO_X[i],LOGO_Y[i]
    if n<85:s=.30
    elif n<89:s=.30
    elif n<92:s=.58
    elif n<95:s=1.07
    elif n<98:s=.96
    else:s=1.0
    return front,x,y,s


def frame(n):
    ms=n*FRAME_MS
    cv=Canvas()
    G4.field(cv,ms)
    if ms>=280:G2.row(cv,min(99,max(0,int((ms-280)/92)+1)))
    if ms<FILL_AT:G4.outlined_word(cv,ms)
    else:G5.filled_word(cv)
    if n<WIPE_FRAME:
        x,y,s=404,259,.30
    else:
        front,x,y,s=wipe_state(n)
        cv.rect(front,0,466,466,'BLACK')
    G6.logo(cv,x,y,s)
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
        ('G7 / OUTLINE',frame(41)),
        ('G7 / FILLED POP',frame(71)),
        ('WIPE / FIRST HIT',frame(75)),
        ('BLACK JUMPS AHEAD',frame(78)),
        ('LOGO JUMPS LATER',frame(79)),
        ('LOGO / PARKED',frame(81)),
        ('BLACK / FINAL HIT',frame(84)),
        ('LOGO / SIZE JUMP',frame(89)),
        ('LOGO / HELD',frame(113)),
        ('CARD / INVERSION',LC.card(4)),
        ('CLOCK / F2',S1.clock_entry(16)),
    ]
    cell,gap,cap,margin=233,14,30,20
    sh=Image.new('RGB',(4*cell+3*gap+2*margin,3*(cell+cap)+2*gap+2*margin),(24,24,27))
    d=ImageDraw.Draw(sh);f=ImageFont.truetype('fonts/MonoB.otf',14)
    for i,(label,im) in enumerate(picks):
        x=margin+(i%4)*(cell+gap);y=margin+(i//4)*(cell+cap+gap)
        sh.paste(im.resize((cell,cell),Image.Resampling.LANCZOS),(x,y))
        d.text((x,y+cell+6),label,font=f,fill=(214,220,226))
    sh.save(path)


if __name__=='__main__':
    OUT.mkdir(parents=True,exist_ok=True)
    storyboard(OUT/'startup-S1-G7-storyboard.png')
    identity=[frame(n) for n in range(FRAMES)]
    save_gif30(identity,str(OUT/'identity-G7-aggressive-black-wipe.gif'))
    for n,label in ((71,'filled'),(76,'black-leads'),(79,'logo-follows'),(81,'logo-parks'),(83,'wipe-continues'),(89,'size-jump'),(113,'card-hold')):
        identity[n].save(OUT/f'identity-G7-{label}.png')
    save_gif30(sequence(),str(OUT/'startup-S1-G7-success.gif'))
