"""G10 identity: large centered title, microtext beneath, continuous column field.

Run from renderer: PYTHONPATH=. python3 concept/startup_s1_g10.py OUTPUT_DIR
"""
from pathlib import Path
import math
import random
import sys

from PIL import Image, ImageDraw, ImageFont

import lib
lib.RECORD=False
from lib import Canvas, SS, rgb, fade
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
TITLE='OCTOWHERE'
PX,SY=49,1.0
TITLE_CX,TITLE_CY=233,233
FILL_AT=2410

# Preserve the 326 px microtext row, center it and place it under the title.
G2.LEFT=70
G2.ROW_TOP=268


def field(cv,ms):
    # One continuous field: the columns cross the old central gap. Density and
    # lightness fall near the title instead of stopping at a horizontal edge.
    if ms<320:phase=0
    elif ms<1190:phase=1
    elif ms<2240:phase=2
    else:phase=3
    rng=random.Random(38143+phase*967)
    density=(.29,.56,.69,.54)[phase]
    for _ in range(270):
        x=rng.randrange(8,452);y=rng.randrange(8,455)
        w=rng.choice((1,1,2,2,3,5,8,12));h=rng.choice((9,16,27,40,55,77))
        # Elliptical quiet region with a gradual falloff, no rectilinear band.
        d=((x+w/2-233)/185)**2+((y+h/2-249)/92)**2
        quiet=math.exp(-1.5*d)
        if rng.random()>density*(1-.75*quiet):continue
        color=fade('PURPLE',1-.79*quiet)
        cv.rect(x,y,x+w,y+h,color)
        if w>=5:
            for cut in range(y+5,y+h,9):cv.rect(x,cut,x+w,cut+1,'BLACK')
    for x in (29,434):
        cv.rect(x,76,x+1,167,'PURPLE')
        cv.rect(x,348,x+1,403,'PURPLE')


def word_geometry():
    mask=IG.tm(TITLE,'shapiro',PX,sy=SY)
    left=round((TITLE_CX-mask.width/SS/2)*SS)
    top=round((TITLE_CY-mask.height/SS/2)*SS)
    return mask,left,top


def outlined_word(cv,ms):
    mask,left,top=word_geometry()
    edge=IG.outline(mask,w=1.55)
    bounds=[0]+[min(edge.width,IG.tm(TITLE[:i],'shapiro',PX,sy=SY).width) for i in range(1,len(TITLE))]+[edge.width]
    alpha=Image.new('L',edge.size,0)
    for i in range(len(TITLE)):
        k=G4.ease((ms-G4.OUTLINE_START-i*G4.GLYPH_DELAY)/G4.GLYPH_FADE)
        if k<=0:continue
        x0,x1=bounds[i],bounds[i+1]
        section=edge.crop((x0,0,x1,edge.height))
        if k<1:section=section.point(lambda v:round(v*k))
        alpha.paste(section,(x0,0))
    cv.img.paste(Image.new('RGB',edge.size,rgb('LIME')),(left,top),alpha)


def frame(n):
    ms=n*FRAME_MS
    cv=Canvas()
    field(cv,ms)
    if ms<FILL_AT:
        outlined_word(cv,ms)
    else:
        mask,left,top=word_geometry()
        cv.img.paste(Image.new('RGB',mask.size,rgb('LIME')),(left,top),mask)
    # The small readout appears after the word's outline resolves.
    if ms>=1950:G2.row(cv,min(99,1+int((ms-1950)/50)))
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
        ('G10 / FIRST OUTLINE',frame(20)),
        ('G10 / HALF TYPED',frame(41)),
        ('G10 / ALL OUTLINED',frame(66)),
        ('G10 / MICROTEXT BELOW',frame(69)),
        ('G10 / FILLED POP',frame(73)),
        ('G10 / HELD',frame(113)),
        ('CARD / IMPACT',LC.card(12)),
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
    storyboard(OUT/'startup-S1-G10-storyboard.png')
    identity=[frame(n) for n in range(FRAMES)]
    save_gif30(identity,str(OUT/'identity-G10-centered-large.gif'))
    for n,label in ((41,'outline'),(69,'microtext'),(113,'filled')):
        identity[n].save(OUT/f'identity-G10-{label}.png')
    save_gif30(sequence(),str(OUT/'startup-S1-G10-success.gif'))
