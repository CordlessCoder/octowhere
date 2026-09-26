"""G15: continuous, stationary dual-tone arc with recoloured internal blocks.

Run from renderer: PYTHONPATH=. python3 concept/startup_s1_g15.py OUTPUT_DIR
"""
from functools import lru_cache
from pathlib import Path
import math
import random
import sys

from PIL import Image, ImageChops, ImageDraw, ImageFont

import lib
lib.RECORD=False
from lib import Canvas, SS, rgb
from identity import finish
from anim import FRAME_MS, save_gif30
import identity_g2 as G2
import logo_card as LC
import startup30 as S30
from concept import startup_s1 as S1
from concept import startup_s1_g11 as G11
from concept import startup_s1_g12 as G12

OUT=Path(sys.argv[1]).resolve() if len(sys.argv)>1 else None
FRAMES=120
CX,CY=310,270
OUTER,INNER=245,166
SECTORS=24
BLUE=(20,54,182)
RED=(238,35,24)


def box(radius):
    return ((CX-radius)*SS,(CY-radius)*SS,(CX+radius)*SS,(CY+radius)*SS)


@lru_cache(maxsize=1)
def band_mask():
    mask=Image.new('L',(466*SS,466*SS),0)
    d=ImageDraw.Draw(mask)
    d.pieslice(box(OUTER),180,360,fill=255)
    d.pieslice(box(INNER),180,360,fill=0)
    return mask


@lru_cache(maxsize=1)
def cells():
    rng=random.Random(77531)
    blocks=[]
    for i in range(105):
        angle=math.radians(181+rng.random()*178)
        radius=rng.randrange(INNER+3,OUTER-1)
        x=CX+radius*math.cos(angle)
        y=CY+radius*math.sin(angle)
        w=rng.choice((5,9,17,29,46));h=rng.choice((2,3,5,8,13))
        blocks.append((round(x-w/2),round(y-h/2),w,h,
                       min(SECTORS-1,int((math.degrees(angle)-180)/180*SECTORS)),
                       rng.choice((.28,.42,.57,.74))))
    return blocks


def sector_color(segment,n):
    # Static wedges. Small groups change hue and lightness at different beats.
    epoch=n//5
    group=segment//3
    update=epoch//(2+group%3)
    rng=random.Random(41923+group*2203+update*719)
    red=(group in (1,2,5,7)) != (rng.random()<.32)
    base=RED if red else BLUE
    k=rng.choice((.66,.82,1.0))
    if n%23==22:k*=.48
    return tuple(round(v*k) for v in base)


def continuous_arc(cv,n):
    # A complete blue ribbon is painted first. Angular colour panels may change
    # but can never remove material from the path.
    cv.d.pieslice(box(OUTER),180,360,fill=BLUE)
    for i in range(SECTORS):
        a=180+i*180/SECTORS
        b=180+(i+1)*180/SECTORS
        cv.d.pieslice(box(OUTER),a,b,fill=sector_color(i,n))
    cv.d.pieslice(box(INNER),180,360,fill=rgb('BLACK'))

    # Fixed compression cells and thin scanlines colour the *interior* of the
    # ribbon. The transparent overlay is clipped to its mask; no black holes
    # are cut out of the continuous silhouette.
    texture=Image.new('RGB',cv.img.size,(0,0,0))
    cover=Image.new('L',cv.img.size,0)
    td=ImageDraw.Draw(texture);md=ImageDraw.Draw(cover)
    for x,y,w,h,sector,strength in cells():
        hue=sector_color(sector,n)
        col=tuple(round(v*strength) for v in hue)
        xy=(x*SS,y*SS,(x+w)*SS-1,(y+h)*SS-1)
        td.rectangle(xy,fill=col);md.rectangle(xy,fill=255)
        if w>16 and h>3:
            line=(x*SS,(y+2)*SS,(x+w)*SS-1,(y+2)*SS)
            td.rectangle(line,fill=tuple(min(255,round(v*1.5)) for v in col))
    cv.img.paste(texture,(0,0),ImageChops.multiply(cover,band_mask()))


def frame(n):
    ms=n*FRAME_MS
    cv=Canvas()
    continuous_arc(cv,n)
    if n<G12.FLICKER_FIRST:G11.typed(cv,ms)
    else:G11.full_word(cv,G12.filled_state(n))
    if ms>=G12.MICRO_START:G2.row(cv,min(99,1+int((ms-G12.MICRO_START)/34)))
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
        ('TYPE / COLOUR A',frame(24)),('TYPE / COLOUR B',frame(30)),
        ('OUTLINE / COLOUR C',frame(38)),('FILLED / COLOUR D',frame(44)),
        ('SETTLED / COLOUR E',frame(64)),('SETTLED / COLOUR F',frame(83)),
        ('SETTLED / COLOUR G',frame(103)),('CARD / IMPACT',LC.card(12)),
    ]
    cell,gap,cap,margin=233,14,30,20
    sh=Image.new('RGB',(4*cell+3*gap+2*margin,2*(cell+cap)+gap+2*margin),(24,24,27))
    d=ImageDraw.Draw(sh);f=ImageFont.truetype('fonts/MonoB.otf',14)
    for i,(label,im) in enumerate(picks):
        x=margin+(i%4)*(cell+gap);y=margin+(i//4)*(cell+cap+gap)
        sh.paste(im.resize((cell,cell),Image.Resampling.LANCZOS),(x,y))
        d.text((x,y+cell+6),label,font=f,fill=(214,220,226))
    sh.save(path)


if __name__=='__main__':
    OUT.mkdir(parents=True,exist_ok=True)
    storyboard(OUT/'identity-G15-continuous-arc-storyboard.png')
    identity=[frame(n) for n in range(FRAMES)]
    save_gif30(identity,str(OUT/'identity-G15-continuous-arc.gif'))
    for n,label in ((30,'type'),(44,'filled'),(64,'colour-a'),(83,'colour-b'),(103,'colour-c')):
        identity[n].save(OUT/f'identity-G15-{label}.png')
    save_gif30(sequence(),str(OUT/'startup-S1-G15-continuous-arc-success.gif'))
