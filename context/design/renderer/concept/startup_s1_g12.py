"""G12: short title flicker over moving purple columns or dual-tone artifacts.

Run from renderer: PYTHONPATH=. python3 concept/startup_s1_g12.py OUTPUT_DIR
"""
from functools import lru_cache
from pathlib import Path
import math
import random
import sys

from PIL import Image, ImageDraw, ImageFont

import lib
lib.RECORD=False
from lib import Canvas, fade
from identity import finish
from anim import FRAME_MS, save_gif30
import identity_g2 as G2
import logo_card as LC
import startup30 as S30
from concept import startup_s1 as S1
from concept import startup_s1_g11 as G11

OUT=Path(sys.argv[1]).resolve() if len(sys.argv)>1 else None
FRAMES=120
FLICKER_FIRST=36       # about 1.20 seconds
MICRO_START=1550
RED=(229,42,23)
BLUE=(28,69,228)


def filled_state(n):
    # Eight frames (~267 ms), then no further title flicker.
    if n<37:return False
    if n==37:return True
    if n==38:return False
    if n==39:return True
    if n==40:return False
    if n in (41,42):return True
    if n==43:return False
    return True


@lru_cache(maxsize=1)
def column_base():
    rng=random.Random(58331)
    shapes=[]
    for _ in range(285):
        x=rng.randrange(8,450);y=rng.randrange(8,451)
        w=rng.choice((1,1,2,3,4,7,11));h=rng.choice((9,15,23,36,53,72))
        quiet=math.exp(-1.5*(((x-233)/185)**2+((y-245)/89)**2))
        if rng.random()>.68*(1-.72*quiet):continue
        group=(0 if x<233 else 1)+(0 if y<233 else 2)
        strength=(.72 if rng.random()<.7 else 1.0)*(1-.68*quiet)
        shapes.append((x,y,w,h,group,strength))
    return shapes


@lru_cache(maxsize=128)
def offset(epoch,group):
    if epoch<0:return 0,0
    rng=random.Random(12659+epoch*1747+group*4087)
    return rng.randrange(-42,43),rng.randrange(-31,32)


def moving_columns(cv,n):
    epoch,local=divmod(n,8)          # a new position every ~267 ms
    k=(.52,.85,1.0)[min(local,2)]   # two short moves, then a hold
    for x,y,w,h,group,strength in column_base():
        old=offset(epoch-1,group);new=offset(epoch,group)
        xx=round(x+old[0]+(new[0]-old[0])*k)
        yy=round(y+old[1]+(new[1]-old[1])*k)
        col=fade('PURPLE',strength)
        cv.rect(xx,yy,xx+w,yy+h,col)
        if w>=4:
            for cut in range(yy+5,yy+h,9):cv.rect(xx,cut,xx+w,cut+1,'BLACK')


def plate(cv,x,y,w,h,color,rng):
    cv.rect(x,y,x+w,y+h,color)
    if h>5:
        for yy in range(y+3,y+h,4):
            cv.rect(x,yy,x+w,yy+1,tuple(min(255,int(v*1.25)) for v in color))
    if w>22:
        for _ in range(rng.randrange(1,4)):
            xx=x+rng.randrange(1,w)
            cv.rect(xx,y+rng.randrange(0,max(1,h-2)),xx+rng.randrange(2,8),y+h,'BLACK')


def dual_tone(cv,n):
    epoch,local=divmod(n,7)            # fresh compression state every ~233 ms
    rng=random.Random(71389+epoch*2269)
    # Hold each reconstruction unchanged; one dark frame separates it from
    # the next seed. No intermediate brightness ramp.
    dark=.18 if local==6 else 1.0
    # Broad, jagged red/blue fragments favour the outer sectors. Adjacent
    # opposing colours echo the split plates around 0:29 and 1:06.
    for i in range(25):
        x=rng.randrange(-25,450)
        y=rng.choice((rng.randrange(18,180),rng.randrange(302,448),rng.randrange(172,314)))
        w=rng.choice((12,20,34,55,83,112));h=rng.choice((3,5,9,15,23,38))
        center=math.exp(-1.2*(((x-233)/190)**2+((y-241)/78)**2))
        strength=dark*(1-.72*center)
        base=RED if (i+epoch)%3==0 else BLUE
        color=tuple(round(v*strength) for v in base)
        plate(cv,x,y,w,h,color,rng)
        if w>54 and rng.random()<.48:
            other=BLUE if base==RED else RED
            col2=tuple(round(v*strength*.78) for v in other)
            plate(cv,x+w//2,y+rng.randrange(-5,6),w//2,h//2+2,col2,rng)
    # Pixel blocks and thin streaks behave like separate compression layers.
    for i in range(95):
        x=rng.randrange(12,452);y=rng.randrange(12,450)
        w=rng.choice((2,4,6,11,17));h=rng.choice((2,3,5,9))
        base=RED if rng.random()<.38 else BLUE
        k=dark*rng.choice((.18,.3,.42,.66))
        cv.rect(x,y,x+w,y+h,tuple(round(v*k) for v in base))


def frame(n,variant='moving'):
    ms=n*FRAME_MS
    cv=Canvas()
    if variant=='moving':moving_columns(cv,n)
    elif variant=='dual':dual_tone(cv,n)
    else:raise ValueError(variant)
    if n<FLICKER_FIRST:G11.typed(cv,ms)
    else:G11.full_word(cv,filled_state(n))
    if ms>=MICRO_START:G2.row(cv,min(99,1+int((ms-MICRO_START)/34)))
    return finish(cv)


def sequence(variant='dual'):
    frames=[S1.post(n*FRAME_MS) for n in range(S30.LAST+S30.HOLD)]
    frames.extend(frame(n,variant) for n in range(FRAMES))
    frames.extend(LC.card(n) for n in range(LC.F_CLOCK))
    frames.extend(S1.clock_entry(n) for n in range(17))
    frames.extend([frames[-1]]*18)
    return frames


def storyboard(path):
    picks=[
        ('MOVING / TYPE',frame(27)),('DUAL / TYPE',frame(27,'dual')),
        ('MOVING / OUTLINE',frame(36)),('DUAL / OUTLINE',frame(36,'dual')),
        ('MOVING / FLICKER',frame(39)),('DUAL / FLICKER',frame(39,'dual')),
        ('MOVING / SETTLED',frame(63)),('DUAL / SETTLED',frame(63,'dual')),
        ('DUAL / LATER HIT',frame(101,'dual')),('CARD / IMPACT',LC.card(12)),
    ]
    cell,gap,cap,margin=233,14,30,20
    sh=Image.new('RGB',(5*cell+4*gap+2*margin,2*(cell+cap)+gap+2*margin),(24,24,27))
    d=ImageDraw.Draw(sh);f=ImageFont.truetype('fonts/MonoB.otf',14)
    for i,(label,im) in enumerate(picks):
        x=margin+(i%5)*(cell+gap);y=margin+(i//5)*(cell+cap+gap)
        sh.paste(im.resize((cell,cell),Image.Resampling.LANCZOS),(x,y))
        d.text((x,y+cell+6),label,font=f,fill=(214,220,226))
    sh.save(path)


if __name__=='__main__':
    OUT.mkdir(parents=True,exist_ok=True)
    storyboard(OUT/'identity-G12-background-comparison.png')
    for variant in ('moving','dual'):
        frames=[frame(n,variant) for n in range(FRAMES)]
        save_gif30(frames,str(OUT/f'identity-G12-{variant}-short-flicker.gif'))
        for n,label in ((27,'type'),(39,'flicker'),(63,'settled'),(101,'later')):
            frames[n].save(OUT/f'identity-G12-{variant}-{label}.png')
    save_gif30(sequence('dual'),str(OUT/'startup-S1-G12-dual-success.gif'))
