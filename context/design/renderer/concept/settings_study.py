"""Settings overview layout studies; render beside clock F2, compass and H2 AOD.

Run: python3 concept/settings_study.py OUTPUT_DIR
All values are the backup's design fixtures, not device telemetry.
"""
from pathlib import Path
import random
import sys

from PIL import Image, ImageDraw, ImageFont

W=466;S=2
OUT=Path(sys.argv[1]).resolve() if len(sys.argv)>1 else None
ROOT=Path(__file__).resolve().parents[1]
BASE=ROOT.parent
FONT=ROOT/'fonts'
BG=(0,0,0);WHITE=(221,225,231);MUTED=(132,142,154);FAINT=(68,81,98)
BLUE=(48,124,213);NAVY=(6,20,49);ORANGE=(246,112,18)
GLYPHS={
 'ZONE':'11011 10001 00100 10001 11011',
 'BRIGHTNESS':'00001 00011 00111 01111 11111',
 'TIMEOUT':'11111 01110 00100 01110 11111',
 'ALWAYS ON':'00000 01110 11011 01110 00000',
 'COMPASS':'01110 10001 10000 10001 01110',
 'GNSS':'00100 01010 10101 01010 00100',
 'BATTERY':'01110 11111 10001 11111 11111',
 'DEVICE':'00100 00000 01100 00100 01110',
}
PAGES=[
 [('ZONE','AUTO  IST'),('BRIGHTNESS','47%'),('TIMEOUT','1 MIN'),('ALWAYS ON','OFF')],
 [('COMPASS','CAL 54%'),('GNSS','FIX  9'),('BATTERY','USB  87%'),('DEVICE','0.1.0')],
]


def font(face,size):
    return ImageFont.truetype(str(FONT/face),size*S)


def line(d,box,fill,width=1):
    d.line(tuple(round(v*S) for v in box),fill=fill,width=max(1,width*S))


def rect(d,box,fill,outline=None,width=1):
    d.rectangle(tuple(round(v*S) for v in box),fill=fill,outline=outline,width=width*S)


def txt(d,pos,string,face,size,fill,anchor='lt'):
    d.text((round(pos[0]*S),round(pos[1]*S)),string,font=font(face,size),fill=fill,anchor=anchor,
           stroke_width=0)


def icon(d,key,x,y,module=8,pad=4,color=WHITE):
    rows=GLYPHS[key].split()
    total=5*module+2*pad
    rect(d,(x,y,x+total-1,y+total-1),BG,outline=color,width=2)
    for iy,row in enumerate(rows):
        for ix,pixel in enumerate(row):
            if pixel=='1':
                xx=x+pad+ix*module;yy=y+pad+iy*module
                rect(d,(xx,yy,xx+module-1,yy+module-1),color)


def texture(d,seed,region):
    rng=random.Random(seed)
    x0,y0,x1,y1=region
    # Scanline microblocks act as an edge register rather than a full field.
    for _ in range(36):
        x=rng.randrange(x0,x1);y=rng.randrange(y0,y1)
        w=rng.choice([4,7,13,23,34]);h=rng.choice([2,3,6,9])
        col=rng.choice([(4,13,35),(5,20,51),(8,29,64),(12,40,80)])
        rect(d,(x,y,x+w,y+h),col)
        if h>=6:line(d,(x,y+2,x+w,y+2),(17,47,93))


def fresh():
    im=Image.new('RGB',(W*S,W*S),BG);d=ImageDraw.Draw(im)
    d.ellipse((2*S,2*S,464*S,464*S),outline=MUTED,width=2*S)
    return im,d


def clipped(im):
    mask=Image.new('L',im.size,0);m=ImageDraw.Draw(mask)
    m.ellipse((2*S,2*S,464*S,464*S),fill=255)
    outside=Image.new('RGB',im.size,BG)
    outside.paste(im,(0,0),mask)
    return outside.resize((W,W),Image.Resampling.LANCZOS)


def quiet_scatter(d,page):
    """Sparse, static purple registration noise in S1's otherwise empty arcs."""
    rng=random.Random(52501+page)
    cols=[(24,10,54),(37,12,84),(55,19,116)]
    for y in range(24,450,8):
        for x in range(24,450,8):
            if (x-233)**2+(y-233)**2>219**2:continue
            if y<83 and 103<x<363:continue
            if y>370 and 112<x<354:continue
            if 83<=y<=370 and 80<=x<=386:continue
            lobe=(x<80 and 90<y<218) or (x>386 and 244<y<370)
            if rng.random()>(.43 if lobe else .09):continue
            color=rng.choice(cols)
            rect(d,(x,y,x+5,y+5),color)
            if rng.random()<.58:rect(d,(x+2,y+2,x+3,y+3),BG)


def ledger(page,scatter=False):
    im,d=fresh()
    if scatter:quiet_scatter(d,page)
    txt(d,(233,29),'SETTINGS','Shapiro.ttf',27,WHITE,'mt')
    txt(d,(233,64),('DISPLAY' if page==0 else 'DEVICE')+' / 0'+str(page+1),'MonoR.otf',12,MUTED,'mt')
    bounds=[(83,159,83,383),(160,226,57,409),(227,293,57,409),(294,370,83,383)]
    for i,((key,value),(y0,y1,x0,x1)) in enumerate(zip(PAGES[page],bounds)):
        line(d,(x0,y0,x1,y0),FAINT)
        if key=='COMPASS':rect(d,(x0,y0+8,x0+3,y1-8),ORANGE)
        else:rect(d,(x0,y0+12,x0+2,y0+27),NAVY)
        txt(d,(x0+13,y0+10),f'{page*4+i+1:02}','MonoR.otf',12,MUTED)
        txt(d,(x0+45,y0+9),key,'MonoB.otf',19,WHITE)
        txt(d,(x0+45,y0+38),value,'MonoB.otf',20,ORANGE if key=='COMPASS' else (MUTED if value=='OFF' else WHITE))
        icon(d,key,x1-47,y0+13,7,4,ORANGE if key=='COMPASS' else (BLUE if key=='GNSS' else WHITE))
    line(d,(83,370,383,370),FAINT)
    rect(d,(221,387,229,395),WHITE if page==0 else BG,outline=FAINT)
    rect(d,(237,387,245,395),WHITE if page==1 else BG,outline=FAINT)
    txt(d,(233,406),'DRAG UP TO CLOSE','MonoR.otf',12,MUTED,'mt')
    return clipped(im)


def matrix(page):
    im,d=fresh()
    txt(d,(233,35),'SETTINGS','Shapiro.ttf',26,WHITE,'mt')
    txt(d,(233,69),('DISPLAY' if page==0 else 'DEVICE')+' / 0'+str(page+1),'MonoR.otf',12,MUTED,'mt')
    boxes=[(62,90,225,222),(241,90,404,222),(62,238,225,370),(241,238,404,370)]
    for i,((key,value),(x0,y0,x1,y1)) in enumerate(zip(PAGES[page],boxes)):
        color=ORANGE if key=='COMPASS' else (BLUE if key=='GNSS' else WHITE)
        rect(d,(x0,y0,x1,y1),(10,8,9) if key=='COMPASS' else (2,8,20),outline=color if key=='COMPASS' else FAINT,width=1)
        rect(d,(x0+1,y0+1,x0+39,y0+3),color if key in ('COMPASS','GNSS') else BLUE)
        txt(d,(x1-11,y0+10),f'{page*4+i+1:02}','MonoR.otf',13,MUTED,'rt')
        icon(d,key,x0+12,y0+13,8,4,color)
        txt(d,(x0+12,y0+80),key,'MonoB.otf',15,MUTED)
        txt(d,(x0+12,y0+102),value,'MonoB.otf',18,ORANGE if key=='COMPASS' else (MUTED if value=='OFF' else WHITE))
    rect(d,(221,385,229,393),WHITE if page==0 else BG,outline=FAINT)
    rect(d,(237,385,245,393),WHITE if page==1 else BG,outline=FAINT)
    txt(d,(233,405),'DRAG UP TO CLOSE','MonoR.otf',12,MUTED,'mt')
    return clipped(im)


def board(path,ledgers,matrices):
    import sys as _sys
    _sys.path.insert(0,str(ROOT))
    import display_settings as DS
    current=DS.P.panel(e=DS.FULL8).render()
    clock=Image.open(ROOT/'concept/families-package/noise-blocks-0.png').convert('RGB') if (ROOT/'concept/families-package/noise-blocks-0.png').exists() else None
    if clock is None:
        import sys as _sys
        _sys.path.insert(0,str(ROOT))
        from concept.noise_families import still
        clock=still('blocks',0)
    # Resolve the current study beside the extracted backup; otherwise show captured UI.
    session=ROOT.parents[2]/'concept/compass-out/compass-state-sheet.png'
    if session.exists():compass=Image.open(session).convert('RGB').crop((22,22,488,488))
    else:compass=Image.open(BASE/'references/firmware-captures/2026-09-25/compass-heading.png').convert('RGB')
    aod=Image.open(ROOT/'concept/glitch-package/aod-H2-1307.png').convert('RGB')
    items=[('CLOCK / F2',clock),('COMPASS / C1',compass),('AOD / H2',aod),
           ('SETTINGS / CURRENT 8',current),('S1 / INDEX 1 OF 2',ledgers[0]),('S2 / MATRIX 1 OF 2',matrices[0])]
    margin=20;gap=16;label=36
    canvas=Image.new('RGB',(margin*2+W*3+gap*2,margin*2+2*(W+label)+gap),(25,25,28))
    d=ImageDraw.Draw(canvas)
    for i,(name,face) in enumerate(items):
        x=margin+(i%3)*(W+gap);y=margin+(i//3)*(W+label+gap)
        canvas.paste(face.resize((W,W)),(x,y));d.text((x,y+W+8),name,font=font('MonoB.otf',9),fill=WHITE)
    canvas.save(path)


def pages_board(path,ledgers,matrices):
    import sys as _sys
    _sys.path.insert(0,str(ROOT))
    import display_settings as DS
    current=[DS.P.panel(e=DS.FULL8,scroll=i).render() for i in (0,2)]
    items=[('CURRENT / START',current[0]),('S1 / PAGE 1',ledgers[0]),('S2 / PAGE 1',matrices[0]),
           ('CURRENT / END',current[1]),('S1 / PAGE 2',ledgers[1]),('S2 / PAGE 2',matrices[1])]
    margin=20;gap=16;label=36
    canvas=Image.new('RGB',(margin*2+W*3+gap*2,margin*2+2*(W+label)+gap),(25,25,28))
    d=ImageDraw.Draw(canvas)
    for i,(name,face) in enumerate(items):
        x=margin+(i%3)*(W+gap);y=margin+(i//3)*(W+label+gap)
        canvas.paste(face,(x,y));d.text((x,y+W+8),name,font=font('MonoB.otf',9),fill=WHITE)
    canvas.save(path)


if __name__=='__main__':
    OUT.mkdir(parents=True,exist_ok=True)
    aa=[ledger(i) for i in range(2)]
    bb=[matrix(i) for i in range(2)]
    for i,im in enumerate(aa):im.save(OUT/f'settings-S1-index-page-{i+1}.png')
    for i in range(2):ledger(i,scatter=True).save(OUT/f'settings-S1-sparse-scatter-page-{i+1}.png')
    for i,im in enumerate(bb):im.save(OUT/f'settings-S2-matrix-page-{i+1}.png')
    board(OUT/'screen-family-board.png',aa,bb)
    pages_board(OUT/'settings-eight-option-comparison.png',aa,bb)
