"""F3 clock study: F2 families plus brief push and two-tone compression fields.

Run from the renderer directory: PYTHONPATH=. python3 concept/noise_f3.py OUTPUT_DIR
Every image is drawn from geometry; none uses sampled cinematic pixels.
"""
import random
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw, ImageFont

try:
    from . import noise_families as F
    from . import noise_refresh as N
except ImportError:
    import noise_families as F
    import noise_refresh as N

W = 466
OUT = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else None

PALETTES = {
    'cold': [(5, 26, 61), (7, 48, 103), (11, 86, 143), (19, 122, 177)],
    'warm': [(43, 8, 24), (81, 14, 29), (117, 22, 31), (154, 34, 29)],
}


def block(d, x, y, w, h, color, rng):
    d.rectangle((x, y, x+w-1, y+h-1), fill=color)
    for yy in range(y + rng.randrange(3), y+h, 3):
        d.line((x, yy, x+w-1, yy), fill=tuple(min(255, int(c*1.30)+2) for c in color))
    if w >= 20 and rng.random() < .60:
        bx = x+rng.randrange(1, max(2,w-8))
        d.rectangle((bx, y, min(x+w-1,bx+rng.randrange(3,12)), y+rng.randrange(2,min(9,h)+1)), fill=(0,0,0))


def two_tone_background(seed, palette):
    rng = random.Random(284399 + 6553*seed)
    im = Image.new('RGB', (W,W), (0,0,0))
    d = ImageDraw.Draw(im)
    secondary = PALETTES[palette]
    # Large, irregular, clipped plates; cool blue remains the dominant field.
    centres=[]
    for i in range(12):
        x=rng.randrange(-38,430)
        y=rng.choice([rng.randrange(18,160),rng.randrange(333,438)])
        w=rng.choice([24,32,51,76,108,140]);h=rng.choice([8,14,22,36,53])
        tint=secondary[rng.choices(range(4),[3,4,2,1])[0]] if i%3==0 else N.BLUE[rng.randrange(1,5)]
        block(d,x,y,w,h,tint,rng)
        centres.append((x+w//2,y+h//2))
    for i in range(130):
        cx,cy=rng.choice(centres)
        x=cx+rng.randrange(-86,86)
        y=cy+rng.randrange(-38,38)
        w=rng.choice([3,5,8,12,18,27,42]);h=rng.choice([2,3,4,8,12,22])
        tint=secondary[rng.randrange(4)] if rng.random()<.36 else N.BLUE[rng.randrange(1,5)]
        block(d,x,y,w,h,tint,rng)
    # Adjacent two-tone half tiles look like decoding errors, not sliding patches.
    for j in range(15):
        x=rng.randrange(12,438);y=rng.choice([rng.randrange(32,175),rng.randrange(339,434)])
        size=rng.choice([10,16,24]);tint=secondary[rng.randrange(1,4)]
        d.polygon([(x,y),(x+size,y),(x,y+size)],fill=tint)
        if rng.random()<.5:
            d.polygon([(x+size,y),(x+size,y+size),(x,y+size)],fill=N.BLUE[rng.randrange(2,5)])
    return im


def push(background, scale):
    if scale == 1:
        return background
    # Zoom the *background only*, with an offset optical centre. The time stays fixed.
    nw=round(W*scale)
    large=background.resize((nw,nw),Image.Resampling.NEAREST)
    ox=round((nw-W)*.54);oy=round((nw-W)*.42)
    return large.crop((ox,oy,ox+W,oy+W))


def compose(background):
    a=np.array(background)
    a[~N.CIRC]=0
    a[N.HALO]=0
    a[N.COVER]=N.FG[N.COVER]
    return Image.fromarray(a)


def animation(palette):
    # F2 opening retained. Then a fast three-step camera push, one dark recovery,
    # and two independently reseeded two-tone fields. 125 ms per entry.
    plan=[('blocks',0,1)]*3+[('triangles',1,1)]*3+\
         [('dark',0,1)]+[('blobs',2,1)]*3+\
         [('blocks',3,1)]*3+\
         [('dual',1,1.0),('dual',1,1.13),('dual',1,1.31)]+\
         [('dark',0,1)]+[('dual',2,1)]*3+[('triangles',4,1)]*3
    cache={};frames=[]
    for family,seed,scale in plan:
        if family=='dual':
            key=('dual',palette,seed)
            if key not in cache:cache[key]=two_tone_background(seed,palette)
            frames.append(compose(push(cache[key],scale)))
        else:
            key=(family,seed)
            if key not in cache:cache[key]=F.still(family,seed)
            frames.append(cache[key])
    return plan,frames


def sheet(items, path):
    fw=466;gap=18;margin=24;cap=64
    result=Image.new('RGB',(margin*2+4*fw+3*gap,margin*2+fw+cap),(25,25,28))
    d=ImageDraw.Draw(result)
    title=ImageFont.truetype('fonts/MonoB.otf',17)
    small=ImageFont.truetype('fonts/MonoR.otf',13)
    for i,(im,label,note) in enumerate(items):
        x=margin+i*(fw+gap);result.paste(im,(x,margin))
        d.text((x,margin+fw+7),label,font=title,fill=(220,220,220))
        d.text((x,margin+fw+33),note,font=small,fill=(160,168,179))
    result.save(path)


if __name__=='__main__':
    OUT.mkdir(parents=True,exist_ok=True)
    for palette in ('cold','warm'):
        plan,frames=animation(palette)
        frames[0].save(OUT/f'clock-F3-{palette}.gif',save_all=True,
                       append_images=frames[1:]+[frames[-1]]*5,
                       duration=125,loop=0,optimize=False)
        sheet([(frames[i],f'F3 {palette.upper()} / {i*125} MS',note)
               for i,note in [(12,'F2 block field'),(14,'background push; type holds'),
                              (17,'fresh two-tone field'),(20,'triangular release')]],
              OUT/f'clock-F3-{palette}-states.png')
        print(palette,len(plan))
