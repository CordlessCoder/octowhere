"""Fault ticker and continuous-surface slide exit study. Requires the handed-off project next door.

Run: python3 fault_study.py
The firmware capture is a visual base; fixtures and colors are proposed, not device output.
"""
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

HERE=Path(__file__).resolve().parent
PROJECT=HERE.parent.parent/'octowhere-design-project'
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

single=[fault_frame(n) for n in range(120)]
multi=[fault_frame(n,('MAGNET','GNSS')) for n in range(120)]
# A standalone exit study follows the full 4-second red hold. The final clock is a fixture.
exit_stock=exit_layer(single[-1])
exit_frames=[exit_frame(n,exit_stock) for n in range(18)]
for name,frames in [('fault-alternating-single.gif',single),('fault-alternating-two-failures.gif',multi),
                    ('fault-damaged-slide-exit.gif',exit_frames+[CLOCK])]:
    # GIF's clock is 10 ms, so 30/30/40 gives exactly 30 fps over each 3-frame group.
    durations=[40 if i%3==2 else 30 for i in range(len(frames))]
    if len(frames)==19:durations[-1]=600  # Hold the destination fixture for review.
    frames[0].save(HERE/name,save_all=True,append_images=frames[1:],duration=durations,loop=0,optimize=True,disposal=2)
single[3].save(HERE/'fault-blue-name.png')
single[15].save(HERE/'fault-yellow-fault.png')
multi[3].save(HERE/'fault-two-failures.png')
fault_frame(3,('MAGNET','GNSS'),True).save(HERE/'fault-two-lane-comparison.png')
for n in (0,1,2,3,5,8,11,14,17):exit_frames[n].save(HERE/f'exit-frame-{n:02}.png')
exit_stock.save(HERE/'exit-overscan-layer.png')

# Same 466-px source scale within a review board, with clear concept labels.
items=[('CURRENT / FIRMWARE',BASE),('A / BLUE NAME',single[3]),('A / YELLOW FAULT',single[15]),
       ('B / TWO FAILURES',multi[3]),('C / TWO LANES',fault_frame(3,('MAGNET','GNSS'),True)),
       ('EXIT / 300 MS',exit_frames[8])]
from PIL import ImageFont
lab=ImageFont.truetype(str(MONO),18)
sheet=Image.new('RGB',(3*486+40,2*526+80),(18,20,23));sd=ImageDraw.Draw(sheet)
for i,(label,im) in enumerate(items):
    x=20+(i%3)*486;y=20+(i//3)*526
    sheet.paste(im,(x,y));sd.text((x+5,y+475),label,font=lab,fill=(215,218,222))
sheet.save(HERE/'fault-study-board.png',optimize=True)
timeline=(0,1,2,3,5,8,11,14,17)
sequence=Image.new('RGB',(3*486+40,3*514+20),(18,20,23));seq_draw=ImageDraw.Draw(sequence)
for i,n in enumerate(timeline):
    x=20+(i%3)*486;y=20+(i//3)*514
    sequence.paste(exit_frames[n],(x,y))
    seq_draw.text((x+6,y+475),f'{n+1:02}/18  {round((n+1)*1000/30)} MS',font=lab,fill=(215,218,222))
sequence.save(HERE/'exit-sequence-board.png',optimize=True)
progress=Image.new('RGB',(4*486+20,2*119+20),(18,20,23));pd=ImageDraw.Draw(progress)
for i,n in enumerate((0,11,24,35,12,23,36,47)):
    x=20+(i%4)*486;y=20+(i//4)*119
    progress.paste(single[n].crop((0,220,466,298)),(x,y))
    pd.text((x+4,y+83),f'{"BLUE" if i<4 else "YELLOW"} / FRAME {n:02}',font=lab,fill=(215,218,222))
progress.save(HERE/'ticker-continuity-board.png',optimize=True)
print('rendered',len(list(HERE.glob('*.png'))),'PNGs and 3 GIFs')
