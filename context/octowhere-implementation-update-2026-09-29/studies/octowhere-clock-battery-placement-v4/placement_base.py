"""Wordmark-free K1 clock studies for placing the horizontal battery gauge."""
from pathlib import Path
from PIL import Image, ImageDraw
import clock_study_base as BASE
from face import SEC_PX, SEC_PEN

HERE=Path(__file__).resolve().parent
CK=BASE.CK
BF=BASE.BF
PANEL=BASE.PANEL
INK=BASE.INK
FONT=BASE.FONT

# External x0,y0,x1,y1. All candidates retain a one-pixel outline and a
# three-pixel margin to the measured-level fill.
BOXES={
    'low_right':(323,281,440,311),
    'top_right':(323,207,440,236),
    'mid_right':(365,242,440,276),
    'wide_low':(262,281,440,311),
    'wide_high':(262,281,440,311),
}
CAPTIONS={
    'low_right':'1 / LOWER RIGHT, CURRENT BAR',
    'top_right':'2 / UPPER RIGHT',
    'mid_right':'3 / MID RIGHT COLUMN',
    'wide_low':'4 / WIDE BAR, SECONDS MID',
    'wide_high':'5 / WIDE BAR, SECONDS HIGH',
}

def gauge(cv,pct,charging,phase,k,band_col,style):
    x0,y0,x1,y1=BOXES[style]
    col=BASE.gauge_color(band_col,pct)
    cv.rect(x0,y0,x1,y1,'BLACK')
    for a,b,c,d in ((x0+1,y0+1,x1-1,y0+2),(x0+1,y1-2,x1-1,y1-1),
                    (x0+1,y0+1,x0+2,y1-1),(x1-2,y0+1,x1-1,y1-1)):
        cv.rect(a,b,c,d,col)
    inner0,inner1=x0+3,x1-3
    upper,lower=y0+3,y1-3
    if pct is None:
        mid=(inner0+inner1)/2
        for x in (mid-20,mid+7):
            cv.rect(x,(upper+lower)/2-1,x+13,(upper+lower)/2+2,'GRAY')
        return
    length=round((inner1-inner0)*max(0,min(100,pct))/100*k)
    if length<=0:return
    if not charging:
        cv.rect(inner0,upper,inner0+length,lower,col)
        return
    if band_col=='RED':phase=27
    bands,gaps=BF.pattern_for_height(length,phase)
    x=inner0
    for i,width in enumerate(bands):
        cv.rect(x,upper,x+width,lower,col)
        x+=width+(gaps[i] if i<len(gaps) else 0)


def face(style,state='gnss',pct=87,charging=False,phase=27):
    orig=(CK.CC.wordmark,CK.battery_col,CK.digits,CK.C4.scatter,CK.lower)
    def bat(cv,p,c=False,ph=0,k=1.0,bc='LIME'):
        gauge(cv,p,c,ph,k,bc,style)
    def digits(cv,s,px,col,x0,base,name=None,clip=None):
        if style=='wide_low' and px==SEC_PX and x0==SEC_PEN:
            x0,base=388,273
        elif style=='wide_high' and px==SEC_PX and x0==SEC_PEN:
            x0,base=388,244
        return orig[2](cv,s,px,col,x0,base,name=name,clip=clip)
    def lower(cv,st,f,kd=1.0,kz=1.0):
        if st!='nodata':orig[4](cv,st,f,kd,kz)
    CK.CC.wordmark,CK.battery_col,CK.digits,CK.C4.scatter,CK.lower=(
        lambda cv,*args:None,bat,digits,BASE.k1_scatter,lower)
    try:
        fixture=dict(CK.F,bat=pct,charging=charging,phase=phase)
        image=CK.face(state,f=fixture,scatter_k=0 if state=='nodata' else 1)
        if state!='nodata':
            image=BASE.hour_rail(image,fixture['h'],active=state not in ('stopped','nozone'))
        return image
    finally:
        CK.CC.wordmark,CK.battery_col,CK.digits,CK.C4.scatter,CK.lower=orig


def board(items,columns,filename):
    rows=(len(items)+columns-1)//columns
    sh=Image.new('RGB',(columns*486+20,rows*516+20),PANEL)
    d=ImageDraw.Draw(sh)
    for i,(label,im) in enumerate(items):
        x=20+(i%columns)*486;y=20+(i//columns)*516
        sh.paste(im,(x,y))
        d.text((x+4,y+478),label,font=FONT,fill=INK)
    sh.save(HERE/filename,optimize=True)


def main():
    styles=tuple(BOXES)
    board([('K1 / VERTICAL GAUGE',BF.compose('gnss',charging=False))]+[
        (CAPTIONS[s],face(s)) for s in styles],3,
          'battery-placement-options.png')
    board([('K1 / VERTICAL CHARGING',BF.compose('gnss',charging=True))]+[
        (CAPTIONS[s]+' / CHG',face(s,charging=True)) for s in styles],3,
          'battery-placement-charging.png')
    for s in styles:
        face(s).save(HERE/f'clock-{s}-gnss.png',optimize=True)
    board([(BF.LABELS[st],face('wide_high',st)) for st in BF.STATES],3,
          'battery-wide-high-all-states.png')
    board([(label,face('wide_high',st,pct,chg)) for label,st,pct,chg in (
        ('GNSS / 12% SOLID','gnss',12,False),
        ('GNSS / 12% CHG','gnss',12,True),
        ('GNSS / UNKNOWN','gnss',None,False),
        ('NO ZONE / UNKNOWN','nozone',None,False),
        ('STOPPED / CHG','stopped',87,True),
        ('NO DATA / CHG','nodata',87,True),
    )],3,'battery-wide-high-edge-states.png')
    print('Rendered battery placement choices',HERE)

if __name__=='__main__':main()
