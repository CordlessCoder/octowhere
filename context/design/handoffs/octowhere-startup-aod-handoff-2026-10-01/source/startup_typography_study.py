"""Startup small-type study. Maratype title and animation timing are fixed."""
from pathlib import Path
import importlib.util
import sys
import math

from PIL import Image, ImageDraw, ImageFont

OUT=Path(__file__).resolve().parent
WORK=OUT.parent
OLD=WORK/'octowhere-identity-maratype-study-v11'
sys.path.insert(0,str(OLD))
spec=importlib.util.spec_from_file_location('identity_baseline_v11',OLD/'render_study.py')
V=importlib.util.module_from_spec(spec);spec.loader.exec_module(V)
L=V.lib;S=V.S1;G=V.G17
L.FONT.update({'khb':str(V.RENDERER/'fonts/KHB.otf'),
               'khr':str(V.RENDERER/'fonts/KH.otf'),
               'sansl':str(V.RENDERER/'fonts/SansL.otf'),
               'sansb':str(V.RENDERER/'fonts/SansB.otf')})
ACTIVE='A'
ROW_ORIGINAL=V.micro_row
TEXT_ORIGINAL=L.Canvas.text
RING_ORIGINAL=L.Canvas.ring
LINES=[('VERSION ','0.1.0'),('SELF TEST ','6/6 OK')]
SPEC={'A':('mono',14),'B':('khr',18),'C':('sansl',14)}
LABELS={'A':'A CURRENT MONO','B':'B KH REGULAR','C':'C SANS LIGHT','D':'D KH LABEL / SANS DATA'}


def runs(line,v):
    a,b=line
    if v=='D':return [(a,'khb',18),(b,'sansl',14)]
    face,size=SPEC[v]
    return [(a+b,face,size)]


def line_width(line,v):
    pieces=runs(line,v);width=0
    for s,f,z in pieces:width+=L.font(f,z).getlength(s)/L.SS
    s,f,z=pieces[-1];bb=L.ink_box(s,f,z)[0]
    width-=L.font(f,z).getlength(s)/L.SS-bb[2]/L.SS
    return width


def draw_line(cv,line,x,y,v):
    for s,f,z in runs(line,v):
        # Align all pieces by their visible top within the two-line copy band.
        cv.text(s,f,z,'LIME',x=x,y=y)
        x+=L.font(f,z).getlength(s)/L.SS


def micro_row(cv,count,n):
    copy=max(line_width(line,ACTIVE) for line in LINES)
    V.BARCODE_ADVANCE=400-(25+72+25+copy+4*V.ROW_GAP)
    V.BARCODE_SCALE=V.BARCODE_ADVANCE/67
    V.MARK_ORIGIN=round(V.ROW_LEFT+V.BARCODE_ADVANCE+V.ROW_GAP)
    # Original glyph construction and reveal; suppress its final old text block.
    ROW_ORIGINAL(cv,min(count,4),n)
    if count>=5:
        x=V.ROW_LEFT+V.BARCODE_ADVANCE+4*V.ROW_GAP+25+72+25
        for i,line in enumerate(LINES):draw_line(cv,line,x,V.ROW_Y-2+16*i,ACTIVE)


def selftest_text(cv,s,face,px,col,**kwargs):
    if s in ('POWER','CLOCK','MOTION','MAGNET','TOUCH','GNSS'):
        if ACTIVE in 'BD':face,px='khb',18
        elif ACTIVE=='C':face,px='sansb',18
    elif s in ('OK','FAIL','--'):
        if ACTIVE=='B':face,px='khb',17
        elif ACTIVE=='C':face,px='sansb',17
    elif (s.startswith('DECIDED') or s.startswith('VERSION')) and ACTIVE=='C':
        face,px='sansl',14
    return TEXT_ORIGINAL(cv,s,face,px,col,**kwargs)


def post(t,cells=None):
    oldtext=L.Canvas.text;oldring=L.Canvas.ring
    L.Canvas.text=selftest_text
    L.Canvas.ring=lambda *a,**k:None
    try:return S.post(t) if cells is None else S.post(t,cells)
    finally:L.Canvas.text=oldtext;L.Canvas.ring=oldring


def boot_mask(v):
    f,z={'A':('khb',38),'B':('bold',38),'C':('sansb',38),'D':('shapiro',36)}[v]
    ft=L.font(f,z);bb=ft.getbbox('BOOT')
    mask=Image.new('L',(bb[2]-bb[0],bb[3]-bb[1]))
    ImageDraw.Draw(mask).text((-bb[0],-bb[1]),'BOOT',font=ft,fill=255)
    return mask.rotate(-90,expand=True)


def identity(n):
    old=V.micro_row;V.micro_row=micro_row
    try:return V.identity_frame(n)
    finally:V.micro_row=old


def board(items,name,cols=4):
    cw=486;rh=518
    out=Image.new('RGB',(cols*cw+20,math.ceil(len(items)/cols)*rh+20),(22,24,28))
    d=ImageDraw.Draw(out);f=ImageFont.truetype(V.RENDERER/'fonts/MonoB.otf',15)
    for i,(label,im) in enumerate(items):
        x=10+i%cols*cw;y=10+i//cols*rh
        d.text((x+3,y+8),label,font=f,fill=(215,220,225))
        out.paste(im,(x,y+42))
    out.save(OUT/name)


def make_stills():
    global ACTIVE
    passing=[];failing=[];ids=[];boots=[];crops=[]
    originalmask=G.BOOT_MASK
    for v in 'ABCD':
        ACTIVE=v
        a=post(800);b=post(800,V.S30.FCELLS);c=identity(97)
        for tag,im in [('selftest-pass',a),('selftest-fail',b),('identity-subtitle',c)]:
            im.save(OUT/f'{v}-{tag}.png')
        plabel={'A':'A CURRENT MONO','B':'B KH BOLD','C':'C SANS BOLD',
                'D':'D KH NAME / MONO STATUS'}[v]
        passing.append((plabel,a));failing.append((plabel,b));ids.append((LABELS[v],c))
        G.BOOT_MASK=boot_mask(v)
        boots.append(({'A':'A KH BOLD CURRENT','B':'B FRAKTION MONO BOLD',
                       'C':'C FRAKTION SANS BOLD','D':'D SHAPIRO'}[v],V.identity_frame(7)))
        crops.append((v,c.crop((25,303,440,343))))
    G.BOOT_MASK=originalmask
    board(passing+failing,'01-selftest-pass-fail.png')
    board(ids,'02-identity-subtitle.png')
    board(boots,'03-boot-label.png')
    close=Image.new('RGB',(880,4*135+20),(22,24,28));d=ImageDraw.Draw(close)
    f=ImageFont.truetype(V.RENDERER/'fonts/MonoB.otf',16)
    for i,(v,im) in enumerate(crops):
        d.text((20,12+i*135),LABELS[v],font=f,fill=(215,220,225))
        close.paste(im.resize((830,80),Image.Resampling.NEAREST),(20,42+i*135))
    close.save(OUT/'02b-subtitle-detail.png')
    oldlines=LINES[:]
    LINES[:]=[('VERSION ','0.12.10'),('SELF TEST ','5/6 FAIL')]
    stress=[]
    for v in 'ABCD':
        ACTIVE=v;stress.append((LABELS[v],identity(97)))
    LINES[:]=oldlines
    board(stress,'04-subtitle-copy-stress.png')


def make_motion():
    global ACTIVE
    oldmask=G.BOOT_MASK
    for v in 'ABCD':
        ACTIVE=v
        G.BOOT_MASK=boot_mask(v)
        frames=[identity(n) for n in range(120)]
        frames += [V.marker_impact(n) for n in range(19)]
        V.save_mp4(frames,OUT/f'identity-subtitle-{v}.mp4')
        # Functional decisions are fixture times, not a replacement boot duration.
        test=[post(n*1000/30,V.S30.FCELLS) for n in range(30)]
        test += [post(800,V.S30.FCELLS)]*15
        V.save_mp4(test,OUT/f'selftest-{v}.mp4')
        print('motion complete',v,flush=True)
    G.BOOT_MASK=oldmask


if __name__=='__main__':
    make_stills()
    if '--motion' in sys.argv:make_motion()
