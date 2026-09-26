"""OCTOWHERE clock: hard-reseeded scanline noise, inspired by timing observed in
first two seconds of the supplied Marathon Alpha Intro Cinematic. Original
geometric textures only; no footage or extracted reference pixels in output.
Run from backup renderer: PYTHONPATH=. python3 concept/noise_refresh.py OUTPUT_DIR
"""
import sys,random,math
from pathlib import Path
from PIL import Image,ImageDraw,ImageFilter,ImageFont
import numpy as np
import clockface4 as C
from lib import N

OUT=Path(sys.argv[1]).resolve() if len(sys.argv)>1 else None
if OUT:OUT.mkdir(parents=True,exist_ok=True)
W=466
FG=np.array(C.face('gnss',scatter_k=0).convert('RGB'))
ink=Image.fromarray(((FG.max(axis=2)>0)*255).astype('uint8'))
md=ImageDraw.Draw(ink)
# The solid band is ground even where the digits and wordmark knock out to black.
md.rectangle((0,198,465,317),fill=255)
md.rectangle((262,90,357,185),fill=255)
HALO=np.array(ink.filter(ImageFilter.MaxFilter(5)))>0
COVER=np.array(ink)>0
CIRCLE=Image.new('L',(W,W),0);ImageDraw.Draw(CIRCLE).ellipse((5,5,460,460),fill=255)
CIRC=np.array(CIRCLE)>0

BLUE=[(3,11,30),(6,18,46),(11,31,68),(17,44,91),(27,66,126),(48,101,177)]
LINES=[(5,17,40),(11,29,61),(18,42,82),(27,58,106),(43,81,147),(68,123,195)]

def block(d,x,y,w,h,level,phase,notch=0):
    if w<2 or h<2:return
    d.rectangle((x,y,x+w-1,y+h-1),fill=BLUE[level])
    if notch:
        cut=min(w//3,5+notch*3)
        if cut:
            d.rectangle((x+w-cut,y,x+w-1,y+min(h//2,4+notch*2)),fill=(0,0,0))
    if h>=7:
        for yy in range(y+phase%4,y+h,4):
            c=LINES[level] if (yy+x)//4%6 else (1,5,18)
            d.line((x,yy,x+w-1,yy),fill=c)
        if w>30:
            for j in range(x+21,x+w,27):
                d.line((j,y,j,y+h-1),fill=(1,7,20))

def field(state,variant='N1'):
    # Independent deterministic seeds per state: a new composition, not a block's next position.
    r=random.Random(76543 + state*19739 + (0 if variant=='N1' else 41))
    im=Image.new('RGB',(W,W),(0,0,0));d=ImageDraw.Draw(im)
    dense = (state%4 in (1,3))
    flash = variant=='N2' and state in (4,9)
    n_plates=(7 if dense else 5)+(1 if flash else 0)
    plate_centres=[]
    for i in range(n_plates):
        upper=(i%2==0)
        x=r.randrange(-15,410); y=r.randrange(12,150) if upper else r.randrange(333,440)
        w=r.choice([51,68,92,117,145]);h=r.choice([14,22,38,52,78])
        level=r.choices([0,1,2,3,4,5],[1,2,4,4,3,1 if flash else .25])[0]
        if flash and i==0:level=5
        block(d,x,y,w,h,level,r.randrange(4),r.randrange(3))
        plate_centres.append((x+w//2,y+h//2))
        # Offset bite and steps make a large island feel compressed, not pristine.
        if r.random()<.7:
            d.rectangle((x+r.randrange(5,18),y+h//2,x+w-r.randrange(4,18),y+h//2+r.randrange(3,9)),fill=(0,0,0))
    n_frag=(100 if dense else 65)+(22 if flash else 0)
    for i in range(n_frag):
        if plate_centres and r.random()<.72:
            cx,cy=r.choice(plate_centres); x=cx+r.randrange(-80,80);y=cy+r.randrange(-45,45)
        else:
            x=r.randrange(7,450);y=r.randrange(9,174) if i%2 else r.randrange(332,459)
        w=r.choice([2,3,4,6,8,12,16,24,37,55]);h=r.choice([2,3,4,6,8,12,16,24])
        level=r.choice([1,1,2,2,3,4] if dense else [0,1,1,2,3])
        if flash and i%5==0:level=5
        block(d,x,y,w,h,level,r.randrange(4),r.choice([0,0,1]))
    # One sparse signal shelf, with holes, can cut across a plate on an active state.
    if dense:
        yy=r.choice([57,143,368,431]);xx=r.randrange(20,170)
        for k in range(8):
            if r.random()<.18:continue
            block(d,xx+k*26,yy+r.randrange(-3,4),r.randrange(6,23),r.choice([2,3,5]),r.choice([2,3,4]),r.randrange(4))
    data=np.array(im);data[~CIRC]=0;data[HALO]=0;data[COVER]=FG[COVER]
    return Image.fromarray(data)


def animation(variant):
    if variant=='N1':
        # A wholly new field every 250 ms, held for two 125 ms frames.
        states=[i//2 for i in range(16)]
    else:
        # 375 ms holds; a single-frame denser hit and a near-blank recovery.
        states=[0,0,0,1,1,1,2,2,2,4,6,6,7,7,7,9,8,8]
    frames=[field(s,variant) for s in states]
    return states,frames

def sheet(picks,path,variant):
    fw=466;gap=18;mg=24;cap=70
    out=Image.new('RGB',(mg*2+4*fw+3*gap,mg*2+fw+cap),(25,25,28))
    d=ImageDraw.Draw(out);f=ImageFont.truetype('fonts/MonoB.otf',17);s=ImageFont.truetype('fonts/MonoR.otf',13)
    for i,(state,im,desc) in enumerate(picks):
        x=mg+i*(fw+gap);out.paste(im,(x,mg));d.text((x,mg+fw+8),f'{variant} / STATE {state}',font=f,fill=(212,211,214));d.text((x,mg+fw+33),desc,font=s,fill=(164,172,180))
    out.save(path)

if __name__=='__main__':
    for variant in ('N1','N2'):
        states,frames=animation(variant)
        frames[0].save(OUT/f'clock-{variant}-reseed.gif',save_all=True,
                       append_images=frames[1:]+[frames[-1]]*5,duration=125,loop=0,optimize=False)
        indices=[0,3,7,11] if variant=='N1' else [0,6,9,15]
        picks=[(states[i],frames[i],f'{i*125} ms, fresh field' if i else '0 ms, held field') for i in indices]
        sheet(picks,OUT/f'clock-{variant}-states.png',variant)
        for i in indices:frames[i].save(OUT/f'clock-{variant}-frame-{i:02d}.png')
        print(variant,states)
