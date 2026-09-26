"""G13: dual-tone compression fragments follow a diagonal or curved flow path.

Run from renderer: PYTHONPATH=. python3 concept/startup_s1_g13.py OUTPUT_DIR
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
import logo_card as LC
import startup30 as S30
from concept import startup_s1 as S1
from concept import startup_s1_g11 as G11
from concept import startup_s1_g12 as G12

OUT=Path(sys.argv[1]).resolve() if len(sys.argv)>1 else None
FRAMES=120


def path(t,variant):
    if variant=='diagonal':
        return -70+605*t,47+355*t,605,355
    if variant=='curve':
        # A crescent wrapping over the headline: lower left → top → lower right.
        p0=(-42,391);p1=(17,-24);p2=(377,-34);p3=(511,376)
        q=1-t
        x=q**3*p0[0]+3*q*q*t*p1[0]+3*q*t*t*p2[0]+t**3*p3[0]
        y=q**3*p0[1]+3*q*q*t*p1[1]+3*q*t*t*p2[1]+t**3*p3[1]
        dx=3*q*q*(p1[0]-p0[0])+6*q*t*(p2[0]-p1[0])+3*t*t*(p3[0]-p2[0])
        dy=3*q*q*(p1[1]-p0[1])+6*q*t*(p2[1]-p1[1])+3*t*t*(p3[1]-p2[1])
        return x,y,dx,dy
    raise ValueError(variant)


def stream(cv,n,variant):
    # Individual tiles advance in quantized steps along a stable path. Their
    # red/blue halves sit on opposite sides of its tangent and recompose at a
    # slower cadence, giving the noise a clear direction.
    travel=(n//3)*.024
    state=n//7
    dim=.36 if n%11==10 else 1.0
    head=.08+.84*n/(FRAMES-1)
    for i in range(33):
        rng=random.Random(22621+i*491+state*2971)
        t=((i+.42*rng.random())/33+travel)%1
        x,y,dx,dy=path(t,variant)
        mag=math.hypot(dx,dy);nx,ny=-dy/mag,dx/mag
        if rng.random()<.17:continue
        for side,base in ((-1,G12.BLUE),(1,G12.RED)):
            if rng.random()<(.13 if side<0 else .23):continue
            off=side*rng.choice((10,19,29))
            xx=x+nx*off+rng.randrange(-7,8)
            yy=y+ny*off+rng.randrange(-5,6)
            w=rng.choice((14,24,38,57,79));h=rng.choice((4,7,11,17,25))
            quiet=math.exp(-1.4*(((xx-233)/203)**2+((yy-239)/66)**2))
            wave=math.exp(-.5*((t-head)/.15)**2)
            brightness=dim*(1-.75*quiet)*(.28+.72*wave)*rng.choice((.55,.78,1.0))
            col=tuple(round(v*brightness) for v in base)
            G12.plate(cv,round(xx-w/2),round(yy-h/2),w,h,col,rng)
            # Off-register fragments retain the compression artefact grammar.
            for _ in range(rng.randrange(0,3)):
                mx=round(xx+rng.randrange(-24,25));my=round(yy+rng.randrange(-18,19))
                cv.rect(mx,my,mx+rng.choice((2,4,8,13)),my+rng.choice((2,3,5)),
                        tuple(round(v*brightness*.37) for v in base))


def frame(n,variant='curve'):
    ms=n*FRAME_MS
    cv=Canvas()
    stream(cv,n,variant)
    if n<G12.FLICKER_FIRST:G11.typed(cv,ms)
    else:G11.full_word(cv,G12.filled_state(n))
    if ms>=G12.MICRO_START:G2.row(cv,min(99,1+int((ms-G12.MICRO_START)/34)))
    return finish(cv)


def sequence(variant='curve'):
    frames=[S1.post(n*FRAME_MS) for n in range(S30.LAST+S30.HOLD)]
    frames.extend(frame(n,variant) for n in range(FRAMES))
    frames.extend(LC.card(n) for n in range(LC.F_CLOCK))
    frames.extend(S1.clock_entry(n) for n in range(17))
    frames.extend([frames[-1]]*18)
    return frames


def storyboard(path_):
    picks=[]
    for variant in ('diagonal','curve'):
        for n,label in ((25,'TYPE'),(40,'FLICKER'),(64,'SETTLED'),(88,'FLOW'),(111,'LATER')):
            picks.append((f'{variant.upper()} / {label}',frame(n,variant)))
    cell,gap,cap,margin=233,14,30,20
    sh=Image.new('RGB',(5*cell+4*gap+2*margin,2*(cell+cap)+gap+2*margin),(24,24,27))
    d=ImageDraw.Draw(sh);f=ImageFont.truetype('fonts/MonoB.otf',14)
    for i,(label,im) in enumerate(picks):
        x=margin+(i%5)*(cell+gap);y=margin+(i//5)*(cell+cap+gap)
        sh.paste(im.resize((cell,cell),Image.Resampling.LANCZOS),(x,y))
        d.text((x,y+cell+6),label,font=f,fill=(214,220,226))
    sh.save(path_)


if __name__=='__main__':
    OUT.mkdir(parents=True,exist_ok=True)
    storyboard(OUT/'identity-G13-path-study.png')
    for variant in ('diagonal','curve'):
        frames=[frame(n,variant) for n in range(FRAMES)]
        save_gif30(frames,str(OUT/f'identity-G13-{variant}-flow.gif'))
        for n,label in ((25,'type'),(64,'settled'),(88,'flow')):
            frames[n].save(OUT/f'identity-G13-{variant}-{label}.png')
    save_gif30(sequence(),str(OUT/'startup-S1-G13-curve-success.gif'))
