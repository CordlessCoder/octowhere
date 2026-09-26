"""AOD hour rail without horizontal rules. Run from the backup renderer folder.
PYTHONPATH=. python3 concept/aod_norules.py OUTPUT_DIR
"""
import sys
from pathlib import Path
from PIL import Image,ImageDraw,ImageFont
import numpy as np
import aod as A
from lib import Canvas
from face import X0,BIG,HOURS_BASE,MIN_BASE

OUT=Path(sys.argv[1]).resolve() if len(sys.argv)>1 else None
if OUT is not None: OUT.mkdir(parents=True,exist_ok=True)

def foreground(h=13,m=7,date='THU 24 SEP'):
    cv=Canvas()
    A.mono_digits(cv,f'{h:02d}',BIG,'WHITE',X0,HOURS_BASE)
    A.mono_digits(cv,f'{m:02d}',BIG,'WHITE',X0,MIN_BASE)
    cv.text(date,'mono',16,'GRAY',cx=233,y=334)
    return cv.render().convert('RGB')

def face(minute=7,bridges=False):
    fg=np.array(foreground(m=minute))
    bg=Image.new('RGB',(466,466),(0,0,0));d=ImageDraw.Draw(bg)
    # H: 24 quiet blue cells, with a lime marker at the actual current hour.
    for h in range(24):
        x=64+h*14
        d.rectangle((x,408,x+4,409),fill=(16,28,49))
    x=64+13*14
    d.rectangle((x,404,x+4,410),fill=(56,79,6))
    if bridges:
        # Interrupted scanlined plates near the hours/minutes seam, never a rule.
        # Their staggered lengths and breaks make the two numeral groups distinct.
        parts=[(38,194,37,13,0),(87,202,18,7,1),(222,192,46,13,2),
               (277,201,24,8,1),(370,195,50,12,0),
               (36,380,32,18,1),(379,373,41,21,0)]
        phase=minute%3
        for i,(x,y,w,h,level) in enumerate(parts):
            dx=0 if i in (0,2,4) else ((minute+i)%3-1)*4
            x+=dx
            col=[(6,17,36),(9,24,47),(12,30,55)][level]
            d.rectangle((x,y,x+w-1,y+h-1),fill=col)
            for yy in range(y+phase,y+h,3):
                d.line((x,yy,x+w-1,yy),fill=(17+level*3,37+level*5,67+level*7))
            if i%2==0:d.rectangle((x+w-8,y,x+w-1,y+4),fill=(0,0,0))
        for i in range(8):
            x=30+i*7 if i<4 else 374+(i-4)*8
            d.rectangle((x,214,x+1,215),fill=(23,45,78))
    data=np.array(bg)
    # Foreground covers texture where digit strokes and date ink actually land.
    mask=fg.max(axis=2)>0;data[mask]=fg[mask]
    return Image.fromarray(data)

def sheet(shots,path):
    W=466;gap=18;mar=24;cap=75
    im=Image.new('RGB',(mar*2+W*len(shots)+gap*(len(shots)-1),mar*2+W+cap),(25,25,28))
    d=ImageDraw.Draw(im);f=ImageFont.truetype('fonts/MonoB.otf',17);s=ImageFont.truetype('fonts/MonoR.otf',13)
    for i,(x,name,detail) in enumerate(shots):
        x0=mar+i*(W+gap);im.paste(x,(x0,mar))
        d.text((x0,mar+W+8),name,font=f,fill=(212,211,214))
        d.text((x0,mar+W+34),detail,font=s,fill=(164,172,180))
    im.save(path)

if __name__=='__main__':
    shots=[]
    for key,m,bridge,label,detail in [
            ('H1',7,False,'H1 / HOUR RAIL','No rules, blue cells and current hour'),
            ('H2-1307',7,True,'H2 / GLITCH BRIDGE','Broken scanned blocks frame the time'),
            ('H2-1308',8,True,'H2 / NEXT MINUTE','Secondary cells jump with minute redraw')]:
        im=face(m,bridge);im.save(OUT/f'aod-{key}.png')
        lit,lum=A.lit_fraction(im)
        print(key,round(100*lit,3),round(100*lum,3))
        shots.append((im,label,f'{detail} / {100*lit:.2f}% lit'))
    sheet(shots,OUT/'aod-no-rules-study.png')
