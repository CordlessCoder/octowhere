"""K1 accepted anchor with D2 settings and H2b always-on concept states.

Run from renderer/: PYTHONPATH=. python3 concept/family_pass_v2.py
"""
from pathlib import Path
import sys
from PIL import Image, ImageDraw

import aod as AOD
from lib import Canvas
from face import X0, BIG, HOURS_BASE, MIN_BASE
from concept import settings_study as S

ROOT=Path(__file__).resolve().parents[2]
OUT=Path(sys.argv[1]).resolve() if len(sys.argv)>1 else ROOT/'renderer/concept/family-pass-v2-out'
OUT.mkdir(parents=True,exist_ok=True)
BLACK=(0,0,0);WHITE=(221,225,231);MUTED=(132,142,154);FAINT=(68,81,98)
LIME=(192,254,4);ORANGE=(246,112,18);BLUE=(48,124,213);RED=(242,71,35)

def base(title,index,key,action='CANCEL',accent=LIME,page=0):
    im,d=S.fresh();S.quiet_scatter(d,page)
    S.txt(d,(233,28),'SETTINGS','Shapiro.ttf',27,WHITE,'mt')
    S.txt(d,(233,65),f'{title} / {index}','MonoR.otf',12,MUTED,'mt')
    S.line(d,(83,83,383,83),FAINT)
    S.rect(d,(83,96,182,139),BLACK,outline=FAINT)
    S.txt(d,(132,106),action,'MonoB.otf',15,WHITE,'mt')
    S.icon(d,key,330,92,6,3,accent)
    S.line(d,(83,145,383,145),FAINT)
    return im,d

def save(im,name):S.clipped(im).save(OUT/name)
def footer(d,label):
    S.line(d,(83,370,383,370),FAINT)
    S.txt(d,(233,398),label,'MonoR.otf',12,MUTED,'mt')

im,d=base('OFFSET','01','ZONE')
S.txt(d,(91,158),'12:07   +00:00','MonoR.otf',16,MUTED)
S.rect(d,(83,189,383,277),LIME)
S.txt(d,(96,192),'13:07','MonoB.otf',51,BLACK)
S.txt(d,(370,252),'UTC +01:00','MonoB.otf',14,BLACK,'rt')
S.txt(d,(91,294),'14:07   +02:00','MonoR.otf',17,MUTED)
S.rect(d,(178,326,288,359),WHITE)
S.txt(d,(233,331),'AUTO','MonoB.otf',16,BLACK,'mt')
footer(d,'DRAG OFFSET / TAP FOR ZONES')
save(im,'settings-D2-offset.png')

im,d=base('ZONE','02','ZONE',action='BACK')
S.txt(d,(91,159),'01  NEAREST FIRST','MonoR.otf',13,MUTED)
S.rect(d,(83,188,383,277),LIME)
S.txt(d,(95,193),'DUBLIN','MonoB.otf',37,BLACK)
S.txt(d,(96,245),'EUROPE/DUBLIN  IST','MonoB.otf',14,BLACK)
S.txt(d,(95,293),'LONDON','MonoB.otf',24,MUTED)
S.txt(d,(371,337),'01 / 26','MonoR.otf',13,MUTED,'rt')
footer(d,'DRAG TO CHOOSE / TAP TO SELECT')
save(im,'settings-D2-zone.png')

im,d=base('BRIGHTNESS','02','BRIGHTNESS')
S.txt(d,(91,157),'DRAG TO SET','MonoR.otf',13,MUTED)
S.rect(d,(83,187,383,269),LIME)
S.txt(d,(95,187),'47%','MonoB.otf',58,BLACK)
S.txt(d,(365,244),'LIVE','MonoB.otf',14,BLACK,'rt')
S.txt(d,(88,273),'WHOLE PERCENT / 10–100','MonoR.otf',12,MUTED)
# Each cell is one tenth of 0–100. 47% lights four full cells and 70% of the fifth.
for i in range(10):
    x=88+i*30;S.rect(d,(x,299,x+23,317),BLACK,outline=FAINT)
    fraction=max(0,min(1,4.7-i))
    if fraction:S.rect(d,(x,299,x+round(23*fraction),317),LIME)
S.txt(d,(88,329),'10','MonoR.otf',12,MUTED)
S.txt(d,(382,329),'100','MonoR.otf',12,MUTED,'rt')
footer(d,'DRAG TO PREVIEW / TAP TO KEEP')
save(im,'settings-D2-brightness.png')

im,d=base('TIMEOUT','03','TIMEOUT')
S.txt(d,(95,158),'30 S','MonoB.otf',23,MUTED)
S.rect(d,(83,207,383,276),LIME)
S.txt(d,(97,217),'1 MIN','MonoB.otf',43,BLACK)
S.txt(d,(95,298),'5 MIN','MonoB.otf',23,MUTED)
S.txt(d,(371,337),'03 / 05','MonoR.otf',13,MUTED,'rt')
footer(d,'DRAG TO CHOOSE / TAP TO KEEP')
save(im,'settings-D2-timeout.png')

im,d=base('DEVICE','08','DEVICE',action='BACK',accent=BLUE,page=1)
for i,(label,value) in enumerate((('VERSION','0.1.0'),('BATTERY','USB 87%'),('GNSS','FIX 9'))):
    y=161+i*55
    S.txt(d,(89,y),label,'MonoB.otf',14,MUTED)
    S.txt(d,(376,y),value,'MonoB.otf',16,BLUE if label=='GNSS' else WHITE,'rt')
    S.line(d,(83,y+37,383,y+37),FAINT)
S.txt(d,(233,374),'DRAG FOR DETAILS','MonoR.otf',12,MUTED,'mt')
save(im,'settings-D2-device-top.png')

im,d=base('DEVICE','08','DEVICE',action='BACK',accent=BLUE,page=1)
for i,label in enumerate(('TIME ZONE BOUNDARIES:','TIMEZONE-BOUNDARY-BUILDER 2026D',
                           '© OPENSTREETMAP CONTRIBUTORS','ODBL 1.0','ZONE RULES: IANA TZDATA 2026D')):
    S.txt(d,(233,155+i*23),label,'MonoR.otf',12,MUTED,'mt')
S.rect(d,(83,277,383,318),BLACK,outline=LIME)
S.txt(d,(233,285),'REPLAY START-UP','MonoB.otf',16,LIME,'mt')
S.rect(d,(83,329,383,370),BLACK,outline=ORANGE)
S.txt(d,(233,337),'CLEAR SETTINGS','MonoB.otf',16,ORANGE,'mt')
S.txt(d,(233,397),'END OF DEVICE','MonoR.otf',12,MUTED,'mt')
save(im,'settings-D2-device-end.png')

S.GLYPHS['REPLAY']='10101 00100 01110 00100 10101'
im,d=base('REPLAY','08','REPLAY',action='BACK',page=1)
S.txt(d,(91,158),'06 GNSS FAIL','MonoB.otf',20,MUTED)
S.rect(d,(83,207,383,276),LIME)
S.txt(d,(97,212),'GOOD','MonoB.otf',36,BLACK)
S.txt(d,(370,247),'SUCCESSFUL BOOT','MonoB.otf',12,BLACK,'rt')
S.txt(d,(95,299),'01 POWER FAIL','MonoB.otf',20,MUTED)
footer(d,'DRAG TO CHOOSE / TAP TO REPLAY')
save(im,'settings-D2-replay-choice.png')

im,d=base('FAILURE DEMO','08','REPLAY',action='BACK',accent=ORANGE,page=1)
S.txt(d,(95,158),'04 MOTION FAIL','MonoB.otf',19,MUTED)
S.rect(d,(83,207,383,276),ORANGE)
S.txt(d,(96,217),'05 MAGNET FAIL','MonoB.otf',25,BLACK)
S.txt(d,(95,300),'06 GNSS FAIL','MonoB.otf',19,MUTED)
S.txt(d,(371,342),'DEMO / 05 OF 06','MonoR.otf',12,ORANGE,'rt')
footer(d,'DRAG TO CHOOSE / TAP TO REPLAY')
save(im,'settings-D2-replay-failure.png')

def battery(im,pct=87):
    im=im.copy();d=ImageDraw.Draw(im)
    label=f'BAT {pct}%' if pct is not None else 'BAT --'
    from PIL import ImageFont
    f=ImageFont.truetype(str(S.FONT/'MonoB.otf'),14)
    color=ORANGE if pct is not None and pct<=15 else (132,142,154)
    d.text((367,91),label,font=f,anchor='ra',fill=color)
    for i in range(10):
        x=276+i*10
        d.rectangle((x,112,x+6,115),fill=(24,38,57))
        fraction=max(0,min(1,(pct or 0)/10-i))
        if fraction:d.rectangle((x,112,x+round(6*fraction),115),fill=color)
    return im

H2=ROOT/'renderer/concept/glitch-package'
for name in ('1307','1308'):
    battery(Image.open(H2/f'aod-H2-{name}.png').convert('RGB')).save(OUT/f'aod-H2b-local-{name}.png')

def aod_state(state,pct=87):
    cv=Canvas()
    if state=='nozone':
        AOD.mono_digits(cv,'12',BIG,'WHITE',X0,HOURS_BASE)
        AOD.mono_digits(cv,'07',BIG,'WHITE',X0,MIN_BASE)
        cv.text('UTC  /  NO ZONE','mono',16,'GRAY',cx=233,y=334)
    elif state=='nodata':
        cv.rect(86,204,115,208,RED);cv.rect(350,204,379,208,RED)
        cv.text('NO DATA','shapiro',28,RED,cx=233,cy=256)
        cv.text('CLOCK','mono',13,'GRAY',cx=233,y=301)
    else:
        AOD.mono_digits(cv,'--',BIG,'WHITE',X0,HOURS_BASE)
        AOD.mono_digits(cv,'--',BIG,'WHITE',X0,MIN_BASE)
        cv.text('STOPPED','mono',16,'ORANGE',cx=233,y=334)
    im=cv.render().convert('RGB');d=ImageDraw.Draw(im)
    if state!='nodata':
        for x,y,w in ((42,196,34),(227,194,42),(373,198,39),(36,382,25),(383,380,27)):
            d.rectangle((x,y,x+w,y+5),fill=(7,19,39))
            d.line((x,y+2,x+w,y+2),fill=(15,35,62))
        for h in range(24):
            x=64+h*14;d.rectangle((x,408,x+4,409),fill=(13,23,39))
        if state=='nozone':
            x=64+12*14;d.rectangle((x,404,x+4,410),fill=(35,59,81))
    return battery(im,pct)
for state in ('nozone','stopped','nodata'):
    aod_state(state).save(OUT/f'aod-H2b-{state}.png')
for value,name in ((12,'battery-low'),(None,'battery-unknown')):
    battery(Image.open(H2/'aod-H2-1307.png').convert('RGB'),value).save(OUT/f'aod-H2b-{name}.png')
print('rendered',len(list(OUT.glob('*.png'))),'V2 screens in',OUT)
