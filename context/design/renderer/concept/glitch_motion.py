"""Glitch/compression fill study on OCTOWHERE's approved clock and AOD foregrounds.
Run from backup renderer: PYTHONPATH=. python3 concept/glitch_motion.py OUTPUT_DIR
"""
import sys, math, random
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont
import numpy as np
import clockface4 as C
import clock4 as SC
import aod as A
from lib import Canvas, SS, N

OUT=Path(sys.argv[1]).resolve() if len(sys.argv)>1 else None
if OUT is not None: OUT.mkdir(parents=True,exist_ok=True)
ORIG=SC.scatter
SEED=73
# Fixed descriptors so only their scheduled displacement changes. No procedural noise flicker.
rng=random.Random(SEED)
BLOCKS=[]
for i in range(46):
    upper=i<27
    w=rng.choice([8,12,16,24,36,54,76,92])
    h=rng.choice([6,9,13,20,32,48,68])
    x=rng.randrange(12,455)
    y=rng.randrange(7,188 if upper else 448)
    if not upper: y=max(325,y)
    size=rng.choice([0,1,2,3])
    group=rng.randrange(4)
    notch=rng.choice([0,0,0,1,2])
    BLOCKS.append((x,y,w,h,size,group,notch))

# Four broad underlying plates anchor the field; small tiles make their edges break apart.
PLATES=[(23,24,111,108,0,0),(339,37,446,156,1,1),
        (23,357,112,448,0,2),(312,375,448,460,1,3)]
PALETTE=[(4,13,34),(7,23,51),(10,31,67),(17,45,88),(21,56,104)]
LINE=[(8,23,49),(12,33,66),(17,43,84),(27,62,112),(33,74,129)]

def jump(group,frame):
    # Stepped, staggered moves: a 2 px drift, then a hard 6–12 px reposition.
    step=(frame//3)
    hops=[(0,0),(0,0),(6,-3),(6,-3),(-5,6),(-5,6),
          (8,2),(8,2),(0,0)]
    hx,hy=hops[(step+group*2)%len(hops)]
    return hx + ((frame+group)%3)*2*(1 if group%2 else -1),hy

def draw_box(d,x,y,w,h,level,frame,scanned=True,notch=0):
    if x+w<2 or x>464 or y+h<2 or y>464:return
    def r(x0,y0,x1,y1,col):
        d.rectangle((int(x0*SS),int(y0*SS),int(x1*SS)-1,int(y1*SS)-1),fill=col)
    r(x,y,x+w,y+h,PALETTE[level])
    if notch:
        # Deliberate missing corner / edge as a compression artifact.
        n=min(w//3, 6+3*notch)
        r(x+w-n,y,x+w,y+max(4,h//3),(0,0,0))
    if scanned and h>8:
        # Every fourth raster line has extra light; dark gaps vary with row and block.
        for yy in range(y+((frame//3)%4),y+h,4):
            if yy<0 or yy>=466:continue
            if (yy+y+w)%5==0:
                r(x,yy,x+w,yy+1,(1,5,15))
            else:
                r(x,yy,x+w,yy+1,LINE[level])
        # Coarse 1–2 px column drops stop the scanlines looking mechanically uniform.
        if w>34:
            for xx in range(x+15,x+w-2,22):
                r(xx,y,xx+1,y+h,(1,6,18))

def pattern(cv,frame=0,strength=1):
    layer=Image.new('RGB',(N*SS,N*SS),(0,0,0)); d=ImageDraw.Draw(layer)
    for x,y,x1,y1,level,group in PLATES:
        dx,dy=jump(group,frame)
        draw_box(d,x+dx,y+dy,x1-x,y1-y,min(4,level+strength),frame,True,(group%2)+1)
    for i,(x,y,w,h,level,group,notch) in enumerate(BLOCKS):
        if strength==0 and i%3==0:continue
        dx,dy=jump(group,frame)
        # Sometimes the block is displaced into a neighboring cell, while the rest holds.
        if i%11==0 and frame//4%2: dx+=w//2
        draw_box(d,x+dx,y+dy,w,h,min(4,level+strength-1),frame,True,notch)
        if i%5==0:
            # Tail made of smaller discontinuous cells rather than one smooth rectangle.
            for j in range(3):
                draw_box(d,x+dx+w+3+j*7,y+dy+2,4,3,min(4,level+strength),frame,False)
    mask=Image.new('L',(N*SS,N*SS),0)
    ImageDraw.Draw(mask).ellipse((5*SS,5*SS,461*SS-1,461*SS-1),fill=255)
    cv.img.paste(layer,(0,0),mask)

def clock(frame,strength):
    SC.scatter=lambda cv,*args,**kw:pattern(cv,frame,strength)
    try:return C.face('gnss')
    finally:SC.scatter=ORIG

def aod_texture(minute=7):
    fg=np.array(A.aod('local').render().convert('RGB'))
    bg=Image.new('RGB',(N,N),(0,0,0));d=ImageDraw.Draw(bg)
    # Four small plates with scanlines. AOD redraw is once per minute, so this
    # jumps by one cell only when the clock digits already change.
    for i,(x,y,w,h) in enumerate([(53,43,27,14),(379,70,30,19),(43,392,28,14),(385,412,35,16)]):
        dx=((minute+i*3)%3-1)*4
        x+=dx
        d.rectangle((x,y,x+w,y+h),fill=(8,19,36) if i%2 else (7,21,39))
        for yy in range(y+minute%3,y+h,3):
            d.line((x,yy,x+w,yy),fill=(14,32,55))
        if i%2:d.rectangle((x+w-7,y,x+w,y+5),fill=(0,0,0))
    # A few disconnected 2x2 cells, never under the numerals or date.
    for i in range(12):
        x=58+i*7 if i<6 else 357+(i-6)*8
        y=423 if i<6 else 43
        d.rectangle((x,y,x+1,y+1),fill=(18,41,67))
    a=np.array(bg);mask=np.max(fg,axis=2)>0;a[mask]=fg[mask]
    return Image.fromarray(a)

def board(items,path):
    n=len(items);gap=18;mg=24;fw=466;cap=70
    out=Image.new('RGB',(mg*2+n*fw+(n-1)*gap,mg*2+fw+cap),(25,25,28))
    d=ImageDraw.Draw(out);f=ImageFont.truetype('fonts/MonoB.otf',17);s=ImageFont.truetype('fonts/MonoR.otf',13)
    for i,(im,name,desc) in enumerate(items):
        x=mg+i*(fw+gap);out.paste(im,(x,mg));d.text((x,mg+fw+9),name,font=f,fill=(212,211,214));d.text((x,mg+fw+35),desc,font=s,fill=(165,171,178))
    out.save(path)

if __name__=='__main__':
    shots=[]
    for strength,name,desc in [(0,'G1 / QUIET','Dim plates, fewer fragments'),
                               (1,'G2 / COMPRESSION','Multiple sizes and lightness steps'),
                               (2,'G3 / HIGH SIGNAL','Brighter tiled interruptions')]:
        im=clock(3,strength);im.save(OUT/f'clock-G{strength+1}.png');shots.append((im,name,desc))
    board(shots,OUT/'clock-glitch-study.png')
    frames=[clock(frame,1) for frame in range(18)]
    frames[0].save(OUT/'clock-G2-motion.gif',save_all=True,append_images=frames[1:]+[frames[-1]]*4,
                   duration=125,loop=0,optimize=False)
    board([(frames[2],'MOTION / DRIFT','A few blocks slide 2 px per step'),
           (frames[7],'MOTION / JUMP','Grouped hard reposition'),
           (frames[13],'MOTION / RESET','Scanline phase shifts inside the blocks')],OUT/'motion-frames.png')
    a0=A.aod('local').render();a1=aod_texture(7);a2=aod_texture(8)
    board([(a0,'AOD / CURRENT','5.67% lit pixels'),
           (a1,'AOD / 13:07','Sparse scanlined fragments'),
           (a2,'AOD / 13:08','Cells jump on minute redraw')],OUT/'aod-glitch-study.png')
    for tag,im in [('aod-current',a0),('aod-1307',a1),('aod-1308',a2)]:
        im.save(OUT/(tag+'.png'));print(tag,A.lit_fraction(im))
