"""Start-up study: six-row S1 self-test, revised G2 field, card, F2 clock entry.

Run from renderer: PYTHONPATH=. python3 concept/startup_s1.py OUTPUT_DIR
All test answer times and live values come from the supplied concept fixtures.
"""
from pathlib import Path
import random
import sys

import numpy as np
from PIL import Image, ImageDraw, ImageFont

import lib
lib.RECORD=False
from lib import Canvas, cap_height, icon_sized
import startup as S
import startup30 as S30
import identity_g2 as G2
import identity_g as IG
import logo_card as LC
from anim import FRAME_MS, save_gif30
from concept import noise_families as F

OUT=Path(sys.argv[1]).resolve() if len(sys.argv)>1 else None
W=466


def post(t,cells=S.CELLS):
    cv=Canvas()
    cv.ring(230,232,'GRAY')
    cv.text('SELF TEST','shapiro',27,'WHITE',cx=233,y=30)
    decided=sum(t>=entry[2] for entry in cells)
    cv.text(f'DECIDED  {decided}/6','mono',13,'GRAY',cx=233,y=70)
    ys=(89,134,179,224,269,314)
    for i,((name,glyph,at,ok),y) in enumerate(zip(cells,ys)):
        left=82 if i in (0,5) else 62
        right=384 if i in (0,5) else 404
        cv.rect(left,y,right,y+1,'GRAY')
        cv.text(f'{i+1:02}','mono',13,'GRAY',x=left+9,y=y+14)
        if t>=at:
            color='WHITE' if ok else 'RED'
            rows=min(5,max(0,int((t-at)//S.ROW_MS+1))) if ok else 5
            glyph_=glyph if ok else S.NODATA
            state='OK' if ok else 'FAIL'
        else:
            color='GRAY';rows=0;glyph_=glyph;state='--'
        # The test row has the same index/icon/name/value rhythm as S1 settings.
        icon_sized(cv,left+38,y+6,color,glyph_,module=5,pad=4,rows_shown=rows)
        cv.text(name,'bold',17,'WHITE' if t>=at and ok else ('RED' if t>=at else 'GRAY'),
                x=left+82,y=y+12)
        cv.text(state,'bold',16,color,right=right-12,cy=y+23)
        if t>=at and not ok:cv.rect(left,y+4,left+4,y+40,'RED')
    cv.rect(82,359,384,360,'GRAY')
    cv.text('VERSION 0.1.0','mono',13,'GRAY',cx=233,y=380)
    return cv.render()


def purple_field(cv,n,appear=1.0):
    """Purple vertical signal strips recompose on hard cuts, then hold still."""
    phase=min(4,n//5)
    rng=random.Random(10479+phase*4099)
    density=[.35,.55,.9,.7,.62][phase]
    for band in (0,1):
        for _ in range(48 if phase>1 else 35):
            x=rng.randrange(8,448)
            y=rng.randrange(10,184) if band==0 else rng.randrange(325,446)
            w=rng.choice([1,2,2,3,5,9,15]);h=rng.choice([8,12,25,44,68])
            if rng.random()>density*appear:continue
            # Short scan gaps make these read as transmitted columns, not F2 blocks.
            cv.rect(x,y,x+w,y+h,'PURPLE')
            if w>=5:
                for cut in range(y+5,y+h,10):
                    cv.rect(x,cut,x+w,cut+2,'BLACK')
    if phase>=2:
        # Edge registration lines survive the final held state.
        for x in (30,432):
            cv.rect(x,62,x+1,166,'PURPLE')
            cv.rect(x,348,x+1,410,'PURPLE')


def identity(n,bold=False):
    G2.scatter=purple_field
    # G3 starts with tall glyph cells that step down into the established G2 composition.
    G2.F_WORD=4 if bold else 8
    if bold:
        stretch=3.15 if 4<=n<=6 else 2.60 if 7<=n<=9 else 2.10 if 10<=n<=12 else 1.8
    else:
        stretch=2.45 if 8<=n<=10 else 2.05 if 11<=n<=13 else 1.8
    G2.BIG['hatch']=(40,stretch)
    return G2.render(n)


F2=np.asarray(F.still('blocks',0).convert('RGB'))
yy,xx=np.mgrid[:W,:W]
RING=(xx-233)**2+(yy-233)**2>=227**2
RING&=(xx-233)**2+(yy-233)**2<=233**2


def clock_entry(n):
    if n>=13:return Image.fromarray(F2.copy())
    a=np.zeros_like(F2)
    if n>=1:a[RING]=F2[RING]
    upper=(yy<198)&(xx<min(W,max(0,(n-1)*W//8)))
    a[upper]=F2[upper]
    band=(yy>=198)&(yy<318)&(xx<min(W,max(0,(n-2)*W//6)))
    a[band]=F2[band]
    lower=(yy>=318)&(np.abs(xx-233)<max(0,(n-6)*39))
    a[lower]=F2[lower]
    return Image.fromarray(a)


def ok_frames():
    fr=[post(n*FRAME_MS) for n in range(S30.LAST+S30.HOLD)]
    for n in range(G2.REST+LC.F_CLOCK):fr.append(identity(n,bold=True))
    for n in range(17):fr.append(clock_entry(n))
    fr.extend([fr[-1]]*15)
    return fr


def fail_frames():
    fr=[post(n*FRAME_MS,S30.FCELLS) for n in range(S30.LAST+S30.FAIL_HOLD)]
    fr.extend([IG.k_frame(n*FRAME_MS) for n in range(S30.K_FRAMES)])
    fr.extend([clock_entry(n) for n in range(17)])
    fr.extend([fr[-1]]*15)
    return fr


def board(path):
    samples=[
      ('SELF TEST / PENDING',post(0)),
      ('POWER + CLOCK',post(120)),
      ('SIX DECIDED / HOLD',post(800)),
      ('IDENTITY / COLUMNS',identity(3,bold=True)),
      ('IDENTITY / STRETCH',identity(8,bold=True)),
      ('IDENTITY / HELD',identity(40,bold=True)),
      ('LOGO CARD',identity(62,bold=True)),
      ('CLOCK / ENTRY',clock_entry(5)),
      ('CLOCK / F2',clock_entry(16)),
    ]
    cell=233;gap=14;cap=30;margin=20
    sh=Image.new('RGB',(3*cell+2*gap+2*margin,3*(cell+cap)+2*gap+2*margin),(24,24,27))
    d=ImageDraw.Draw(sh);f=ImageFont.truetype('fonts/MonoB.otf',14)
    for i,(name,im) in enumerate(samples):
        x=margin+(i%3)*(cell+gap);y=margin+(i//3)*(cell+cap+gap)
        sh.paste(im.resize((cell,cell),Image.Resampling.LANCZOS),(x,y))
        d.text((x,y+cell+6),name,font=f,fill=(214,220,226))
    sh.save(path)


def failure_board(path):
    samples=[
      ('SELF TEST / PENDING',post(0,S30.FCELLS)),
      ('05 MAGNET / FAIL',post(350,S30.FCELLS)),
      ('FAULT / CUT',IG.k_frame(0)),
      ('FAULT / HELD',IG.k_frame(2000)),
      ('CLOCK / F2',clock_entry(16)),
    ]
    cell=233;gap=14;cap=30;margin=20
    sh=Image.new('RGB',(len(samples)*cell+(len(samples)-1)*gap+2*margin,cell+cap+2*margin),(24,24,27))
    d=ImageDraw.Draw(sh);f=ImageFont.truetype('fonts/MonoB.otf',14)
    for i,(name,im) in enumerate(samples):
        x=margin+i*(cell+gap)
        sh.paste(im.resize((cell,cell),Image.Resampling.LANCZOS),(x,margin))
        d.text((x,margin+cell+6),name,font=f,fill=(214,220,226))
    sh.save(path)


if __name__=='__main__':
    OUT.mkdir(parents=True,exist_ok=True)
    board(OUT/'startup-S1-storyboard.png')
    failure_board(OUT/'startup-S1-failure-path.png')
    post(800).save(OUT/'selftest-S1-all-pass.png')
    post(350,S30.FCELLS).save(OUT/'selftest-S1-fail.png')
    identity(40,bold=True).save(OUT/'identity-G3-held.png')
    save_gif30([identity(n,bold=False) for n in range(G2.REST)],str(OUT/'identity-G2r-restrained.gif'))
    save_gif30([identity(n,bold=True) for n in range(G2.REST)],str(OUT/'identity-G3-stretch.gif'))
    save_gif30(ok_frames(),str(OUT/'startup-S1-success.gif'))
    save_gif30(fail_frames(),str(OUT/'startup-S1-failure.gif'))
