"""Reversed 45-degree, fifteen-pixel hatch for the absent battery state."""
from pathlib import Path
from math import floor
import subprocess
from PIL import Image, ImageDraw, ImageChops
import placement_base as PB

HERE=Path(__file__).resolve().parent
BF=PB.BF
PANEL=PB.PANEL
INK=PB.INK
FONT=PB.FONT
BITS=tuple(2 if (byte>>k)&1 else 1 for byte in b'0.1.0' for k in range(4))
assert len(BITS)==20 and sum(BITS)==27


def apportion(weights,total):
    """Allocate exact panel pixels in direct proportion to positive weights."""
    raw=[total*w/sum(weights) for w in weights]
    out=[floor(v) for v in raw]
    for i in sorted(range(len(out)),key=lambda j:raw[j]-out[j],reverse=True)[:total-sum(out)]:
        out[i]+=1
    assert min(out)>=1 and sum(out)==total
    return tuple(out)


def title_pattern(length,phase):
    """20 marks at the 87% reference, varying naturally with actual fill length.

    The identity barcode uses 1/2-unit bars and 2-unit gaps. Its data gives
    27 bar units to 38 inter-bar gap units, ignoring its trailing 2-unit gap.
    Preserve that visual ratio. V6's asymmetric gap weights supply motion
    without resetting the bar shapes or measured level endpoint.
    """
    if length<=0:return (),()
    count=max(1,min(23,round(length*20/150)))
    if count==1:return (length,),()
    widths=BF.resample(BITS,count)
    gap_ratio=2*(count-1)/(sum(widths)+2*(count-1))
    gap_total=max(count-1,min(length-count,round(length*gap_ratio)))
    bar_total=length-gap_total
    bars=apportion(widths,bar_total)
    # The title barcode's gaps are two units each. Let the established V6 beat
    # perturb that spacing gently, instead of letting its wider 1..6 grammar
    # dominate this denser horizontal mark.
    phase_weights=BF.resample(BF.gaps(phase),count-1)
    mean=sum(phase_weights)/len(phase_weights)
    gap_weights=tuple(1+0.35*(v/mean-1) for v in phase_weights)
    gaps=apportion(gap_weights,gap_total)
    assert sum(bars)+sum(gaps)==length
    return bars,gaps


def absent_hatch(cv,left,right,top,bottom,previous=False):
    """Clipped 45-degree gray bands; previous mode retains the V3 study."""
    ss=PB.BASE.SS
    clip=Image.new('L',cv.img.size,0)
    ImageDraw.Draw(clip).rectangle((left*ss,top*ss,right*ss-1,bottom*ss-1),fill=255)
    stripes=Image.new('L',cv.img.size,0)
    draw=ImageDraw.Draw(stripes)
    height=bottom-top
    if previous:
        width,pitch,slope,start=20,40,height,left-24
    else:
        width,pitch,slope,start=15,35,-height,left-27
    for x in range(start,right+height+pitch,pitch):
        draw.polygon([(x*ss,top*ss),((x+width)*ss-1,top*ss),
                      ((x+width+slope)*ss-1,bottom*ss-1),
                      ((x+slope)*ss,bottom*ss-1)],fill=255)
    mask=ImageChops.multiply(clip,stripes)
    cv.img.paste(Image.new('RGB',cv.img.size,PB.BASE.rgb('GRAY')),(0,0),mask)


def gauge(cv,pct,charging,phase,k,band_col,style,pattern='title'):
    assert style=='wide_high'
    x0,y0,x1,y1=(262,281,440,311)
    ink=PB.BASE.gauge_color(band_col,pct)
    cv.rect(x0,y0,x1,y1,'BLACK')
    left,right,top,bottom=(265,437,284,308)
    if pct is None:
        if pattern=='dashes':
            mid=(left+right)/2
            for x in (mid-20,mid+7):
                cv.rect(x,(top+bottom)/2-1,x+13,(top+bottom)/2+2,'GRAY')
        else:
            absent_hatch(cv,left,right,top,bottom,previous=(pattern=='hatch20'))
        return
    length=round((right-left)*max(0,min(100,pct))/100*k)
    if length<=0:return
    if not charging:
        cv.rect(left,top,left+length,bottom,ink)
        return
    if band_col=='RED':phase=27
    bars,gaps=(title_pattern(length,phase) if pattern=='title'
               else BF.pattern_for_height(length,phase))
    x=left
    for i,w in enumerate(bars):
        cv.rect(x,top,x+w,bottom,ink)
        x+=w+(gaps[i] if i<len(gaps) else 0)
    assert x==left+length


def face(state='gnss',pct=87,charging=False,phase=27,pattern='title'):
    old=PB.gauge
    PB.gauge=lambda cv,p,c,ph,k,bc,style:gauge(cv,p,c,ph,k,bc,style,pattern)
    try:return PB.face('wide_high',state,pct,charging,phase)
    finally:PB.gauge=old


def board(items,columns,filename):
    rows=(len(items)+columns-1)//columns
    sheet=Image.new('RGB',(columns*486+20,rows*516+20),PANEL)
    d=ImageDraw.Draw(sheet)
    for i,(label,im) in enumerate(items):
        x=20+(i%columns)*486;y=20+(i//columns)*516
        sheet.paste(im,(x,y))
        d.text((x+4,y+478),label,font=FONT,fill=INK)
    sheet.save(HERE/filename,optimize=True)


def proportion_sheet():
    identity=Image.open(HERE/'identity-barcode-reference.png').convert('RGB')
    clock=face(charging=True)
    sh=Image.new('RGB',(900,420),PANEL)
    d=ImageDraw.Draw(sh)
    ref=identity.crop((31,308,146,339)).resize((575,155),Image.Resampling.NEAREST)
    bar=clock.crop((261,279,441,312)).resize((720,132),Image.Resampling.NEAREST)
    sh.paste(ref,(20,35));sh.paste(bar,(20,247))
    d.text((20,10),'IDENTITY / 20 BARS / 1:2 BAR WIDTH / 2-UNIT GAPS',font=FONT,fill=INK)
    d.text((20,215),'CLOCK / 20 BARS AT 87% / SAME BAR:GAP PROPORTION',font=FONT,fill=INK)
    sh.save(HERE/'battery-title-barcode-proportions.png',optimize=True)


def motion_sheet():
    phases=(0,17,27,46,58,70)
    sheet=Image.new('RGB',(3*580+20,2*165+20),PANEL)
    d=ImageDraw.Draw(sheet)
    for i,n in enumerate(phases):
        im=face(charging=True,phase=n)
        crop=im.crop((258,278,444,313)).resize((558,105),Image.Resampling.NEAREST)
        x=20+(i%3)*580;y=20+(i//3)*165
        sheet.paste(crop,(x,y))
        d.text((x,y+116),f'PHASE {n:02d} / {n/30:.2f} S',font=FONT,fill=INK)
    sheet.save(HERE/'battery-title-barcode-rhythm.png',optimize=True)


def charge_loop():
    """Exact 72-frame, 30fps loop with only the battery slot redrawn."""
    base=face(charging=True,phase=0)
    box=(262,281,440,311)
    cmd=['ffmpeg','-hide_banner','-loglevel','error','-y',
         '-f','rawvideo','-pix_fmt','rgb24','-s','466x466','-r','30',
         '-i','pipe:0','-an','-c:v','libx264','-pix_fmt','yuv420p',
         '-crf','10','-movflags','+faststart',
         str(HERE/'clock-unoutlined-barcode-charge-loop.mp4')]
    with subprocess.Popen(cmd,stdin=subprocess.PIPE) as process:
        for phase in range(72):
            cv=PB.BASE.Canvas()
            gauge(cv,87,True,phase,1.0,'LIME','wide_high')
            frame=base.copy()
            frame.paste(cv.render(offpanel=False).crop(box),box[:2])
            process.stdin.write(frame.tobytes())
        process.stdin.close()
        if process.wait():raise RuntimeError('ffmpeg failed')


def main():
    board([
        (f'{st.upper()} / 20PX \\',face(st,pct=None,pattern='hatch20'))
        for st in ('gnss','nozone','nodata')
    ]+[
        (f'{st.upper()} / 15PX /',face(st,pct=None))
        for st in ('gnss','nozone','nodata')
    ],3,'battery-hatch-direction-width-comparison.png')
    board([(f'{st.upper()} / BAT --',face(st,pct=None)) for st in BF.STATES],
          3,'battery-hatch-15px-all-states.png')
    board([
        ('12% / SOLID',face(pct=12)),
        ('BAT -- / HATCH',face(pct=None)),
        ('87% / SOLID',face()),
        ('87% / CHARGING',face(charging=True)),
    ],2,'battery-hatch-15px-neighbor-states.png')
    old=face(pct=None,pattern='hatch20').crop((260,277,442,313))
    new=face(pct=None).crop((260,277,442,313))
    sh=Image.new('RGB',(760,358),PANEL)
    d=ImageDraw.Draw(sh)
    sh.paste(old.resize((728,144),Image.Resampling.NEAREST),(16,25))
    sh.paste(new.resize((728,144),Image.Resampling.NEAREST),(16,203))
    d.text((16,5),'PREVIOUS / 20PX BAND + 20PX GAP / DOWN-RIGHT',font=FONT,fill=INK)
    d.text((16,182),'PROPOSED / 15PX BAND + 20PX GAP / DOWN-LEFT',font=FONT,fill=INK)
    sh.save(HERE/'battery-hatch-15px-detail.png',optimize=True)
    face(pct=None).save(HERE/'clock-absent-hatch-15px.png',optimize=True)
    print('Rendered reversed 15px absent-battery hatch',HERE)

if __name__=='__main__':main()
