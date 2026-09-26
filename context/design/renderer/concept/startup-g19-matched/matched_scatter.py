from PIL import Image, ImageDraw, ImageSequence
import math, random
from pathlib import Path

ROOT=Path(__file__).parent
SOURCE=ROOT/'identity-G18-plume.gif'
OUT=ROOT
OUT.mkdir(exist_ok=True)
W=H=466
PURPLE=(24,7,62)
ORIGIN=(12,-2)
BAND=(197,317)
SEED=0x8c70
FIELDS=((270,145,150,-.65,.65),(190,334,125,2.55,.40))

def hash32(v):
    v=(v^ (v>>16))*0x7feb352d & 0xffffffff
    v=(v^ (v>>15))*0x846ca68b & 0xffffffff
    return (v^(v>>16))&0xffffffff

def marks(cx,cy,r,facing,density,seed):
    result={}
    for iy in range(-2,62):
        y=ORIGIN[1]+8*iy
        for ix in range(-2,62):
            x=ORIGIN[0]+8*ix
            yy=y+2 if y>317 else y
            # Exclusion applies to the full 6x6 mark, not merely its anchor.
            if yy+6>=BAND[0] and yy<=BAND[1]:continue
            if not (0<=x-3 and x+3<W and 0<=yy-3 and yy+3<H):continue
            if math.hypot(x-233,yy-233)>230:continue
            dx=x-cx;dy=y-cy;distance=math.hypot(dx,dy)
            if distance>r:continue
            radial=.45+.55*max(0,min(1,(distance-40)/180))
            turn=.45+.55*((dx*math.cos(facing)+dy*math.sin(facing))/distance if distance else 1)
            h=hash32(seed ^ hash32(ix*0x9e3779b1 & 0xffffffff) ^ hash32(iy*0x85ebca77 & 0xffffffff))
            if h/2**32 >= max(0,radial*turn*density):continue
            kind=hash32(h^0x12b591)&0xffffffff
            hollow=(kind/2**32)<.60
            result[(x,yy)]=hollow
    return result

def facing_at(n):
    # Gray marks finish by frame 64. Small facing steps occur only in the settled hold.
    if n<66:return -.65
    return -.65+.09*min(5,(n-66)//11+1)

raw=Image.open(SOURCE)
source=[(f.convert('RGB').copy(),f.info.get('duration',33)) for f in ImageSequence.Iterator(raw)]
starts=[];t=0
for _,d in source:starts.append(t);t+=d
lower=marks(*FIELDS[1],SEED+1)
frames=[];counts=[];changes=[];prev=None
for n in range(120):
    ms=n*1000/30
    i=max(j for j,s in enumerate(starts) if s<=ms)
    base=source[i][0].copy()
    p=base.load()
    # Remove the G18 concept's purple marks, preserving every other pixel.
    if n>=13:
        for y in range(H):
            for x in range(W):
                rr,g,b=p[x,y]
                if b>rr*1.6 and b>g*2 and rr<=48 and g<=18:
                    p[x,y]=(0,0,0)
    upper=marks(*FIELDS[0][:3],facing_at(n),FIELDS[0][4],SEED)
    combined=dict(lower);combined.update(upper)
    scatter=Image.new('RGB',(W,H))
    draw=ImageDraw.Draw(scatter)
    for (x,y),hollow in combined.items():
        if hollow:
            draw.rectangle((x-3,y-3,x+2,y+2),fill=PURPLE)
            draw.rectangle((x-1,y-1,x,y),fill=(0,0,0))
        else:draw.rectangle((x-2,y-2,x+1,y+1),fill=PURPLE)
    sp=scatter.load()
    if n>=13:
        for y in range(H):
            for x in range(W):
                if p[x,y]==(0,0,0) and sp[x,y]!=(0,0,0):p[x,y]=sp[x,y]
    frames.append(base)
    counts.append((len(upper),len(lower),len(combined)))
    if prev is not None: changes.append(len(set(upper)^set(prev)))
    prev=upper
frames[38].save(OUT/'outline.png')
frames[71].save(OUT/'filled.png')
frames[100].save(OUT/'settled.png')
frames[0].save(OUT/'unlock.png')
frames[0].save(OUT/'identity-matched.gif',save_all=True,append_images=frames[1:],duration=[33]*120,loop=0,optimize=True,disposal=2)
print('counts at 38,71,100:',counts[38],counts[71],counts[100])
print('max changed upper points:',max(changes),'step changes:',[(n,changes[n-1]) for n in range(1,120) if changes[n-1]>0][:10])
print('files:',*(str(p) for p in OUT.iterdir()))
