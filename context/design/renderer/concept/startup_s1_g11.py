"""G11: rapid outline type-on, hard outline/fill flicker, optional shrinking columns.

Run from renderer: PYTHONPATH=. python3 concept/startup_s1_g11.py OUTPUT_DIR
"""
from functools import lru_cache
from pathlib import Path
import random
import sys

from PIL import Image, ImageDraw, ImageFont

import lib
lib.RECORD=False
from lib import Canvas, SS, rgb
from identity import finish
from anim import FRAME_MS, save_gif30
import identity_g as IG
import identity_g2 as G2
import logo_card as LC
import startup30 as S30
from concept import startup_s1 as S1
from concept import startup_s1_g4 as G4
from concept import startup_s1_g10 as G10

OUT=Path(sys.argv[1]).resolve() if len(sys.argv)>1 else None
FRAMES=120
TYPE_START=430
GLYPH_INTERVAL=77
GLYPH_RISE=60
FLICKER_FIRST=39
MICRO_START=2090


@lru_cache(maxsize=1)
def art():
    mask,left,top=G10.word_geometry()
    edge=IG.outline(mask,w=1.55)
    bounds=[0]+[min(edge.width,IG.tm(G10.TITLE[:i],'shapiro',G10.PX,sy=G10.SY).width)
                for i in range(1,len(G10.TITLE))]+[edge.width]
    return mask,edge,left,top,bounds


def typed(cv,ms):
    _,edge,left,top,bounds=art()
    alpha=Image.new('L',edge.size,0)
    for i in range(len(G10.TITLE)):
        k=G4.ease((ms-TYPE_START-i*GLYPH_INTERVAL)/GLYPH_RISE)
        if k<=0:continue
        x0,x1=bounds[i],bounds[i+1]
        section=edge.crop((x0,0,x1,edge.height))
        if k<1:section=section.point(lambda v:round(v*k))
        alpha.paste(section,(x0,0))
    cv.img.paste(Image.new('RGB',edge.size,rgb('LIME')),(left,top),alpha)


def filled_state(n):
    # Abrupt 1–4-frame changes, with increasingly long filled states.
    if n<42:return False
    if n<44:return True
    if n<47:return False
    if n<48:return True
    if n<51:return False
    if n<55:return True
    if n<57:return False
    if n<59:return True
    if n<61:return False
    return True


def full_word(cv,fill):
    mask,edge,left,top,_=art()
    ink=mask if fill else edge
    cv.img.paste(Image.new('RGB',ink.size,rgb('LIME')),(left,top),ink)


@lru_cache(maxsize=1)
def shutters():
    rng=random.Random(61457)
    return [(x+rng.randrange(-4,5),rng.choice((6,8,11,15,19)),
             rng.choice((196,223,240,257)),rng.randrange(90,165),rng.randrange(0,185))
            for x in range(34,436,14)]


def shrinking_columns(cv,ms):
    if ms<140 or ms>=1320:return
    for x,w,cy,height,delay in shutters():
        k=G4.ease((ms-450-delay)/615)
        half=round((height*(1-k)/2)/4)*4
        if half<=0:continue
        top,bottom=cy-half,cy+half
        cv.rect(x,top,x+w,bottom,'PURPLE')
        # Small black gaps keep the bars in the same signal vocabulary.
        if w>=11:
            for y in range(top+5,bottom,9):cv.rect(x,y,x+w,y+1,'BLACK')


def frame(n,variant='typing'):
    ms=n*FRAME_MS
    cv=Canvas()
    G10.field(cv,ms)
    if n<FLICKER_FIRST:typed(cv,ms)
    else:full_word(cv,filled_state(n))
    if variant=='columns':shrinking_columns(cv,ms)
    elif variant!='typing':raise ValueError(variant)
    if ms>=MICRO_START:G2.row(cv,min(99,1+int((ms-MICRO_START)/34)))
    return finish(cv)


def sequence(variant='columns'):
    frames=[S1.post(n*FRAME_MS) for n in range(S30.LAST+S30.HOLD)]
    frames.extend(frame(n,variant) for n in range(FRAMES))
    frames.extend(LC.card(n) for n in range(LC.F_CLOCK))
    frames.extend(S1.clock_entry(n) for n in range(17))
    frames.extend([frames[-1]]*18)
    return frames


def storyboard(path):
    picks=[
        ('TYPE / EARLY',frame(20)),('COLUMNS / EARLY',frame(20,'columns')),
        ('TYPE / NEAR COMPLETE',frame(31)),('COLUMNS / SHRINKING',frame(31,'columns')),
        ('TYPE / OUTLINE',frame(40)),('COLUMNS / REVEALED',frame(40,'columns')),
        ('FLICKER / OUTLINE',frame(46)),('FLICKER / FILLED',frame(48)),
        ('SETTLED / FILLED',frame(68)),('CARD / IMPACT',LC.card(12)),
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
    storyboard(OUT/'identity-G11-comparison-storyboard.png')
    for variant in ('typing','columns'):
        frames=[frame(n,variant) for n in range(FRAMES)]
        save_gif30(frames,str(OUT/f'identity-G11-{variant}-flicker.gif'))
        for n,label in ((31,'typeon'),(46,'outline'),(48,'filled'),(68,'settled')):
            frames[n].save(OUT/f'identity-G11-{variant}-{label}.png')
    save_gif30(sequence(),str(OUT/'startup-S1-G11-columns-success.gif'))
