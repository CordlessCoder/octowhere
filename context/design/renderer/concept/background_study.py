"""OCTOWHERE clock/AOD background study. Run from backup's renderer folder.

Output path is supplied as the first argument. Uses the exact round 4 clock and AOD
foregrounds; only the field below the foreground changes. Concept render, not firmware.
"""
import sys, math
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont
import numpy as np
import clockface4 as C
import clock4 as SC
import aod as A
from lib import Canvas, SS, N

OUT = Path(sys.argv[1]).resolve()
OUT.mkdir(parents=True, exist_ok=True)
ORIG_SCATTER = SC.scatter


def field(cv, style):
    layer = Image.new('RGB', (N*SS, N*SS), (0,0,0))
    d = ImageDraw.Draw(layer)
    def rect(x0,y0,x1,y1,c):
        d.rectangle((x0*SS,y0*SS,x1*SS-1,y1*SS-1), fill=c)
    if style == 'A': # broad dim lime blocks, narrow datum strokes
        blocks = [(9,37,46,182),(48,18,110,71),(113,35,153,87),
                  (227,17,275,64),(286,44,330,78),(349,24,429,85),
                  (13,339,54,404),(63,399,122,447),(126,407,164,457),
                  (302,399,353,456),(365,351,425,393),(416,393,457,449)]
        for j,(x0,y0,x1,y1) in enumerate(blocks):
            rect(x0,y0,x1,y1, (9,26,3) if j%3 else (13,34,4))
            if j%2 == 0:
                for x in range(x0+5,x1-2,12): rect(x,y0,x+1,y1,(17,43,5))
        for x,y,n in [(42,108,9),(106,47,6),(328,44,5),(390,111,7),
                      (35,389,5),(139,424,4),(358,416,6)]:
            for i in range(n): rect(x+i*3,y,x+i*3+1,y+8,(25,49,6))
    elif style == 'B': # dim blue signal traces, broken vertical columns
        for x0,y0,x1,y1 in [(22,39,55,171),(116,18,172,83),(221,12,258,67),
                            (365,34,416,104),(402,131,450,187),(23,339,58,391),
                            (86,407,138,455),(197,414,244,460),(339,381,386,448)]:
            rect(x0,y0,x1,y1,(4,12,34))
        for i,x in enumerate(range(16,457,23)):
            length = 35 + (i*19)%54
            for y0,y1 in [(28,28+length),(421-length,421)]:
                if (i*7 + y0)%5 == 0: continue
                rect(x,y0,x+2,y1,(5,13,37))
                if i%3 == 0: rect(x+4,y0+7,x+7,min(y0+25,y1),(7,19,48))
        for x,y,w in [(13,142,32),(117,21,34),(317,76,39),(408,169,22),
                      (13,366,28),(55,423,52),(283,431,38),(397,370,41)]:
            rect(x,y,x+w,y+6,(8,21,58)); rect(x+3,y+8,x+w-7,y+10,(5,14,37))
        for x,y in [(72,42),(170,32),(333,117),(58,415),(342,407)]:
            for k in range(7): rect(x+4*k,y,x+4*k+2,y+3,(12,29,72))
    elif style == 'C': # clock-specific 24 hour ledger, active fixture hour
        for h in range(24):
            x = 43 + h*16
            col = (41,60,5) if h==C.F['h'] else (13,26,5)
            rect(x,27,x+9,32,col)
            rect(x,35,x+1,45,(22,35,6))
            # a second, segmented row makes the time register legible around the rim
            if h%3 == 0: rect(x,428,x+9,431,(22,36,5))
        rect(43+C.F['h']*16,23,43+C.F['h']*16+9,25,(53,77,7))
        for x0,y0,w in [(14,102,36),(36,143,18),(383,108,57),
                        (21,363,51),(112,406,22),(345,383,52)]:
            rect(x0,y0,x0+w,y0+16,(9,24,4))
            rect(x0,y0+19,x0+w-9,y0+20,(19,39,5))
    else: raise ValueError(style)
    mask = Image.new('L',(N*SS,N*SS),0)
    ImageDraw.Draw(mask).ellipse((5*SS,5*SS,461*SS-1,461*SS-1), fill=255)
    cv.img.paste(layer,(0,0),mask)


def clock(style):
    SC.scatter = lambda cv,*args,**kwargs: field(cv,style)
    try: return C.face('gnss')
    finally: SC.scatter = ORIG_SCATTER


def aod_variant(style):
    fg = A.aod('local').render().convert('RGB')
    bg = Image.new('RGB', fg.size, (0,0,0))
    d = ImageDraw.Draw(bg)
    if style=='T': # peripheral trace, dotted corners and interruptions
        col=(28,39,5)
        for x0,y0 in [(63,58),(371,70),(46,388),(366,399)]:
            d.rectangle((x0,y0,x0+22,y0+1), fill=col)
            d.rectangle((x0,y0,x0+1,y0+11), fill=col)
        for x in range(52,148,14): d.rectangle((x,415,x+3,416), fill=(22,31,5))
        for x in range(335,411,14): d.rectangle((x,415,x+3,416), fill=(22,31,5))
    elif style=='H': # 24 hours, active hour 13, no time-independent animation
        for h in range(24):
            x=64+h*14
            d.rectangle((x,402,x+4,403), fill=(14,26,47))
        x=64+C.F['h']*14
        d.rectangle((x,398,x+4,404), fill=(54,76,6))
        d.rectangle((102,52,146,53),fill=(13,25,43))
        d.rectangle((320,52,363,53),fill=(13,25,43))
    else: raise ValueError(style)
    out=np.array(bg); top=np.array(fg)
    out[np.max(top,axis=2)>0] = top[np.max(top,axis=2)>0]
    return Image.fromarray(out)


def board(items, path, cols=3):
    fw,fh=466,466
    gap,margin,caption=18,24,70
    rows=math.ceil(len(items)/cols)
    dst=Image.new('RGB',(margin*2+cols*fw+(cols-1)*gap,margin*2+rows*(fh+caption)+(rows-1)*gap),(25,25,28))
    d=ImageDraw.Draw(dst); f=ImageFont.truetype('fonts/MonoB.otf',17); s=ImageFont.truetype('fonts/MonoR.otf',13)
    for i,(im,title,sub) in enumerate(items):
        x=margin+(i%cols)*(fw+gap); y=margin+(i//cols)*(fh+caption+gap)
        dst.paste(im,(x,y)); d.text((x,y+fh+9),title,font=f,fill=(212,211,214));d.text((x,y+fh+34),sub,font=s,fill=(165,171,178))
    dst.save(path)

if __name__=='__main__':
    clock_items=[]
    for k,title,sub in [
        ('A','A / DIM MODULAR BLOCKS','Lime shadow, cut columns and ticks'),
        ('B','B / BROKEN SIGNAL TRACES','Blue interruptions and short cells'),
        ('C','C / 24 HOUR REGISTER','24 cells; hour 13 marked, static at rest')]:
        im=clock(k);im.save(OUT/f'clock-{k}.png');clock_items.append((im,title,sub))
    board(clock_items,OUT/'clock-study.png')
    aod_items=[]
    for k,title,sub in [
        ('0','CURRENT / HOLLOW FACE','Control: no background'),
        ('T','T / PERIPHERAL TRACE','Four tiny corner markers, sparse ticks'),
        ('H','H / HOUR REGISTER','Dim blue cells; active hour changes hourly')]:
        im=A.aod('local').render() if k=='0' else aod_variant(k)
        im.save(OUT/f'aod-{k}.png')
        lit,lum=A.lit_fraction(im)
        aod_items.append((im,title,f'{sub}  /  {lit*100:.2f}% lit'))
        print(k,round(lit*100,3),round(lum*100,3))
    board(aod_items,OUT/'aod-study.png')
