"""KH clock typography applied to settled H2b AOD, with its sparse layout preserved."""
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont
import numpy as np, math
OUT=Path(__file__).resolve().parent
PROJECT=OUT.parent/'octowhere-design-project'
FONT=PROJECT/'renderer/fonts'
WHITE=(210,211,214);GRAY=(136,142,152);ORANGE=(241,113,13);RED=(242,71,35)
SS=4

def face(state='local',h=13,m=7,pct=87,shift=(0,0)):
    bg=Image.new('RGB',(466,466));d=ImageDraw.Draw(bg)
    if state!='nodata':
        if state=='local':
            for hour in range(24):
                x=64+hour*14;d.rectangle((x,408,x+4,409),fill=(16,28,49))
            x=64+h*14;d.rectangle((x,404,x+4,410),fill=(56,79,6))
            parts=[(38,194,37,13,0),(87,202,18,7,1),(222,192,46,13,2),(277,201,24,8,1),(370,195,50,12,0),(36,380,32,18,1),(379,373,41,21,0)]
            for i,(x,y,w,height,lev) in enumerate(parts):
                x+=0 if i in (0,2,4) else ((m+i)%3-1)*4
                d.rectangle((x,y,x+w-1,y+height-1),fill=[(6,17,36),(9,24,47),(12,30,55)][lev])
                for yy in range(y+m%3,y+height,3): d.line((x,yy,x+w-1,yy),fill=(17+lev*3,37+lev*5,67+lev*7))
                if i%2==0:d.rectangle((x+w-8,y,x+w-1,y+4),fill=(0,0,0))
            for i in range(8):
                x=30+i*7 if i<4 else 374+(i-4)*8;d.rectangle((x,214,x+1,215),fill=(23,45,78))
        else:
            for x,y,w in ((42,196,34),(227,194,42),(373,198,39),(36,382,25),(383,380,27)):
                d.rectangle((x,y,x+w,y+5),fill=(7,19,39));d.line((x,y+2,x+w,y+2),fill=(15,35,62))
            for hour in range(24):
                x=64+hour*14;d.rectangle((x,408,x+4,409),fill=(13,23,39))
            if state=='nozone':
                x=64+h*14;d.rectangle((x,404,x+4,410),fill=(35,59,81))
    im=bg.resize((466*SS,466*SS),Image.Resampling.NEAREST);d=ImageDraw.Draw(im)
    def font(file,size):return ImageFont.truetype(str(FONT/file),size*SS)
    def text(s,file,size,col,x=None,y=0,cx=None):
        f=font(file,size);box=f.getbbox(s,anchor='lt')
        xx=x*SS if x is not None else cx*SS-(box[0]+box[2])/2
        d.text((round(xx),y*SS),s,font=f,fill=col,anchor='lt')
    def digits(s,base):
        f=font('KHB.otf',136) if s!='--' else font('MonoB.otf',136)
        bottom=max(f.getbbox(ch,anchor='ls')[3] for ch in '0123456789') if s!='--' else 0
        for ch,cx in zip(s,(103,185)):
            box=f.getbbox(ch,anchor='ls');d.text((round(cx*SS-(box[0]+box[2])/2),base*SS-bottom),ch,font=f,fill=WHITE,anchor='ls')
    if state=='nodata':
        for a,b in ((86,115),(350,379)):d.rectangle((a*SS,204*SS,b*SS-1,208*SS-1),fill=RED)
        f=font('Shapiro.ttf',28);box=f.getbbox('NO DATA',anchor='lt')
        text('NO DATA','Shapiro.ttf',28,RED,cx=233,y=round(256-(box[3]-box[1])/SS/2))
        text('CLOCK','KHB.otf',14,GRAY,cx=233,y=301)
    else:
        digits('--' if state=='stopped' else f'{h:02}',184)
        digits('--' if state=='stopped' else f'{m:02}',305)
        if state=='local':text('THU 24 SEP','MonoR.otf',16,GRAY,cx=233,y=334)
        elif state=='nozone':
            # Same compact line, now following KH label + sans data roles.
            text('UTC / NO ZONE','KHB.otf',16,GRAY,cx=233,y=334)
        else:text('STOPPED','KHB.otf',16,ORANGE,cx=233,y=334)
    col=ORANGE if pct is not None and pct<=15 else (132,142,154)
    label='BAT ';value=f'{pct}%' if pct is not None else '--'
    f1=font('KHB.otf',14);f2=font('SansL.otf',14)
    x=367*SS-d.textlength(label,font=f1)-d.textlength(value,font=f2)
    d.text((round(x),96*SS),label,font=f1,fill=col,anchor='lt')
    x+=d.textlength(label,font=f1);d.text((round(x),96*SS),value,font=f2,fill=col,anchor='lt')
    for i in range(10):
        x=276+i*10;d.rectangle((x*SS,112*SS,(x+7)*SS-1,116*SS-1),fill=(24,38,57))
        fraction=max(0,min(1,(pct or 0)/10-i))
        if fraction:d.rectangle((x*SS,112*SS,(x+round(6*fraction)+1)*SS-1,116*SS-1),fill=col)
    im=im.resize((466,466),Image.Resampling.BOX)
    moved=Image.new('RGB',im.size);moved.paste(im,shift)
    mask=Image.new('L',im.size);ImageDraw.Draw(mask).ellipse((0,0,465,465),fill=255)
    clipped=Image.new('RGB',im.size);clipped.paste(moved,(0,0),mask)
    return clipped

def board(items,name,cols=3):
    sheet=Image.new('RGB',(20+486*cols,20+516*math.ceil(len(items)/cols)),(22,24,28));d=ImageDraw.Draw(sheet)
    f=ImageFont.truetype(str(FONT/'MonoB.otf'),15)
    for i,(label,im) in enumerate(items):
        x=10+i%cols*486;y=10+i//cols*516;sheet.paste(im,(x,y+30));d.text((x,y+4),label,font=f,fill=WHITE)
    sheet.save(OUT/name)

def main():
    cases=[('LOCAL / 13:07','local',87),('NO ZONE / UTC 12:07','nozone',87),('STOPPED','stopped',87),('NO DATA','nodata',87),('LOCAL / LOW BATTERY','local',12),('LOCAL / BAT UNKNOWN','local',None)]
    items=[]
    for label,st,pct in cases:
        im=face(st,h=12 if st=='nozone' else 13,pct=pct);name=st if pct==87 else ('low' if pct==12 else 'unknown');im.save(OUT/f'aod-{name}.png');items.append((label,im))
    board(items,'01-aod-all-states.png')
    old=Image.open(PROJECT/'renderer/concept/family-pass-v2-out/aod-H2b-local-1307.png').convert('RGB')
    mask=Image.new('L',(466,466));ImageDraw.Draw(mask).ellipse((0,0,465,465),fill=255);oldclean=Image.new('RGB',(466,466));oldclean.paste(old,(0,0),mask)
    active=Image.open(OUT.parent/'octowhere-clock-band-selected-v1/clock-gnss.png')
    board([('PREVIOUS AOD / H2b',oldclean),('UPDATED AOD / KH',face()),('SELECTED ACTIVE CLOCK',active)],'02-active-aod-comparison.png')
    board([(f'{h:02}:{m:02}',face(h=h,m=m)) for h,m in [(0,0),(8,59),(13,8),(23,59)]],'03-time-fit.png',cols=2)
    board([(f'SHIFT {dx:+},{dy:+}',face(shift=(dx,dy))) for dx,dy in [(0,0),(3,0),(-2,-2)]],'04-pixel-shift.png')
    for name,im in [('previous',oldclean),('updated',face())]:
        arr=np.array(im);print(name,'lit>16',round(float((arr.max(2)>16).sum())/(math.pi*233**2)*100,2),'weighted',round(float(arr.max(2).sum())/(255*math.pi*233**2)*100,2))
if __name__=='__main__':main()
