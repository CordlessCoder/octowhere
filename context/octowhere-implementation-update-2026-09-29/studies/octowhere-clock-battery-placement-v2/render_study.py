"""Unoutlined wide K1 battery bar with identity-barcode charging proportions."""
from pathlib import Path
from math import floor
import subprocess
from PIL import Image, ImageDraw
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


def gauge(cv,pct,charging,phase,k,band_col,style,pattern='title'):
    assert style=='wide_high'
    x0,y0,x1,y1=(262,281,440,311)
    ink=PB.BASE.gauge_color(band_col,pct)
    cv.rect(x0,y0,x1,y1,'BLACK')
    left,right,top,bottom=(265,437,284,308)
    if pct is None:
        mid=(left+right)/2
        for x in (mid-20,mid+7):
            cv.rect(x,(top+bottom)/2-1,x+13,(top+bottom)/2+2,'GRAY')
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
        ('V1 / OUTLINED SOLID',PB.face('wide_high')),
        ('V2 / NO OUTLINE, SOLID',face()),
        ('V2 / UNKNOWN',face(pct=None)),
        ('V1 / 13-SLICE CHARGING',PB.face('wide_high',charging=True)),
        ('V2 / UNFRAMED, OLD DENSITY',face(charging=True,pattern='old')),
        ('V2 / TITLE-DERIVED DENSITY',face(charging=True)),
    ],3,'battery-outline-density-comparison.png')
    proportion_sheet()
    motion_sheet()
    charge_loop()
    board([(f'{st.upper()} / SOLID',face(st)) for st in BF.STATES]+[
          (f'{st.upper()} / CHARGING',face(st,charging=True)) for st in BF.STATES],
          3,'battery-unoutlined-all-states.png')
    board([(name,face(st,pct,charging)) for name,st,pct,charging in (
        ('GNSS / 12% SOLID','gnss',12,False),
        ('GNSS / 12% CHARGING','gnss',12,True),
        ('GNSS / UNKNOWN','gnss',None,False),
        ('NO ZONE / UNKNOWN','nozone',None,False),
        ('GNSS / ZERO','gnss',0,False),
        ('GNSS / FULL CHARGING','gnss',100,True),
    )],3,'battery-unoutlined-edge-states.png')
    face().save(HERE/'clock-unoutlined-solid.png',optimize=True)
    face(charging=True).save(HERE/'clock-unoutlined-charging.png',optimize=True)
    print('87% bars and gaps',tuple(map(len,title_pattern(round(172*.87),27))))

if __name__=='__main__':main()
