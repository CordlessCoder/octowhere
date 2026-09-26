"""G8 identity: vertical columns, outlined type-on, filled pop, empty right side.

Run from renderer: PYTHONPATH=. python3 concept/startup_s1_g8.py OUTPUT_DIR
"""
from pathlib import Path
import sys

from PIL import Image, ImageDraw, ImageFont

import lib
lib.RECORD=False
from lib import Canvas
from identity import finish
from anim import FRAME_MS, save_gif30
import identity_g as IG
import identity_g2 as G2
import logo_card as LC
import startup30 as S30
from concept import startup_s1 as S1
from concept import startup_s1_g4 as G4

OUT=Path(sys.argv[1]).resolve() if len(sys.argv)>1 else None
FRAMES=120


def frame(n):
    ms=n*FRAME_MS
    cv=Canvas()
    G4.field(cv,ms)
    if ms>=280:
        G2.row(cv,min(99,max(0,int((ms-280)/92)+1)))
    if ms<G4.FILL_AT:
        G4.outlined_word(cv,ms)
    else:
        _,_,mask,cx,cy=G2.word_geom()
        IG.putc(cv,mask,'LIME',cx,cy)
    # No right-hand hatch or replacement object; preserve the text placement.
    return finish(cv)


def sequence():
    frames=[S1.post(n*FRAME_MS) for n in range(S30.LAST+S30.HOLD)]
    frames.extend(frame(n) for n in range(FRAMES))
    # A direct cut starts the original 13-frame, hard-stepped card sequence.
    frames.extend(LC.card(n) for n in range(LC.F_CLOCK))
    frames.extend(S1.clock_entry(n) for n in range(17))
    frames.extend([frames[-1]]*18)
    return frames


def storyboard(path):
    picks=[
        ('S1 / SELF TEST',S1.post(800)),
        ('FIRST OUTLINE',frame(20)),
        ('HALF TYPED',frame(41)),
        ('ALL OUTLINED',frame(66)),
        ('FILLED POP',frame(73)),
        ('HELD / OPEN RIGHT',frame(113)),
        ('HARD CUT TO CARD',LC.card(0)),
        ('CARD / INVERTED',LC.card(8)),
        ('CLOCK / F2',S1.clock_entry(16)),
    ]
    cell,gap,cap,margin=233,14,30,20
    sh=Image.new('RGB',(3*cell+2*gap+2*margin,3*(cell+cap)+2*gap+2*margin),(24,24,27))
    d=ImageDraw.Draw(sh);f=ImageFont.truetype('fonts/MonoB.otf',14)
    for i,(label,im) in enumerate(picks):
        x=margin+(i%3)*(cell+gap);y=margin+(i//3)*(cell+cap+gap)
        sh.paste(im.resize((cell,cell),Image.Resampling.LANCZOS),(x,y))
        d.text((x,y+cell+6),label,font=f,fill=(214,220,226))
    sh.save(path)


if __name__=='__main__':
    OUT.mkdir(parents=True,exist_ok=True)
    storyboard(OUT/'startup-S1-G8-storyboard.png')
    identity=[frame(n) for n in range(FRAMES)]
    save_gif30(identity,str(OUT/'identity-G8-columns-no-right-block.gif'))
    identity[66].save(OUT/'identity-G8-outlined.png')
    identity[113].save(OUT/'identity-G8-filled.png')
    save_gif30(sequence(),str(OUT/'startup-S1-G8-success.gif'))
