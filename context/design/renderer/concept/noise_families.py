"""OCTOWHERE N2 follow-up: whole-field cuts among three generated noise families.
Run from the backup renderer folder:
PYTHONPATH=. python3 concept/noise_families.py OUTPUT_DIR
"""
import sys,random,math
from pathlib import Path
from PIL import Image,ImageDraw,ImageFont
import numpy as np
try:
    from . import noise_refresh as N
except ImportError:
    import noise_refresh as N

OUT=Path(sys.argv[1]).resolve() if len(sys.argv)>1 else None
if OUT:OUT.mkdir(parents=True,exist_ok=True)
W=466

def compose(bg):
    a=np.array(bg.convert('RGB'))
    a[~N.CIRC]=0
    a[N.HALO]=0
    a[N.COVER]=N.FG[N.COVER]
    return Image.fromarray(a)

def triangles(seed):
    r=random.Random(91307+seed*677)
    im=Image.new('RGB',(W,W),(0,0,0))
    # Clusters of half-square tiles. The tile layout is independently regenerated;
    # thin 1px scanlines keep this family related to the block field.
    centres=[(r.randrange(45,420),r.choice([r.randrange(35,178),r.randrange(340,446)])) for _ in range(7)]
    cell=20
    for y in list(range(12,185,cell))+list(range(325,456,cell)):
        for x in range(13,455,cell):
            distance=min(math.hypot(x-cx,y-cy) for cx,cy in centres)
            chance=.67 if distance<85 else (.34 if distance<130 else .06)
            if r.random()>chance:continue
            level=r.choices([1,2,3,4,5],[2,3,4,3,.7])[0]
            tile=Image.new('RGB',(cell,cell),N.BLUE[level])
            td=ImageDraw.Draw(tile)
            for yy in range(r.randrange(4),cell,4):td.line((0,yy,cell-1,yy),fill=N.LINES[level])
            if r.random()<.15:td.rectangle((cell-5,0,cell-1,5),fill=(0,0,0))
            mask=Image.new('L',(cell,cell),0);md=ImageDraw.Draw(mask)
            if r.random()<.5:md.polygon([(0,0),(cell-1,0),(0,cell-1)],fill=255)
            else:md.polygon([(cell-1,0),(cell-1,cell-1),(0,cell-1)],fill=255)
            im.paste(tile,(x,y),mask)
            if r.random()<.20:
                d=ImageDraw.Draw(im);xx=x+cell+4;yy=y+cell//2
                d.line((xx-2,yy,xx+2,yy),fill=N.BLUE[3])
                d.line((xx,yy-2,xx,yy+2),fill=N.BLUE[3])
    return compose(im)

def blobs(seed):
    r=random.Random(50923+seed*1279)
    im=Image.new('RGB',(W,W),(0,0,0));d=ImageDraw.Draw(im)
    centres=[(r.randrange(50,414),r.choice([r.randrange(35,172),r.randrange(352,436)]),r.randrange(38,70)) for _ in range(4)]
    cell=8
    for y in list(range(12,189,cell))+list(range(325,454,cell)):
        for x in range(13,453,cell):
            dmin=min(math.hypot(x-cx,y-cy)/rad for cx,cy,rad in centres)
            # Low frequency outline, with missing pixels and holes; no continuous
            # interpolation or sliding between successive states.
            if dmin>1.25 or r.random()>(.68 if dmin<.65 else .38):continue
            level=r.choices([1,2,3,4,5],[1,3,4,3,.6])[0]
            w=cell if r.random()<.75 else 2*cell
            h=cell if r.random()<.82 else 2*cell
            d.rectangle((x,y,x+w-2,y+h-2),fill=N.BLUE[level])
            for yy in range(y+(seed+x)%3,y+h-1,3):
                d.line((x,yy,x+w-2,yy),fill=N.LINES[level])
            if r.random()<.10:d.rectangle((x+2,y+2,x+4,y+4),fill=(0,0,0))
    return compose(im)

def still(family,seed):
    if family=='blocks':return N.field(seed,'N2')
    if family=='triangles':return triangles(seed)
    if family=='blobs':return blobs(seed)
    if family=='dark':return compose(Image.new('RGB',(W,W),(0,0,0)))
    raise ValueError(family)

# Same sequence and duration in both loops; R inserts two 125 ms dark recoveries.
# There is no interpolation. Apart from the inserted frames, each family holds
# for around 375 ms. A fresh seed is used when a family recurs.
SEQUENCE=[('blocks',0)]*3+[('triangles',1)]*3+[('blobs',2)]*4+\
         [('blocks',3)]*3+[('triangles',4)]*4+[('blobs',5)]*3

def frames(with_recovery):
    plan=list(SEQUENCE)
    if with_recovery:
        for index in (6,13):plan[index]=('dark',0)
    cache={}
    out=[]
    for key in plan:
        if key not in cache:cache[key]=still(*key)
        out.append(cache[key])
    return plan,out

def sheet(items,path):
    fw=466;gap=18;mg=24;cap=70
    out=Image.new('RGB',(mg*2+4*fw+3*gap,mg*2+fw+cap),(25,25,28))
    d=ImageDraw.Draw(out);f=ImageFont.truetype('fonts/MonoB.otf',17);s=ImageFont.truetype('fonts/MonoR.otf',13)
    for i,(im,name,detail) in enumerate(items):
        x=mg+i*(fw+gap);out.paste(im,(x,mg));d.text((x,mg+fw+8),name,font=f,fill=(212,211,214));d.text((x,mg+fw+34),detail,font=s,fill=(164,172,180))
    out.save(path)

if __name__=='__main__':
    for name,recovery in [('F1',False),('F2',True)]:
        plan,images=frames(recovery)
        images[0].save(OUT/f'clock-{name}-families.gif',save_all=True,
                       append_images=images[1:]+[images[-1]]*5,duration=125,loop=0,optimize=False)
        print(name,[f for f,_ in plan])
    picks=[('blocks',0),('triangles',1),('blobs',2),('dark',0)]
    sheet([(still(f,s),f.upper(),('125 ms recovery' if f=='dark' else f'fresh seed {s}')) for f,s in picks],OUT/'noise-family-states.png')
    for family,seed in picks:still(family,seed).save(OUT/f'noise-{family}-{seed}.png')
