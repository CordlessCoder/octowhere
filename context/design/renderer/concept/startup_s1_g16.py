"""G16 identity: distortion entrance, framed type, reference-cadence flicker.

Run from renderer: PYTHONPATH=. python3 concept/startup_s1_g16.py OUTPUT_DIR
"""
from pathlib import Path
import math
import random
import sys

from PIL import Image, ImageDraw, ImageFont

import lib
lib.RECORD=False
from lib import Canvas, SS, rgb
from identity import finish
from anim import FRAME_MS, save_gif30
import identity_g2 as G2
import logo_card as LC
import marks_frame as MF
import startup30 as S30
from concept import startup_s1 as S1
from concept import startup_s1_g4 as G4
from concept import startup_s1_g11 as G11

OUT=Path(sys.argv[1]).resolve() if len(sys.argv)>1 else None
FRAMES=120
TYPE_START=430
GLYPH_INTERVAL=68
GLYPH_RISE=55
FLICKER_START=42
LOGO_START=56           # 14 frames after the title
GRAY=(43,46,50)
DIM_LIME=(65,91,7)
FAINT_PURPLE=(24,7,62)


def state(rel):
    """2 on, 2 outline, 1 on, 2 outline, 2 on, 2 partial, then on."""
    if rel<0:return 'pre'
    if rel<2:return 'on'
    if rel<4:return 'off'
    if rel<5:return 'on'
    if rel<7:return 'off'
    if rel<9:return 'on'
    if rel<11:return 'partial'
    return 'on'


def title_ink(cv,kind,ms=0):
    mask,edge,left,top,bounds=G11.art()
    if kind=='type':
        alpha=Image.new('L',edge.size,0)
        for i in range(9):
            k=G4.ease((ms-TYPE_START-i*GLYPH_INTERVAL)/GLYPH_RISE)
            if k<=0:continue
            x0,x1=bounds[i],bounds[i+1]
            section=edge.crop((x0,0,x1,edge.height))
            if k<1:section=section.point(lambda v:round(v*k))
            alpha.paste(section,(x0,0))
        ink=alpha;col=DIM_LIME
    elif kind=='off':ink,col=edge,DIM_LIME
    elif kind=='on':ink,col=mask,'LIME'
    else:
        # The surviving glyph stems are literal slices of the undistorted mask.
        ink=Image.new('L',mask.size,0)
        for frac in (.055,.15,.29,.42,.55,.69,.82,.95):
            x=round(mask.width*frac)
            sl=mask.crop((x,0,min(x+8,mask.width),mask.height))
            ink.paste(sl,(x,0))
        col='LIME'
    cv.img.paste(Image.new('RGB',ink.size,rgb(col)),(left,top),ink)


def scatter(cv,n):
    # Stable sparse dot clusters with slight colour breathing, far below title
    # lightness. The logo reference uses a faint purple dot field, not a fill.
    rng=random.Random(11039)
    clusters=((300,130,89,65),(180,354,95,67),(402,244,45,76))
    pulse=(.68,.82,1.0,.75)[(n//11)%4]
    for cx,cy,rx,ry in clusters:
        for _ in range(120):
            x=rng.randrange(cx-rx,cx+rx);y=rng.randrange(cy-ry,cy+ry)
            d=((x-cx)/rx)**2+((y-cy)/ry)**2
            if d>1 or rng.random()>.43*(1-d):continue
            k=pulse*rng.choice((.40,.65,1.0))
            color=tuple(round(v*k) for v in FAINT_PURPLE)
            cv.rect(x,y,x+rng.choice((2,3,4)),y+rng.choice((2,3,4)),color)


def gray_marks(cv,n):
    if n<13:return
    move=round(G4.ease((n-13)/27)*28/4)*4
    for j in range(4):
        if n>=55+j*3:continue  # remove one symmetric group per beat
        dx=89+j*30+move
        dy=53+j*19+move
        for sx in (-1,1):
            for sy in (-1,1):
                x=233+sx*dx;y=233+sy*dy
                cv.rect(x-5,y,x+5,y+1,GRAY)
                if j==3:cv.rect(x,y-3,x+1,y+4,GRAY)


def lime_ticks(cv,n):
    if n<36:return
    s=state(n-FLICKER_START)
    color='LIME' if s=='on' else DIM_LIME
    if s=='partial':color=(111,149,5)
    for x in (23,443):
        cv.rect(x-5,232,x+6,234,color)
        if s!='partial':cv.rect(x,228,x+2,238,color)


def small_logo(cv,n):
    s=state(n-LOGO_START)
    if s in ('pre','off'):return
    x0,y0,module=407,180,1.30
    for j,row in enumerate(MF.ROW['L1']):
        for i,bit in enumerate(row):
            if bit!='1' or (s=='partial' and i not in (0,7,14)):continue
            x=x0+i*module;y=y0+j*module
            cv.rect(x,y,x+module,y+module,'LIME')


def entry_impact(cv,n):
    # A few hard, diagonal triangle/scanline hits separated by black recoveries
    # distil the 0:04 cinematic transition without importing its image content.
    if n in (0,3,6,10):return
    rng=random.Random(44071+(n//2)*297)
    power={1:1.0,2:.62,4:.90,5:1.0,7:.34,8:.62,9:.43,11:.27,12:.18}.get(n,.30)
    for k in range(-2,4):
        xx=k*105+rng.randrange(-28,29)
        cv.poly([(xx,-20),(xx+22,-20),(xx+385,486),(xx+342,486)],
                tuple(round(v*power*.40) for v in rgb('LIME')))
    for y in range(24,455,10):
        for x in range(20,450,10):
            distance=abs(y-(385-.66*x))
            if distance>100 or rng.random()>(.72*(1-distance/130)*power):continue
            col=tuple(round(v*power*rng.choice((.55,.85,1.0))) for v in rgb('LIME'))
            if rng.random()<.5:cv.poly([(x,y),(x+8,y),(x,y+8)],col)
            else:cv.poly([(x+8,y),(x+8,y+8),(x,y+8)],col)
    for _ in range(24):
        x=rng.randrange(15,440);y=rng.randrange(20,440)
        if rng.random()<.5:cv.rect(x,y,x+rng.randrange(5,30),y+1,(5,26,65))
    if n in (11,12):title_ink(cv,'partial')


def frame(n):
    ms=n*FRAME_MS
    cv=Canvas()
    if n<13:
        entry_impact(cv,n)
        return finish(cv)
    scatter(cv,n)
    gray_marks(cv,n)
    if n<FLICKER_START:title_ink(cv,'type',ms)
    else:title_ink(cv,state(n-FLICKER_START))
    if n>=21:G2.row(cv,min(99,n-20))
    lime_ticks(cv,n)
    small_logo(cv,n)
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
        ('ENTRY / FIRST HIT',frame(1)),('ENTRY / BLACK',frame(3)),
        ('ENTRY / SECOND HIT',frame(5)),('OUTLINE / TYPE',frame(24)),
        ('GRAY MARKS OUT',frame(38)),('TITLE / 2 ON',frame(42)),
        ('TITLE / 2 OFF',frame(44)),('TITLE / 1 ON',frame(46)),
        ('TITLE / PARTIAL',frame(51)),('TITLE / SETTLED',frame(53)),
        ('LOGO / DELAYED',frame(57)),('LOGO / SETTLED',frame(69)),
        ('CARD / IMPACT',LC.card(12)),('CLOCK / F2',S1.clock_entry(16)),
    ]
    cell,gap,cap,margin=233,14,30,20
    sh=Image.new('RGB',(4*cell+3*gap+2*margin,4*(cell+cap)+3*gap+2*margin),(24,24,27))
    d=ImageDraw.Draw(sh);f=ImageFont.truetype('fonts/MonoB.otf',14)
    for i,(label,im) in enumerate(picks):
        x=margin+(i%4)*(cell+gap);y=margin+(i//4)*(cell+cap+gap)
        sh.paste(im.resize((cell,cell),Image.Resampling.LANCZOS),(x,y))
        d.text((x,y+cell+6),label,font=f,fill=(214,220,226))
    sh.save(path)


if __name__=='__main__':
    OUT.mkdir(parents=True,exist_ok=True)
    storyboard(OUT/'startup-S1-G16-storyboard.png')
    identity=[frame(n) for n in range(FRAMES)]
    save_gif30(identity,str(OUT/'identity-G16-framed-impact-flicker.gif'))
    for n,label in ((1,'entry'),(24,'type'),(42,'filled'),(44,'outline'),(51,'partial'),(57,'logo'),(69,'settled')):
        identity[n].save(OUT/f'identity-G16-{label}.png')
    save_gif30(sequence(),str(OUT/'startup-S1-G16-success.gif'))
