"""G14: one fixed curved compression path; only block colours change.

Run from renderer: PYTHONPATH=. python3 concept/startup_s1_g14.py OUTPUT_DIR
"""
from functools import lru_cache
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
import logo_card as LC
import startup30 as S30
from concept import startup_s1 as S1
from concept import startup_s1_g11 as G11
from concept import startup_s1_g12 as G12
from concept import startup_s1_g13 as G13

OUT=Path(sys.argv[1]).resolve() if len(sys.argv)>1 else None
FRAMES=120


@lru_cache(maxsize=1)
def fixed_blocks():
    """Positions, dimensions, scanlines and cutouts are frozen for the full clip."""
    shapes=[]
    for i in range(45):
        rng=random.Random(72113+i*617)
        t=(i+.35*rng.random())/45
        x,y,dx,dy=G13.path(t,'curve')
        mag=math.hypot(dx,dy);nx,ny=-dy/mag,dx/mag
        for side in (-1,1):
            off=side*rng.choice((14,19,26,32))
            xx=x+nx*off+rng.randrange(-4,5)
            yy=y+ny*off+rng.randrange(-3,4)
            w=rng.choice((20,28,38,52,68));h=rng.choice((6,10,15,21,28))
            left,top=round(xx-w/2),round(yy-h/2)
            cuts=[]
            if w>=28:
                for _ in range(rng.randrange(1,4)):
                    cuts.append((rng.randrange(2,w-2),rng.randrange(0,max(1,h-2)),
                                 rng.randrange(2,8),rng.randrange(2,max(4,h//2+2))))
            shapes.append((left,top,w,h,min(7,int(t*8)),side,cuts,rng.choice((.66,.82,1.0))))
            # Fixed small artefacts share the same path and colour sector.
            if rng.random()<.56:
                mx=left+rng.randrange(-18,w+18);my=top+rng.randrange(-15,h+15)
                shapes.append((mx,my,rng.choice((3,5,9,13)),rng.choice((2,3,5)),
                               min(7,int(t*8)),side,[],.42))
    return shapes


@lru_cache(maxsize=256)
def sector_color(group,phase):
    # Asynchronous updates: each sector holds for 2–4 colour epochs. No
    # geometry, visibility mask or texture cutout changes between frames.
    update=phase//(2+group%3)
    rng=random.Random(34831+group*2203+update*883)
    red=(group%2==0) != (rng.random()<.45)
    base=G12.RED if red else G12.BLUE
    strength=rng.choice((.43,.62,.83,1.0))
    return base,strength


def anchored_arc(cv,n):
    phase=n//5
    recovery=.48 if n%23==22 else 1.0
    for x,y,w,h,group,side,cuts,local in fixed_blocks():
        base,strength=sector_color(group,phase)
        # A few blocks in each sector preserve the opposing tone, maintaining
        # red/blue adjacency while the larger colour regions swap.
        if (x//19+group)%7==0:base=G12.BLUE if base==G12.RED else G12.RED
        k=strength*local*recovery
        col=tuple(round(v*k) for v in base)
        cv.rect(x,y,x+w,y+h,col)
        if h>=6:
            lit=tuple(min(255,round(v*1.20)) for v in col)
            for yy in range(y+3,y+h,4):cv.rect(x,yy,x+w,yy+1,lit)
        for hx,hy,hw,hh in cuts:cv.rect(x+hx,y+hy,x+hx+hw,y+hy+hh,'BLACK')


def frame(n):
    ms=n*FRAME_MS
    cv=Canvas()
    anchored_arc(cv,n)
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
    storyboard(OUT/'identity-G14-fixed-arc-storyboard.png')
    identity=[frame(n) for n in range(FRAMES)]
    save_gif30(identity,str(OUT/'identity-G14-fixed-arc-color-changes.gif'))
    for n,label in ((30,'type'),(44,'filled'),(64,'color-a'),(83,'color-b'),(103,'color-c')):
        identity[n].save(OUT/f'identity-G14-{label}.png')
    save_gif30(sequence(),str(OUT/'startup-S1-G14-fixed-arc-success.gif'))
