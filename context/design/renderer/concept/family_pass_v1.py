"""OCTOWHERE family pass V1: clock, C1 missing views, S1 subpages, H2 states.

From renderer/: PYTHONPATH=. python3 concept/family_pass_v1.py concept/family-pass-v1-out
Visual concepts only. Fixtures and functional semantics come from the existing renderer.
"""
from pathlib import Path
import math
import sys
from PIL import Image, ImageDraw, ImageFont

import lib
lib.RECORD = False
from lib import Canvas, SS, rgb
import clockface4 as CK
import clock4 as OLD_SCATTER
import aod as AOD
from face import X0, BIG, HOURS_BASE, MIN_BASE
from concept import settings_study as S1
from concept import compass_noise as C1

OUT = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else Path('concept/family-pass-v1-out').resolve()
OUT.mkdir(parents=True, exist_ok=True)
W = 466
BLACK=(0,0,0); WHITE=(221,225,231); MUTED=(132,142,154)
FAINT=(67,81,98); PURPLE=(52,16,112); BLUE=(42,102,175); ORANGE=(246,112,18)
LIME=(192,254,4); RED=(242,71,35)

# Clock K1: two stable, 8 px grid regions in the current rectangle grammar.
# This is a concept seed; it deliberately does not assert the firmware's hash.
def hash32(v):
    v=(v^(v>>16))*0x7feb352d & 0xffffffff
    v=(v^(v>>15))*0x846ca68b & 0xffffffff
    return (v^(v>>16))&0xffffffff

def clock_scatter(cv,*args,**kwargs):
    fields=((304,139,137,-.60,.48,0x8c70),(164,363,111,2.55,.28,0x8c71))
    # Names above are (cx, cy, radius, facing, density, seed).
    for cx,cy,radius,facing,density,seed in fields:
        for iy in range(0,58):
            y=-2+8*iy
            if 196 <= y <= 319: continue
            for ix in range(0,58):
                x=12+8*ix
                if math.hypot(x-233,y-233)>228: continue
                dx=x-cx;dy=y-cy;dist=math.hypot(dx,dy)
                if dist>radius:continue
                radial=.45+.55*max(0,min(1,(dist-40)/180))
                turn=.45+.55*((dx*math.cos(facing)+dy*math.sin(facing))/dist if dist else 1)
                h=hash32(seed^hash32(ix*0x9e3779b1&0xffffffff)^hash32(iy*0x85ebca77&0xffffffff))
                if h/2**32>=max(0,radial*turn*density):continue
                hollow=(hash32(h^0x12b591)/2**32)<.6
                if hollow:
                    cv.rect(x-3,y-3,x+3,y+3,PURPLE)
                    cv.rect(x-1,y-1,x+1,y+1,'BLACK')
                else:cv.rect(x-2,y-2,x+2,y+2,PURPLE)

def hour_rail(im,h,active=True):
    im=im.copy();d=ImageDraw.Draw(im)
    for index in range(24):
        x=92+index*12
        d.rectangle((x,412,x+4,414),fill=(39,47,54))
    if active:
        x=92+(h%24)*12
        d.rectangle((x-1,408,x+6,418),fill=LIME)
        d.rectangle((x+1,410,x+4,416),fill=BLACK)
    return im

original_scatter=OLD_SCATTER.scatter
original_lower=CK.lower
OLD_SCATTER.scatter=clock_scatter
def clock_lower(cv,state,f,kd=1.0,kz=1.0):
    if state=='nodata': return
    return original_lower(cv,state,f,kd,kz)
CK.lower=clock_lower
try:
    for state in CK.STATES:
        im=CK.face(state,scatter_k=0 if state=='nodata' else 1)
        if state!='nodata':
            im=hour_rail(im,CK.F['h'],active=state not in ('stopped','nozone'))
        im.save(OUT/f'clock-K1-{state}.png')
    fixtures={
        'charging':dict(CK.F,charging=True),
        'battery-low':dict(CK.F,bat=12),
        'battery-unknown':dict(CK.F,bat=None),
        'long-zone':dict(CK.F,zone='AMERICA/ARGENTINA/BUENOS_AIRES',abbr='ART',off='-03:00'),
    }
    for name,fixture in fixtures.items():
        hour_rail(CK.face('gnss',f=fixture),fixture['h']).save(OUT/f'clock-K1-{name}.png')
finally:
    OLD_SCATTER.scatter=original_scatter
    CK.lower=original_lower

# S1's indexed pages remain unchanged. These subpages reuse their glyph construction,
# column alignment and sparse edge marks, with the functional controls made explicit.
def sf(n):return ImageFont.truetype(str(S1.FONT/'MonoR.otf'),n*S1.S)

def subpage(title,index,page,key,accent=WHITE,action='BACK'):
    im,d=S1.fresh();S1.quiet_scatter(d,page)
    S1.txt(d,(233,28),'SETTINGS','Shapiro.ttf',27,WHITE,'mt')
    S1.txt(d,(233,65),title+' / '+index,'MonoR.otf',12,MUTED,'mt')
    S1.line(d,(83,83,383,83),FAINT)
    S1.rect(d,(83,96,182,139),BLACK,outline=FAINT)
    S1.txt(d,(132,106),action,'MonoB.otf',15,WHITE,'mt')
    S1.icon(d,key,330,92,6,3,accent)
    S1.line(d,(83,145,383,145),FAINT)
    return im,d

def finish(im):return S1.clipped(im)

def footer(d,text='TAP TO KEEP'):
    S1.line(d,(83,370,383,370),FAINT)
    S1.txt(d,(233,398),text,'MonoR.otf',12,MUTED,'mt')

# Zone picker: single selected zone and visible next value, preserving stepping/confirm.
im,d=subpage('ZONE','02',0,'ZONE')
S1.txt(d,(97,161),'01  NEAREST FIRST','MonoR.otf',13,MUTED)
S1.txt(d,(97,190),'DUBLIN','MonoB.otf',39,WHITE)
S1.txt(d,(97,239),'EUROPE/DUBLIN  IST','MonoR.otf',14,MUTED)
S1.line(d,(83,278,383,278),WHITE)
S1.txt(d,(97,295),'LONDON','MonoB.otf',24,MUTED)
S1.txt(d,(97,335),'1/26','MonoR.otf',14,WHITE)
footer(d,'DRAG TO CHOOSE  /  TAP TO SELECT')
finish(im).save(OUT/'settings-D1-zone.png')

# Brightness: no changed behavior; ten 10–100% steps, immediate live value.
im,d=subpage('BRIGHTNESS','02',0,'BRIGHTNESS',action='CANCEL')
S1.txt(d,(91,160),'DRAG TO SET','MonoR.otf',14,MUTED)
S1.txt(d,(89,189),'47%','MonoB.otf',59,WHITE)
S1.txt(d,(91,258),'LEVEL 05 / 10','MonoR.otf',14,MUTED)
for i in range(10):
    x=88+i*30
    S1.rect(d,(x,288,x+23,306),WHITE if i<5 else BLACK,outline=WHITE if i<5 else FAINT)
S1.txt(d,(88,322),'10','MonoR.otf',14,MUTED)
S1.txt(d,(382,322),'100','MonoR.otf',14,MUTED,'rt')
footer(d)
finish(im).save(OUT/'settings-D1-brightness.png')

# Timeout: five values, selected 1 MIN. White knockout makes the chosen row unmistakable.
im,d=subpage('TIMEOUT','03',0,'TIMEOUT',action='CANCEL')
S1.txt(d,(95,158),'30 S','MonoB.otf',23,MUTED)
S1.rect(d,(83,207,383,276),WHITE)
S1.txt(d,(97,218),'1 MIN','MonoB.otf',43,BLACK)
S1.txt(d,(95,299),'5 MIN','MonoB.otf',23,MUTED)
S1.txt(d,(331,340),'03 / 05','MonoR.otf',13,MUTED,'rt')
footer(d,'DRAG TO CHOOSE  /  TAP TO KEEP')
finish(im).save(OUT/'settings-D1-timeout.png')

# Device: facts remain readable; destructive entry is separated from live readouts.
im,d=subpage('DEVICE','08',1,'DEVICE')
for i,(label,value) in enumerate((('VERSION','0.1.0'),('BATTERY','USB 87%'),('GNSS','FIX 9'))):
    y=164+i*54
    S1.txt(d,(89,y),label,'MonoB.otf',14,MUTED)
    S1.txt(d,(376,y),value,'MonoR.otf',16,WHITE,'rt')
    S1.line(d,(83,y+36,383,y+36),FAINT)
S1.txt(d,(233,371),'DRAG FOR DETAILS','MonoR.otf',12,MUTED,'mt')
finish(im).save(OUT/'settings-D1-device.png')

im,d=subpage('DEVICE','08',1,'DEVICE')
for i,line in enumerate(('TIME ZONE BOUNDARIES:', 'TIMEZONE-BOUNDARY-BUILDER 2026D',
                         '© OPENSTREETMAP CONTRIBUTORS', 'ODBL 1.0',
                         'ZONE RULES: IANA TZDATA 2026D')):
    S1.txt(d,(233,163+i*25),line,'MonoR.otf',13,MUTED,'mt')
S1.rect(d,(83,324,383,368),BLACK,outline=ORANGE)
S1.txt(d,(233,335),'CLEAR SETTINGS','MonoB.otf',17,ORANGE,'mt')
S1.txt(d,(233,397),'END OF DEVICE','MonoR.otf',12,MUTED,'mt')
finish(im).save(OUT/'settings-D1-device-end.png')

# Clear stage 2: keep orange two-stage drag and the current warning fixture.
S1.GLYPHS['CLEAR']='10001 01010 00100 01010 10001'
def clear_screen(progress):
    im,d=subpage('CLEAR','08',1,'CLEAR',ORANGE,action='CANCEL')
    S1.txt(d,(91,163),'DRAG ACROSS TO CLEAR','MonoR.otf',14,ORANGE)
    S1.line(d,(83,257,383,257),FAINT,2)
    hx=round(83+240*progress)
    if progress:S1.rect(d,(83,225,hx+64,289),ORANGE)
    S1.rect(d,(323,225,383,289),BLACK,outline=FAINT,width=2)
    S1.rect(d,(hx,225,hx+64,289),ORANGE)
    S1.rect(d,(hx+20,255,hx+46,260),BLACK)
    S1.rect(d,(hx+40,248,hx+48,267),BLACK)
    S1.txt(d,(233,318),'ERASES THE ZONE CHOICE','MonoR.otf',14,MUTED,'mt')
    S1.txt(d,(233,341),'AND BRIGHTNESS','MonoR.otf',14,MUTED,'mt')
    footer(d,'RELEASE IN TARGET TO CONFIRM')
    return finish(im)
clear_screen(0).save(OUT/'settings-D1-clear.png')
clear_screen(.55).save(OUT/'settings-D1-clear-drag.png')

saved_aod=S1.PAGES[0][3]
S1.PAGES[0][3]=('ALWAYS ON','ON')
S1.ledger(0,scatter=True).save(OUT/'settings-S1-always-on.png')
S1.PAGES[0][3]=saved_aod

# Manual time is reached through the clock's picker; keep explicit UTC/offset relationship.
im,d=subpage('OFFSET','01',0,'ZONE',action='CANCEL')
S1.txt(d,(91,157),'12:07   +00:00','MonoR.otf',16,MUTED)
S1.txt(d,(91,196),'13:07','MonoB.otf',55,WHITE)
S1.txt(d,(296,239),'UTC +01:00','MonoR.otf',14,WHITE,'rt')
S1.line(d,(83,278,383,278),WHITE)
S1.txt(d,(91,299),'14:07    +02:00','MonoR.otf',18,MUTED)
S1.rect(d,(178,323,288,359),WHITE)
S1.txt(d,(233,331),'AUTO','MonoB.otf',17,BLACK,'mt')
footer(d,'DRAG OFFSET  /  TAP FOR ZONES')
finish(im).save(OUT/'settings-D1-manual-time.png')

# C1 extension: hold a coherent background through the two views that previously
# had no concept still. A swipe is one snapshot; animation ownership remains open.
top=C1.compose(C1.triangles(8),'top-edge-up',.30)
top.save(OUT/'compass-C1-top-edge-up.png')
C1.STATES['swiping']=Image.open(C1.CAP/'compass-swiping.png').convert('RGB')
C1.compose(C1.blocks(4),'swiping',.40).save(OUT/'compass-C1-swiping.png')

# H2's missing states. Unknown/stopped time has no active hour marker.
def aod_state(state):
    cv=Canvas()
    if state=='nodata':
        cv.rect(86,204,115,208,RED);cv.rect(350,204,379,208,RED)
        cv.text('NO DATA','shapiro',28,RED,cx=233,cy=256)
        cv.text('CLOCK','mono',13,'GRAY',cx=233,y=301)
        return cv.render()
    AOD.mono_digits(cv,'--',BIG,'WHITE',X0,HOURS_BASE)
    AOD.mono_digits(cv,'--',BIG,'WHITE',X0,MIN_BASE)
    col='ORANGE' if state=='stopped' else 'GRAY'
    cv.text('STOPPED' if state=='stopped' else 'NO ZONE','mono',16,col,x=262,bottom=MIN_BASE)
    im=cv.render();d=ImageDraw.Draw(im)
    # H2's bridge is quiet and fixed without a minute trigger.
    for x,y,w in ((42,196,34),(227,194,42),(373,198,39),(36,382,25),(383,380,27)):
        d.rectangle((x,y,x+w,y+5),fill=(7,19,39))
        d.line((x,y+2,x+w,y+2),fill=(15,35,62))
    for h in range(24):
        x=64+h*14
        d.rectangle((x,408,x+4,409),fill=(13,23,39))
    return im
for state in ('nozone','stopped','nodata'):
    aod_state(state).save(OUT/f'aod-H2-{state}.png')
print('rendered',len(list(OUT.glob('*.png'))),'screens in',OUT)
