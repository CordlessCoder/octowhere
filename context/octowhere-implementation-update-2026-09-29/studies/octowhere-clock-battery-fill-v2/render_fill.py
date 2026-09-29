"""Apply the solid battery fill and segmented charging treatment to K1.

This is a review compositor. It preserves the selected K1 state stills except
for the battery column and its microcopy, and does not edit firmware or the
approved handoff project. All percentages are fixtures.
"""
from pathlib import Path
import sys
import os
from math import floor
from functools import lru_cache
from PIL import Image, ImageDraw, ImageFont

HERE=Path(__file__).resolve().parent
PROJECT=HERE.parent/'octowhere-design-project'
K1=PROJECT/'renderer/concept/family-pass-v1-out'
sys.path.insert(0,str(PROJECT/'renderer'))
os.chdir(PROJECT/'renderer')  # the handed-off renderer resolves fonts relatively
import clockface4 as CK

LIME=(192,254,4); WHITE=(210,211,214); ORANGE=(241,113,13)
GRAY=(136,142,152); BLACK=(0,0,0); PANEL=(18,20,23)
INK=(215,218,222)
STATES=('gnss','rtc','manual','stopped','nozone','nodata')
LABELS={'gnss':'GNSS LOCAL','rtc':'RTC LOCAL','manual':'MANUAL LOCAL',
        'stopped':'STOPPED','nozone':'NO ZONE','nodata':'NO DATA'}
HEIGHTS=(2,3,2,10,7,3,11,2,9,4,8)
SCATTER=(5,1,2,5,1,4,1,4,1,1)
ASSEMBLED=(1,1,5,2,4,1,5,1,3,2)
PAIRED=(1,3,6,1,3,2,1,5,1,2)
BUILD_ORDER=(0,4,2,8,1,6,9,3,7,5)
PAIR_ORDER=(8,0,5,2,9,4,1,7,3,6)

def ease(t):
    t=max(0,min(1,t));return t*t*(3-2*t)

def gaps_between(a,b,t,order=BUILD_ORDER):
    values=[start+(end-start)*ease((t-.25*order[i]/9)/.75)
            for i,(start,end) in enumerate(zip(a,b))]
    values=[v*25/sum(values) for v in values]
    ints=[floor(v) for v in values]
    for i in sorted(range(10),key=lambda i:values[i]-ints[i],reverse=True)[:25-sum(ints)]:
        ints[i]+=1
    return tuple(ints)

def gaps(phase):
    n=phase%72
    if n<18:return gaps_between(SCATTER,ASSEMBLED,n/17)
    if n<40:return ASSEMBLED
    if n<54:return gaps_between(ASSEMBLED,PAIRED,(n-40)/13,PAIR_ORDER)
    if n<62:return PAIRED
    return gaps_between(PAIRED,SCATTER,(n-62)/9,PAIR_ORDER)

def scaled(weights,total,minimum=1):
    """Integer apportionment with a visible minimum for each band/gap."""
    free=total-minimum*len(weights)
    assert free>=0
    excess=[max(0,w-minimum) for w in weights]
    basis=excess if sum(excess)>0 else weights
    fractions=[free*w/sum(basis) for w in basis]
    result=[minimum+floor(v) for v in fractions]
    for i in sorted(range(len(weights)),key=lambda i:fractions[i]-floor(fractions[i]),reverse=True)[:total-sum(result)]:
        result[i]+=1
    assert sum(result)==total
    return tuple(result)

def resample(weights,count):
    """Interpolate the V6 uneven grammar to any natural segment count."""
    if count==1:return (weights[len(weights)//2],)
    out=[]
    for i in range(count):
        pos=i*(len(weights)-1)/(count-1)
        j=floor(pos);t=pos-j
        out.append(weights[j]*(1-t)+weights[min(j+1,len(weights)-1)]*t)
    return tuple(out)

def pattern_for_height(h,phase):
    if h<=0:return (),()
    # At the 87% fixture: h=86 -> 11 V6 bands. All other levels use the
    # same nominal ~7.8 px pitch, rather than jumping between hand-picked
    # thresholds. Integer rounding is unavoidable on this pixel display.
    count=max(1,min(13,floor(h*11/86+.5)))
    if count==1:return (h,),()
    gap_total=max(count-1,min(h-count,round(h*25/86)))
    return (scaled(resample(HEIGHTS,count),h-gap_total),
            scaled(resample(gaps(phase),count-1),gap_total))

def gauge_color(state,pct):
    if pct is None:return GRAY
    if pct<=15:return ORANGE
    if state in ('gnss','rtc','manual'):return LIME
    if state=='stopped':return ORANGE  # retain selected K1 color
    return WHITE

@lru_cache(maxsize=24)
def line_crop(state,pct,charging):
    f=dict(CK.F,bat=pct,charging=charging)
    source=CK.face(state if state!='nodata' else 'gnss',f=f,scatter_k=0).convert('RGB')
    box=(262,254,363,268)
    return source.crop(box)

def insert_text(im,state,pct,charging):
    box=(262,254,363,268)
    crop=line_crop(state,pct,charging)
    if state!='nodata':
        im.paste(crop,box[:2])
    else:
        # Optional earlier proposal: K1 currently omits band microcopy in
        # NO DATA. The default in this study preserves that selected layout.
        alpha=Image.new('L',crop.size)
        alpha.putdata([max(0,min(255,round((254-crop.getpixel((x,y))[1])*255/254)))
                       for y in range(crop.height) for x in range(crop.width)])
        im.paste(BLACK,box[:2]+(box[0]+crop.width,box[1]+crop.height),alpha)

def compose(state,pct=87,charging=True,phase=27,show_nodata_text=False):
    im=Image.open(K1/f'clock-K1-{state}.png').convert('RGB')
    if state!='nodata' or show_nodata_text:
        insert_text(im,state,pct,charging)
    d=ImageDraw.Draw(im)
    color=gauge_color(state,pct)
    d.rectangle((394,206,439,310),fill=BLACK)
    d.rectangle((395,207,438,309),outline=color,width=1)
    if pct is None:
        # No height can be inferred. The dashes are intentionally static.
        d.rectangle((402,255,412,257),fill=GRAY)
        d.rectangle((422,255,432,257),fill=GRAY)
        return im
    h=round(99*max(0,min(100,pct))/100)
    if h==0:return im
    top=308-h
    if not charging:
        d.rectangle((397,top,436,307),fill=color)
        return im
    if state=='nodata':phase=27  # keep the genuine clock fault quiet
    heights,spaces=pattern_for_height(h,phase)
    y=top
    for i,bar_h in enumerate(heights):
        d.rectangle((397,y,436,y+bar_h-1),fill=color)
        y+=bar_h+(spaces[i] if i<len(spaces) else 0)
    assert y==308
    return im

FONT=PROJECT/'renderer/fonts/MonoB.otf'
font=ImageFont.truetype(str(FONT),17)
small=ImageFont.truetype(str(FONT),14)
def board(items,columns,path):
    rows=(len(items)+columns-1)//columns
    out=Image.new('RGB',(columns*486+20,rows*516+20),PANEL)
    d=ImageDraw.Draw(out)
    for i,(label,im) in enumerate(items):
        x=20+(i%columns)*486;y=20+(i//columns)*516
        out.paste(im,(x,y))
        d.text((x+4,y+480),label,font=font,fill=INK)
    out.save(HERE/path,optimize=True)

def main():
    board([(LABELS[st]+' / 87% CHG',compose(st)) for st in STATES],3,'clock-all-states-charging.png')
    board([(LABELS[st]+' / 87% SOLID',compose(st,charging=False)) for st in STATES],
          3,'clock-solid-all-states.png')
    board([
        *[(LABELS[st]+' / SOLID',compose(st,charging=False)) for st in STATES[:3]],
        *[(LABELS[st]+' / CHARGING',compose(st)) for st in STATES[:3]],
        *[(LABELS[st]+' / SOLID',compose(st,charging=False)) for st in STATES[3:]],
        *[(LABELS[st]+' / CHARGING',compose(st)) for st in STATES[3:]],
    ],3,'clock-solid-charging-pairs.png')
    board([
        ('LOCAL / 12% SOLID',compose('gnss',pct=12,charging=False)),
        ('STOPPED / 12% SOLID',compose('stopped',pct=12,charging=False)),
        ('NO DATA / 12% SOLID',compose('nodata',pct=12,charging=False)),
        ('LOCAL / BAT --',compose('gnss',pct=None,charging=False)),
        ('NO ZONE / BAT --',compose('nozone',pct=None,charging=False)),
        ('NO DATA / UNKNOWN',compose('nodata',pct=None,charging=False)),
    ],3,'clock-solid-edge-states.png')
    board([
        ('LOCAL / 87% SOLID',compose('gnss',charging=False)),
        ('LOCAL / 12% CHG',compose('gnss',pct=12,phase=58)),
        ('LOCAL / BAT -- CHG',compose('gnss',pct=None)),
        ('NO DATA / 87% CHG STATIC',compose('nodata',phase=58)),
    ],2,'clock-battery-edge-states.png')
    board([
        ('STOPPED / 12% CHG',compose('stopped',pct=12,phase=58)),
        ('NO ZONE / 12% CHG',compose('nozone',pct=12,phase=58)),
        ('NO DATA / 12% CHG',compose('nodata',pct=12,phase=58)),
        ('NO DATA / UNKNOWN CHG',compose('nodata',pct=None)),
        ('LOCAL / 0% CHG',compose('gnss',pct=0)),
        ('LOCAL / 100% CHG',compose('gnss',pct=100,phase=58)),
    ],3,'clock-cross-state-edges.png')
    levels=(5,12,20,35,50,70,87,100)
    board([(f'{pct}% / {len(pattern_for_height(round(99*pct/100),27)[0]):02d} BANDS',
            compose('gnss',pct=pct,phase=27)) for pct in levels],4,
          'clock-level-scaling.png')

    crop=(389,199,445,316)
    tiles=(('gnss',87,'GNSS LOCAL'),('stopped',87,'STOPPED'),
           ('nozone',87,'NO ZONE'),('nodata',87,'NO DATA'),
           ('gnss',12,'LOW / 12%'),('gnss',None,'UNKNOWN'))
    frames=[]
    for n in range(72):
        canvas=Image.new('RGB',(len(tiles)*216+20,518),PANEL)
        dd=ImageDraw.Draw(canvas)
        for i,(st,pct,label) in enumerate(tiles):
            x=20+i*216
            image=compose(st,pct=pct,phase=n).crop(crop).resize((168,351),Image.Resampling.NEAREST)
            canvas.paste(image,(x,24))
            dd.text((x,392),label,font=small,fill=INK)
            low_static=pct is not None and len(pattern_for_height(round(99*pct/100),n)[0])<3
            dd.text((x,419),'STATIC' if st=='nodata' or pct is None or low_static else f'{n/30:0.2f} S',font=small,fill=INK)
        frames.append(canvas)
    durations=[40 if n%3==2 else 30 for n in range(72)]
    frames[0].save(HERE/'clock-charging-across-states.gif',save_all=True,
                   append_images=frames[1:],duration=durations,loop=0,optimize=True,disposal=2)
    print('Rendered solid-fill clock battery study to',HERE)

if __name__ == '__main__':
    main()
