"""G18 title-background study, retaining G17's entrance, marks, type and card.

Run from renderer: PYTHONPATH=. python3 concept/startup_s1_g18.py OUTPUT_DIR
"""
from pathlib import Path
import math
import random
import sys

from PIL import Image, ImageDraw, ImageFont

import lib
lib.RECORD=False
from lib import Canvas
from identity import finish
from anim import FRAME_MS, save_gif30
import identity_g2 as G2
import startup30 as S30
from concept import startup_s1 as S1
from concept import startup_s1_g17 as G17

OUT=Path(sys.argv[1]).resolve() if len(sys.argv)>1 else None
VARIANTS=('baseline','plume','buffer','hybrid')


def purple_plume(cv,n,brightness=1.0):
    # Halftone clouds in the reference's diagonal upper-right/lower-left
    # composition. Dots refresh as a field every six frames, not by sliding a
    # fixed sheet across the display.
    phase=n//6
    for y in range(72,405,8):
        for x in range(26,444,8):
            w1=math.exp(-(((x-(294+22*math.sin((y-85)/66))) / 74)**2 + ((y-151)/95)**2)*.85)
            w2=math.exp(-(((x-(174+18*math.sin((y-305)/49))) / 85)**2 + ((y-331)/66)**2)*.85)
            w3=math.exp(-(((x-408)/38)**2 + ((y-245)/90)**2)*.95)
            density=max(w1,w2*.68,w3*.45)
            if density<.13:continue
            seed=(x*73856093 ^ y*19349663 ^ phase*83492791)&0xffffffff
            rng=random.Random(seed)
            if rng.random()>min(.58,density*.56):continue
            light=(.42+.58*rng.random())*brightness
            col=(round(42*light),round(5*light),round(117*light))
            size=2 if rng.random()<.80 else 3
            cv.rect(x,y,x+size,y+size,col)


def blue_buffer(cv,n,brightness=.72):
    # Blue compression cells are redrawn on discrete beats. Multiple block
    # sizes and 1 px dark scanlines add texture without a flowing path.
    phase=n//7
    rng=random.Random(78191+phase*373)
    for i in range(88):
        region=rng.random()
        if region<.43:y=rng.randrange(71,187)
        elif region<.86:y=rng.randrange(303,410)
        else:y=rng.randrange(184,306)
        x=rng.randrange(20,442)//4*4
        if 186<=y<=305 and 72<x<394:continue
        w=rng.choice((8,12,16,24,32,48,64))
        h=rng.choice((3,4,7,9,13))
        level=rng.choice(((2,12,39),(3,17,54),(4,23,68),(6,30,84)))
        col=tuple(round(v*brightness) for v in level)
        cv.rect(x,y,min(448,x+w),y+h,col)
        if h>=7:
            for line in range(y+2,y+h,3):
                cv.rect(x,line,min(448,x+w),line+1,tuple(round(v*.45) for v in col))


def frame(n,variant='plume'):
    if n<13:return G17.frame(n)
    cv=Canvas()
    if variant=='baseline':G17.scatter(cv,n)
    elif variant=='plume':purple_plume(cv,n)
    elif variant=='buffer':blue_buffer(cv,n)
    elif variant=='hybrid':
        blue_buffer(cv,n,.42)
        purple_plume(cv,n,.88)
    else:raise ValueError(variant)
    G17.gray_marks(cv,n)
    if n<G17.FLICKER_START:G17.title_ink(cv,'type',n*FRAME_MS)
    else:G17.title_ink(cv,G17.state(n-G17.FLICKER_START))
    if n>=21:G2.row(cv,min(99,n-20))
    G17.lime_ticks(cv,n)
    G17.small_logo(cv,n)
    return finish(cv)


def sequence(variant):
    frames=[S1.post(n*FRAME_MS) for n in range(S30.LAST+S30.HOLD)]
    frames.extend(frame(n,variant) for n in range(G17.FRAMES))
    frames.extend(G17.card(n) for n in range(G17.CARD_FRAMES))
    frames.extend(S1.clock_entry(n) for n in range(1,17))
    frames.extend([frames[-1]]*18)
    return frames


def board(path):
    cell,gap,cap,margin=300,12,29,18
    sh=Image.new('RGB',(4*cell+3*gap+2*margin,2*(cell+cap)+gap+2*margin),(23,23,26))
    d=ImageDraw.Draw(sh);font=ImageFont.truetype('fonts/MonoB.otf',15)
    for i,v in enumerate(VARIANTS):
        for row,n in enumerate((38,71)):
            x=margin+i*(cell+gap);y=margin+row*(cell+cap+gap)
            sh.paste(frame(n,v).resize((cell,cell),Image.Resampling.LANCZOS),(x,y))
            d.text((x,y+cell+6),f'{v.upper()} / {"OUTLINE" if row==0 else "FILLED"}',font=font,fill=(215,220,225))
    sh.save(path)


if __name__=='__main__':
    OUT.mkdir(parents=True,exist_ok=True)
    board(OUT/'identity-G18-background-comparison.png')
    for v in VARIANTS[1:]:
        save_gif30([frame(n,v) for n in range(G17.FRAMES)],str(OUT/f'identity-G18-{v}.gif'))
        frame(71,v).save(OUT/f'identity-G18-{v}-filled.png')
    save_gif30(sequence('plume'),str(OUT/'startup-S1-G18-plume-success.gif'))
