"""D3 settings: vertically centered values and violet selection surfaces.

Run from renderer/: PYTHONPATH=. python3 concept/family_pass_v3.py
"""
from pathlib import Path
import sys
from concept import settings_study as S

ROOT=Path(__file__).resolve().parents[2]
OUT=Path(sys.argv[1]).resolve() if len(sys.argv)>1 else ROOT/'renderer/concept/family-pass-v3-out'
OUT.mkdir(parents=True,exist_ok=True)
BLACK=(0,0,0);WHITE=(221,225,231);MUTED=(132,142,154);FAINT=(68,81,98)
VIOLET=(179,43,229);ORANGE=(246,112,18);BLUE=(48,124,213)

def ink_center(d,box,label,face,size,fill,x=96):
    """Center visible glyph bounds vertically inside a native-pixel rectangle."""
    f=S.font(face,size);bb=f.getbbox(label,anchor='lt')
    y=round((box[1]+box[3])*S.S/2-(bb[1]+bb[3])/2)
    d.text((x*S.S,y),label,font=f,fill=fill,anchor='lt')

def center_label(d,box,label,face,size,fill):
    f=S.font(face,size);bb=f.getbbox(label,anchor='lt')
    y=round((box[1]+box[3])*S.S/2-(bb[1]+bb[3])/2)
    d.text((((box[0]+box[2])*S.S)/2,y),label,font=f,fill=fill,anchor='mt')

def base(title,index,key,action='CANCEL',accent=VIOLET,page=0):
    im,d=S.fresh();S.quiet_scatter(d,page)
    S.txt(d,(233,28),'SETTINGS','Shapiro.ttf',27,WHITE,'mt')
    S.txt(d,(233,65),f'{title} / {index}','MonoR.otf',12,MUTED,'mt')
    S.line(d,(83,83,383,83),FAINT)
    S.rect(d,(83,96,182,139),BLACK,outline=FAINT)
    center_label(d,(83,96,182,139),action,'MonoB.otf',15,WHITE)
    S.icon(d,key,330,92,6,3,accent)
    S.line(d,(83,145,383,145),FAINT)
    return im,d

def save(im,name):S.clipped(im).save(OUT/name)
def footer(d,label):
    S.line(d,(83,370,383,370),FAINT)
    S.txt(d,(233,398),label,'MonoR.otf',12,MUTED,'mt')

im,d=base('OFFSET','01','ZONE')
S.txt(d,(91,158),'12:07   +00:00','MonoR.otf',16,MUTED)
S.rect(d,(83,189,383,277),VIOLET)
ink_center(d,(83,189,383,277),'13:07','MonoB.otf',51,BLACK)
S.txt(d,(370,250),'UTC +01:00','MonoB.otf',14,BLACK,'rt')
S.txt(d,(91,294),'14:07   +02:00','MonoR.otf',17,MUTED)
S.rect(d,(178,326,288,359),WHITE)
center_label(d,(178,326,288,359),'AUTO','MonoB.otf',16,BLACK)
footer(d,'DRAG OFFSET / TAP FOR ZONES')
save(im,'settings-D3-offset.png')

im,d=base('ZONE','02','ZONE',action='BACK')
S.txt(d,(91,159),'01  NEAREST FIRST','MonoR.otf',13,MUTED)
S.rect(d,(83,188,383,277),VIOLET)
ink_center(d,(83,188,383,248),'DUBLIN','MonoB.otf',37,BLACK)
ink_center(d,(83,243,383,272),'EUROPE/DUBLIN  IST','MonoB.otf',14,BLACK)
S.txt(d,(95,293),'LONDON','MonoB.otf',24,MUTED)
S.txt(d,(371,337),'01 / 26','MonoR.otf',13,MUTED,'rt')
footer(d,'DRAG TO CHOOSE / TAP TO SELECT')
save(im,'settings-D3-zone.png')

im,d=base('BRIGHTNESS','02','BRIGHTNESS')
S.txt(d,(91,157),'DRAG TO SET','MonoR.otf',13,MUTED)
S.rect(d,(83,187,383,269),VIOLET)
ink_center(d,(83,187,383,269),'47%','MonoB.otf',54,BLACK,x=97)
S.txt(d,(365,239),'LIVE','MonoB.otf',14,BLACK,'rt')
S.txt(d,(88,273),'WHOLE PERCENT / 10–100','MonoR.otf',12,MUTED)
# Each cell is one tenth of 0–100. 47% lights four full cells and 70% of the fifth.
for i in range(10):
    x=88+i*30;S.rect(d,(x,299,x+23,317),BLACK,outline=FAINT)
    fraction=max(0,min(1,4.7-i))
    if fraction:S.rect(d,(x,299,x+round(23*fraction),317),VIOLET)
S.txt(d,(88,329),'10','MonoR.otf',12,MUTED)
S.txt(d,(382,329),'100','MonoR.otf',12,MUTED,'rt')
footer(d,'DRAG TO PREVIEW / TAP TO KEEP')
save(im,'settings-D3-brightness.png')

im,d=base('TIMEOUT','03','TIMEOUT')
S.txt(d,(95,158),'30 S','MonoB.otf',23,MUTED)
S.rect(d,(83,207,383,276),VIOLET)
ink_center(d,(83,207,383,276),'1 MIN','MonoB.otf',43,BLACK)
S.txt(d,(95,298),'5 MIN','MonoB.otf',23,MUTED)
S.txt(d,(371,337),'03 / 05','MonoR.otf',13,MUTED,'rt')
footer(d,'DRAG TO CHOOSE / TAP TO KEEP')
save(im,'settings-D3-timeout.png')

im,d=base('DEVICE','08','DEVICE',action='BACK',accent=BLUE,page=1)
for i,(label,value) in enumerate((('VERSION','0.1.0'),('BATTERY','USB 87%'),('GNSS','FIX 9'))):
    y=161+i*55
    S.txt(d,(89,y),label,'MonoB.otf',14,MUTED)
    S.txt(d,(376,y),value,'MonoB.otf',16,BLUE if label=='GNSS' else WHITE,'rt')
    S.line(d,(83,y+37,383,y+37),FAINT)
S.txt(d,(233,374),'DRAG FOR DETAILS','MonoR.otf',12,MUTED,'mt')
save(im,'settings-D3-device-top.png')

im,d=base('DEVICE','08','DEVICE',action='BACK',accent=BLUE,page=1)
for i,label in enumerate(('TIME ZONE BOUNDARIES:','TIMEZONE-BOUNDARY-BUILDER 2026D',
                           '© OPENSTREETMAP CONTRIBUTORS','ODBL 1.0','ZONE RULES: IANA TZDATA 2026D')):
    S.txt(d,(233,155+i*23),label,'MonoR.otf',12,MUTED,'mt')
S.rect(d,(83,277,383,318),BLACK,outline=VIOLET)
center_label(d,(83,277,383,318),'REPLAY START-UP','MonoB.otf',16,VIOLET)
S.rect(d,(83,329,383,370),BLACK,outline=ORANGE)
center_label(d,(83,329,383,370),'CLEAR SETTINGS','MonoB.otf',16,ORANGE)
S.txt(d,(233,397),'END OF DEVICE','MonoR.otf',12,MUTED,'mt')
save(im,'settings-D3-device-end.png')

S.GLYPHS['REPLAY']='10101 00100 01110 00100 10101'
im,d=base('REPLAY','08','REPLAY',action='BACK',page=1)
S.txt(d,(91,158),'06 GNSS FAIL','MonoB.otf',20,MUTED)
S.rect(d,(83,207,383,276),VIOLET)
ink_center(d,(83,207,383,276),'GOOD','MonoB.otf',36,BLACK)
S.txt(d,(370,249),'SUCCESSFUL BOOT','MonoB.otf',12,BLACK,'rt')
S.txt(d,(95,299),'01 POWER FAIL','MonoB.otf',20,MUTED)
footer(d,'DRAG TO CHOOSE / TAP TO REPLAY')
save(im,'settings-D3-replay-choice.png')

im,d=base('FAILURE DEMO','08','REPLAY',action='BACK',accent=ORANGE,page=1)
S.txt(d,(95,158),'04 MOTION FAIL','MonoB.otf',19,MUTED)
S.rect(d,(83,207,383,276),ORANGE)
ink_center(d,(83,207,383,276),'05 MAGNET FAIL','MonoB.otf',25,BLACK)
S.txt(d,(95,300),'06 GNSS FAIL','MonoB.otf',19,MUTED)
S.txt(d,(371,342),'DEMO / 05 OF 06','MonoR.otf',12,ORANGE,'rt')
footer(d,'DRAG TO CHOOSE / TAP TO REPLAY')
save(im,'settings-D3-replay-failure.png')

print('rendered',len(list(OUT.glob('*.png'))),'D3 settings screens in',OUT)
