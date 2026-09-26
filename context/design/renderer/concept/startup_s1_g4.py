"""G4 identity: slower outline type-on, single filled pop, four-second identity.

Run from renderer: PYTHONPATH=. python3 concept/startup_s1_g4.py OUTPUT_DIR
"""
import math
import random
import sys
from pathlib import Path

from PIL import Image, ImageChops, ImageDraw, ImageFont

import lib
lib.RECORD=False
from lib import Canvas, SS, rgb
import identity_g as IG
import identity_g2 as G2
import logo_card as LC
import startup30 as S30
from identity import finish
from anim import FRAME_MS, save_gif30
from concept import startup_s1 as S1

OUT=Path(sys.argv[1]).resolve() if len(sys.argv)>1 else None
W=466
FRAMES=120  # 4 seconds at 30 fps, then the existing 13-frame logo card.
OUTLINE_START=520
GLYPH_DELAY=170
GLYPH_FADE=200
FILL_AT=2410


def ease(v):
    v=max(0.,min(1.,v))
    return v*v*(3-2*v)


def field(cv,ms):
    # These long-lived purple vertical signals retain their own grammar, distinct
    # from the clock's blue block/triangle/pixel-family sequence.
    if ms<320:phase=0
    elif ms<1190:phase=1
    elif ms<2240:phase=2
    else:phase=3
    rng=random.Random(38059+phase*967)
    density=(.25,.60,.78,.51)[phase]
    for band in (0,1):
        for _ in range(68):
            x=rng.randrange(8,450)
            y=rng.randrange(10,182) if band==0 else rng.randrange(326,448)
            w=rng.choice([1,1,2,3,4,7,11]);h=rng.choice([7,14,25,38,52,73])
            if rng.random()>density:continue
            # A small eased drift at different rates gives depth without new random
            # pixels on every frame. The last field holds under the finished word.
            drift=ease((ms-(0,320,1190,2240)[phase])/750)
            x+=int((1 if band==0 else -1)*drift*(2 if w<=3 else 5))
            cv.rect(x,y,x+w,y+h,'PURPLE')
            if w>=4:
                for cut in range(y+5,y+h,8):cv.rect(x,cut,x+w,cut+1,'BLACK')
    for x in (29,434):
        cv.rect(x,76,x+1,167,'PURPLE')
        cv.rect(x,348,x+1,403,'PURPLE')


def outlined_word(cv,ms):
    G2.BIG['hatch']=(40,1.8)
    px,sy,mask,cx,cy=G2.word_geom()
    word='OCTOWHERE'
    edge=IG.outline(mask,w=1.55)
    boundaries=[0]+[min(edge.width,IG.tm(word[:i],'shapiro',px,sy=sy).width) for i in range(1,len(word))]+[edge.width]
    # A glyph begins every 170 ms. Its outline eases in over 200 ms; no filled
    # glyph is shown until the entire word changes in one frame.
    alpha=Image.new('L',edge.size,0)
    for i in range(len(word)):
        k=ease((ms-OUTLINE_START-i*GLYPH_DELAY)/GLYPH_FADE)
        if k<=0:continue
        x0,x1=boundaries[i],boundaries[i+1]
        sl=edge.crop((x0,0,x1,edge.height))
        if k<1:sl=sl.point(lambda v:round(v*k))
        alpha.paste(sl,(x0,0))
    left=round((cx-mask.width/SS/2)*SS)
    top=round((cy-mask.height/SS/2)*SS)
    cv.img.paste(Image.new('RGB',edge.size,rgb('LIME')),(left,top),alpha)


def identity_frame(n):
    ms=n*FRAME_MS
    cv=Canvas()
    field(cv,ms)
    if ms>=280:
        G2.row(cv,min(99,max(0,int((ms-280)/92)+1)))
        G2.weight(cv,ease((ms-320)/560))
    if ms<FILL_AT:
        outlined_word(cv,ms)
    else:
        # The filled word appears all at once, then receives a quiet hold.
        _,_,mask,cx,cy=G2.word_geom()
        IG.putc(cv,mask,'LIME',cx,cy)
    return finish(cv)


def sequence():
    fr=[S1.post(n*FRAME_MS) for n in range(S30.LAST+S30.HOLD)]
    fr.extend(identity_frame(n) for n in range(FRAMES))
    fr.extend(LC.card(n) for n in range(LC.F_CLOCK))
    fr.extend(S1.clock_entry(n) for n in range(17))
    fr.extend([fr[-1]]*18)
    return fr


def storyboard(path):
    picks=[
      ('S1 SELF TEST',S1.post(800)),
      ('G4 / FIRST OUTLINE',identity_frame(20)),
      ('G4 / HALF TYPED',identity_frame(41)),
      ('G4 / ALL OUTLINED',identity_frame(66)),
      ('G4 / FILLED POP',identity_frame(73)),
      ('G4 / HELD',identity_frame(113)),
      ('LOGO CARD',LC.card(4)),
      ('CLOCK / ENTRY',S1.clock_entry(5)),
      ('CLOCK / F2',S1.clock_entry(16)),
    ]
    cell=233;gap=14;cap=30;margin=20
    sh=Image.new('RGB',(3*cell+2*gap+2*margin,3*(cell+cap)+2*gap+2*margin),(24,24,27))
    d=ImageDraw.Draw(sh);f=ImageFont.truetype('fonts/MonoB.otf',14)
    for i,(name,im) in enumerate(picks):
        x=margin+(i%3)*(cell+gap);y=margin+(i//3)*(cell+cap+gap)
        sh.paste(im.resize((cell,cell),Image.Resampling.LANCZOS),(x,y))
        d.text((x,y+cell+6),name,font=f,fill=(214,220,226))
    sh.save(path)


if __name__=='__main__':
    OUT.mkdir(parents=True,exist_ok=True)
    storyboard(OUT/'startup-S1-G4-storyboard.png')
    identity_frame(65).save(OUT/'identity-G4-outline-complete.png')
    identity_frame(73).save(OUT/'identity-G4-filled.png')
    save_gif30([identity_frame(n) for n in range(FRAMES)],str(OUT/'identity-G4-outline-typeon.gif'))
    save_gif30(sequence(),str(OUT/'startup-S1-G4-success.gif'))
