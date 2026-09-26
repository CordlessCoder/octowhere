"""Compass texture and entry studies from the 2026-09-25 firmware captures.

Run from renderer: PYTHONPATH=. python3 concept/compass_noise.py OUTPUT_DIR
The drawn UI is taken from the captured compass states. All noise is original geometry.
"""
from pathlib import Path
import math
import random
import sys

import numpy as np
from PIL import Image, ImageDraw, ImageFilter, ImageFont

W=466
OUT=Path(sys.argv[1]).resolve() if len(sys.argv)>1 else None
CAP=Path(__file__).resolve().parents[2]/'references/firmware-captures/2026-09-25'
STATES={s:Image.open(CAP/f'compass-{s}.png').convert('RGB') for s in
        ('heading','calibrating','interference','no-data','top-edge-up')}
BLUE=[(4,11,29),(6,19,49),(9,31,72),(15,47,103),(25,66,140),(37,88,170)]


def blocks(seed):
    rng=random.Random(34383+seed*2081)
    im=Image.new('RGB',(W,W));d=ImageDraw.Draw(im)
    centres=[]
    for i in range(15):
        x=rng.randrange(-20,440)
        y=rng.choice([rng.randrange(20,115),rng.randrange(340,444),rng.randrange(145,329)])
        w=rng.choice([26,43,66,91,122]);h=rng.choice([8,13,21,34,46]);lev=rng.randrange(1,5)
        plate(d,x,y,w,h,BLUE[lev],rng)
        centres.append((x+w//2,y+h//2))
    for i in range(110):
        cx,cy=rng.choice(centres);x=cx+rng.randrange(-66,67);y=cy+rng.randrange(-42,43)
        w=rng.choice([2,4,6,9,14,22,32]);h=rng.choice([2,3,4,7,12])
        plate(d,x,y,w,h,BLUE[rng.randrange(1,5)],rng)
    return im


def plate(d,x,y,w,h,col,rng):
    d.rectangle((x,y,x+w-1,y+h-1),fill=col)
    if h>4:
        for yy in range(y+rng.randrange(3),y+h,3):
            d.line((x,yy,x+w-1,yy),fill=tuple(min(255,int(v*1.30)+2) for v in col))
    if w>25 and rng.random()<.6:
        xx=x+rng.randrange(5,w-6)
        d.rectangle((xx,y,xx+rng.randrange(3,9),y+rng.randrange(2,min(8,h)+1)),fill=(0,0,0))


def triangles(seed):
    rng=random.Random(44003+seed*481)
    im=Image.new('RGB',(W,W));d=ImageDraw.Draw(im)
    for y in range(14,460,21):
        for x in range(14,460,21):
            # Broken clusters, with large black gaps; no shifting of a prior frame.
            radial=math.hypot(x-233,y-233)
            if rng.random()>(.46 if radial>144 else .16):continue
            lev=rng.choice([1,2,2,3,4])
            if rng.random()<.5:points=[(x,y),(x+19,y),(x,y+19)]
            else:points=[(x+19,y),(x+19,y+19),(x,y+19)]
            d.polygon(points,fill=BLUE[lev])
            if rng.random()<.1:d.point((x+10,y+10),fill=BLUE[5])
    return im


def blobs(seed):
    rng=random.Random(74779+seed*659)
    im=Image.new('RGB',(W,W));d=ImageDraw.Draw(im)
    centres=[(rng.randrange(35,430),rng.choice([rng.randrange(45,124),rng.randrange(335,438)]),rng.randrange(38,84)) for _ in range(9)]
    for y in range(13,462,8):
        for x in range(13,462,8):
            near=min(math.hypot(x-cx,y-cy)/rad for cx,cy,rad in centres)
            if near>1.3 or rng.random()>(.62 if near<.7 else .30):continue
            lev=rng.choice([1,2,3,3,4]);w=rng.choice([6,6,14,22]);h=rng.choice([6,6,14])
            plate(d,x,y,w,h,BLUE[lev],rng)
    return im


def radial_accent(background,progress):
    if progress<=0:return background
    im=background.copy();d=ImageDraw.Draw(im)
    # Transient, data-free registration ring. It never replaces the heading dial.
    extent=round(320*progress)
    for angle in range(-90,-90+extent,12):
        color=(33,91,178) if angle%36==0 else (12,43,103)
        d.arc((43,43,423,423),angle,angle+6,fill=color,width=3)
    d.arc((79,79,387,387),-90,-90+round(240*progress),fill=(12,37,83),width=2)
    return im


def compose(background,state='heading',strength=1.0):
    base=STATES[state]
    ink=np.array(base)
    mask=Image.fromarray(((ink.max(axis=2)>6)*255).astype('uint8')).filter(ImageFilter.MaxFilter(5))
    md=ImageDraw.Draw(mask)
    md.rectangle((135,204,331,295),fill=255)   # Preserve black knockouts in the readout slab.
    md.rectangle((199,86,267,154),fill=255)    # Preserve black cells inside the icon.
    a=np.array(background).astype('float32')*strength
    yy,xx=np.ogrid[:W,:W]
    valid=(xx-233)**2+(yy-233)**2<=229**2
    # Centre stack stays on black; detail concentrates in four outer sectors.
    core=(xx>115)&(xx<351)&(yy>80)&(yy<344)
    a[core]*=.27
    a[~valid]=0
    a[np.array(mask)>0]=ink[np.array(mask)>0]
    return Image.fromarray(np.uint8(a))


def timeline(with_ring):
    families=[('blocks',0)]*3+[('dark',0)]+[('triangles',1)]*3+\
             [('blobs',2)]*3+[('blocks',3)]*3+[('settled',4)]*10
    cache={};frames=[]
    for i,(family,seed) in enumerate(families):
        if family=='dark':b=Image.new('RGB',(W,W))
        elif family=='settled':
            b=blocks(seed)
        else:
            key=(family,seed)
            if key not in cache:cache[key]={'blocks':blocks,'triangles':triangles,'blobs':blobs}[family](seed)
            b=cache[key]
        if with_ring and 9<=i<=15:
            b=radial_accent(b,(i-8)/7)
        # Once the transition settles, the background stops refreshing and dims.
        frames.append(compose(b,strength=.62 if family=='settled' else 1.0))
    return frames


def state_sheet(path):
    items=[('HEADING / SETTLED',compose(blocks(4),'heading',.62)),
           ('CALIBRATING / QUIET',compose(triangles(8),'calibrating',.47)),
           ('INTERFERENCE / DIM',compose(blocks(5),'interference',.26)),
           ('NO DATA / CLEAR',compose(Image.new('RGB',(W,W)),'no-data'))]
    margin=22;gap=14;cap=60
    out=Image.new('RGB',(margin*2+W*4+gap*3,margin*2+W+cap),(25,25,28))
    d=ImageDraw.Draw(out);font=ImageFont.truetype('fonts/MonoB.otf',18)
    for i,(name,im) in enumerate(items):
        x=margin+i*(W+gap);out.paste(im,(x,margin))
        d.text((x,margin+W+13),name,font=font,fill=(216,220,223))
    out.save(path)


if __name__=='__main__':
    OUT.mkdir(parents=True,exist_ok=True)
    for label,ring in [('C1-noise',False),('C2-circular-entry',True)]:
        frames=timeline(ring)
        frames[0].save(OUT/f'compass-{label}.gif',save_all=True,append_images=frames[1:],
                       duration=125,loop=0,optimize=False)
        for i in [0,4,7,11,15,17]:frames[i].save(OUT/f'{label}-frame-{i:02}.png')
    state_sheet(OUT/'compass-state-sheet.png')
