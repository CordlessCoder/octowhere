"""Fault ticker and continuous-surface slide exit study. Requires the handed-off project next door.

Run: python3 fault_study.py
The firmware capture is a visual base; fixtures and colors are proposed, not device output.
"""
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

HERE=Path(__file__).resolve().parent
PROJECT=HERE.parent/'octowhere-design-project'
BASE=Image.open(PROJECT/'references/firmware-captures/2026-09-25/startup-fault.png').convert('RGB')
CLOCK=Image.open(PROJECT/'renderer/concept/family-pass-v1-out/clock-K1-gnss.png').convert('RGB')
FONT=PROJECT/'renderer/fonts/Shapiro.ttf'
MONO=PROJECT/'renderer/fonts/MonoB.otf'
BLUE=(0,29,255)    # Cinematic fault-ticker blue, used only within the red fault state.
YELLOW=(236,219,11) # Unused reference swatch, trialled here only as fault-channel ink.
RED=(239,69,33)     # Captured fault field; do not alter the actual fault-state ground.
BLACK=(0,0,0)
W=H=466
face=ImageFont.truetype(str(FONT),34)
mono=ImageFont.truetype(str(MONO),12)
mask=Image.new('L',(W,H),0)
ImageDraw.Draw(mask).ellipse((0,0,W-1,H-1),fill=255)

def stream(d,text,y,phase,color,fontsize=34,step=4):
    font=face if fontsize==34 else ImageFont.truetype(str(FONT),fontsize)
    unit=text+'  '
    width=round(d.textlength(unit,font=font))
    x=-(phase*step)%width-width
    top=y-(font.getbbox(unit,anchor='lt')[3])/2
    while x<W:
        d.text((int(x),round(top)),unit,font=font,fill=color,anchor='lt')
        x+=width

def fault_frame(n=0,failures=('MAGNET',),two_lane=False):
    im=BASE.copy();d=ImageDraw.Draw(im)
    if len(failures)>1:
        # Stable report in available top-cap area. The giant word remains the first part.
        d.rectangle((151,157,333,197),fill=RED)
        d.text((159,158),f'SELF TEST {6-len(failures)}/6 OK',font=mono,fill=BLACK)
        for i,name in enumerate(failures[:2]):
            index={'POWER':1,'CLOCK':2,'TOUCH':3,'MOTION':4,'MAGNET':5,'GNSS':6}[name]
            d.text((159,171+13*i),f'{index:02} {name} FAIL',font=mono,fill=BLACK)
        if len(failures)>2:
            d.text((298,185),f'+{len(failures)-2}',font=mono,fill=BLACK)
    if two_lane:
        d.rectangle((0,213,465,300),fill=BLACK)
        stream(d,'_'.join(failures)+'_',225,n,BLUE,fontsize=27,step=3)
        stream(d,'FAULT_FAULT_',269,n,YELLOW,fontsize=27,step=-2)
    else:
        d.rectangle((0,234,465,281),fill=BLACK)
        half=(n//12)%2
        text=('_'.join(failures)+'_')*2 if half==0 else 'FAULT_FAULT_'
        color=BLUE if half==0 else YELLOW
        # Both alternating streams advance with absolute time, including while hidden.
        stream(d,text,257,n,color,step=4)
    # The source capture uses black in the square's corners; clip added ink there too.
    clean=Image.new('RGB',(W,H),BLACK);clean.paste(im,(0,0),mask)
    return clean

def exit_layer(source):
    """One long red rectangle behind the round display aperture."""
    layer=Image.new('RGB',(W,880),BLACK)
    d=ImageDraw.Draw(layer)
    d.rectangle((0,0,465,879),fill=RED)
    # The capture's black square corners are only outside the round viewport.
    # Retain its interior art, while the underlying red fill stays rectangular.
    layer.paste(source,(0,0),mask)
    return layer

def exit_frame(n,layer):
    """The visible lower fault drops out, then the remainder eases upward."""
    moving=layer.copy()
    if n:
        # Frame 1 removes the lowest 58 px; frame 2 reaches the giant lettering.
        # This is a real disappearance of initially visible content, not a
        # shorter offscreen red plate passing through the viewport.
        cut=408 if n==1 else 338
        d=ImageDraw.Draw(moving)
        if n>=2:
            # The broad red cap also drops away in one beat, as in the reference.
            # Three large, original-content fragments survive above the ticker.
            d.rectangle((0,0,W-1,170),fill=BLACK)
            for rect in ((40,92,96,158),(176,102,280,156),(344,100,414,158)):
                moving.paste(layer.crop(rect),rect[:2])
        d.rectangle((0,cut,W-1,879),fill=BLACK)
        for x0,x1,rise in ((0,63,8),(104,159,20),(224,287,36),(352,415,24)):
            d.rectangle((x0,cut-rise,x1,879),fill=BLACK)
    u=max(0,(n-2)/15)
    yoff=-round(340*u*u)  # Slow start, then increasing travel per frame.
    out=Image.new('RGB',(W,H),BLACK)
    out.paste(moving,(0,yoff))
    round_out=Image.new('RGB',(W,H),BLACK)
    round_out.paste(out,(0,0),mask)
    return round_out

