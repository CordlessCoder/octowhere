"""Power-key shutdown confirmation, using D3 settings primitives at 466px."""
from pathlib import Path
import sys
import random
import math
from PIL import Image,ImageDraw,ImageFont

HERE=Path(__file__).resolve().parent
ROOT=HERE.parent
OUT=ROOT/'references'
PROJECT=ROOT.parent/'octowhere-design-project'
sys.path.insert(0,str(PROJECT/'renderer'))
from concept import settings_study as S

BLACK=(0,0,0);WHITE=(221,225,231);MUTED=(132,142,154)
FAINT=(68,81,98);ORANGE=(246,112,18);PANEL=(18,20,23)
FONT=ImageFont.truetype(str(PROJECT/'renderer/fonts/MonoB.otf'),16)
S.GLYPHS['POWER OFF']='00100 00100 10001 10001 01110'

def rectangle(d,xy,fill,outline=None,width=1):S.rect(d,xy,fill,outline,width)
def label(d,pos,text,face,size,fill,anchor='lt'):
    S.txt(d,pos,text,face,size,fill,anchor)

def power_scatter(d):
    """Wide, thinning twin fields; no rectangular cut around the content."""
    rng=random.Random(52929)
    fields=((42,169),(424,307))
    colors=((24,10,54),(37,12,84),(55,19,116))
    for y in range(24,450,8):
        for x in range(24,450,8):
            if math.hypot(x-233,y-233)>220:continue
            if y<83 and 110<x<356:continue
            if y>370 and 105<x<361:continue
            w=max(max(0,1-math.hypot(x-cx,y-cy)/220) for cx,cy in fields)
            inner=.12+.88*min(1,abs(x-233)/190)**2
            if rng.random()>.25*w*w*inner:continue
            color=rng.choice(colors)
            rectangle(d,(x,y,x+5,y+5),color)
            if rng.random()<.58:rectangle(d,(x+2,y+2,x+3,y+3),BLACK)

def poweroff(progress=0,confirmed=False):
    """progress is the drag distance toward the 60px release target, 0..1."""
    im,d=S.fresh()
    power_scatter(d)
    label(d,(233,28),'POWER OFF','Shapiro.ttf',27,WHITE,'mt')
    label(d,(233,65),'SYSTEM / POWER','MonoR.otf',12,MUTED,'mt')
    S.line(d,(83,83,383,83),FAINT)
    if not confirmed:
        rectangle(d,(83,96,182,139),BLACK,outline=FAINT)
        label(d,(132.5,117.5),'CANCEL','MonoB.otf',15,WHITE,'mm')
    S.icon(d,'POWER OFF',330,92,6,3,ORANGE)
    S.line(d,(83,145,383,145),FAINT)
    if not confirmed:
        label(d,(91,164),'SLIDE TO POWER OFF','MonoR.otf',14,ORANGE)

    # The wide horizontal travel and release-inside-target are inherited from
    # the settings clear confirmation; a mere tap cannot reach this state.
    y0,y1=225,289
    handle_x=83+round(max(0,min(1,progress))*240)
    target=(323,y0,383,y1)
    rectangle(d,(83,256,383,258),FAINT)
    rectangle(d,target,BLACK,outline=ORANGE,width=1)
    rectangle(d,(83,y0,handle_x+60,y1),ORANGE)
    # A solid right arrow stays centered in the 60px handle at either end.
    cx,cy=handle_x+30,257
    rectangle(d,(cx-19,cy-4,cx+11,cy+4),BLACK)
    d.polygon(tuple((round(x*S.S),round(y*S.S)) for x,y in
                    ((cx+9,cy-14),(cx+23,cy),(cx+9,cy+14))),fill=BLACK)
    if progress<1 and not confirmed:
        rectangle(d,target,BLACK,outline=ORANGE,width=1)
    if not confirmed:
        label(d,(233,323),'RELEASE IN TARGET TO CONFIRM','MonoR.otf',13,MUTED,'mt')
        label(d,(233,346),'HOLD KEY TO WAKE','MonoR.otf',13,MUTED,'mt')
        S.line(d,(83,370,383,370),FAINT)
        label(d,(233,397),'AUTO CANCEL / 10 S','MonoR.otf',12,MUTED,'mt')
    else:
        label(d,(233,327),'POWERING OFF','MonoB.otf',23,WHITE,'mt')
    return S.clipped(im)

def board(items,columns,name):
    rows=(len(items)+columns-1)//columns
    sh=Image.new('RGB',(columns*486+20,rows*516+20),PANEL)
    d=ImageDraw.Draw(sh)
    for i,(caption,im) in enumerate(items):
        x=20+(i%columns)*486;y=20+(i//columns)*516
        sh.paste(im,(x,y))
        d.text((x+3,y+478),caption,font=FONT,fill=WHITE)
    sh.save(OUT/name,optimize=True)

def main():
    OUT.mkdir(exist_ok=True)
    rest=poweroff(0)
    drag=poweroff(.65)
    confirm=poweroff(1,True)
    fade=Image.blend(confirm,Image.new('RGB',(466,466),BLACK),.5)
    dark=Image.new('RGB',(466,466),BLACK)
    for name,im in [('power-off-rest',rest),('power-off-sliding',drag),
                    ('power-off-confirmed',confirm),('power-off-fade-150ms',fade)]:
        im.save(OUT/f'{name}.png',optimize=True)
    board([('REST / LONG PRESS RECEIVED',rest),('DRAG / RELEASE OUTSIDE CANCELS',drag),
           ('RELEASE IN TARGET / CONFIRMED',confirm),('FADE / 150 OF 300 MS',fade)],
          2,'power-off-state-board.png')
    prior=Image.open(PROJECT/'renderer/concept/family-pass-v1-out/settings-D1-clear.png').convert('RGB')
    board([('EXISTING CLEAR / GESTURE SOURCE',prior),('POWER OFF / PROPOSED',rest)],
          2,'power-off-family-comparison.png')
    board([('0 MS / CONFIRMED',confirm),('150 MS / HALF LEVEL',fade),
           ('300 MS / PANEL DARK',dark)],3,'power-off-fade-board.png')
    clock=Image.open(HERE/'clock-unoutlined-solid-input.png').convert('RGB')
    wake=Image.blend(dark,rest,.5)
    board([('ACTIVE / BEFORE LONG-PRESS EVENT',clock),('ACTIVE / IMMEDIATE PAGE CUT',rest),
           ('DARK OR AOD / WAKE AT 125 MS',wake),('DARK OR AOD / WAKE AT 250 MS',rest)],
          2,'power-off-entry-board.png')
    print('Rendered power-off implementation references',OUT)

if __name__=='__main__':main()
