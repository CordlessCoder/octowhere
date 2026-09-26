"""G17 identity: unlocking grid entrance, framed type, reference-cadence flicker.

Run from renderer: PYTHONPATH=. python3 concept/startup_s1_g17.py OUTPUT_DIR
"""
from pathlib import Path
import math
import random
import sys

from PIL import Image, ImageChops, ImageDraw, ImageFont

import lib
lib.RECORD=False
from lib import Canvas, SS, rgb
from identity import finish
from anim import FRAME_MS, save_gif30
import identity_g2 as G2
import logo_card as LC
import marks_frame as MF
import startup30 as S30
from concept import startup_s1 as S1
from concept import startup_s1_g4 as G4
from concept import startup_s1_g11 as G11

OUT=Path(sys.argv[1]).resolve() if len(sys.argv)>1 else None
FRAMES=120
TYPE_START=430
GLYPH_INTERVAL=68
GLYPH_RISE=55
FLICKER_START=42
LOGO_START=56           # 14 frames after the title
GRAY=(43,46,50)
DIM_LIME=(65,91,7)
FAINT_PURPLE=(24,7,62)
BOOT_BLUE=(19,53,218)
BOOT_FONT=ImageFont.truetype('fonts/KHB.otf',38*SS)
_boot_box=BOOT_FONT.getbbox('BOOT')
_boot_mask=Image.new('L',(_boot_box[2]-_boot_box[0],_boot_box[3]-_boot_box[1]),0)
ImageDraw.Draw(_boot_mask).text((-_boot_box[0],-_boot_box[1]),'BOOT',font=BOOT_FONT,fill=255)
BOOT_MASK=_boot_mask.rotate(-90,expand=True)
MARK_TIMING={
    (-1, 1):(13,19,14,27,35), # bottom left: horizontal arm leads
    ( 1,-1):(43,16,23,37,29), # top right: vertical arm leads
    ( 1, 1):(27,23,28,35,31), # bottom right: vertical, then horizontal
    (-1,-1):(41,29,29,31,38), # top left: vertical, late horizontal
}
CARD_FRAMES=19


def state(rel):
    """2 on, 2 outline, 1 on, 2 outline, 2 on, 2 partial, then on."""
    if rel<0:return 'pre'
    if rel<2:return 'on'
    if rel<4:return 'off'
    if rel<5:return 'on'
    if rel<7:return 'off'
    if rel<9:return 'on'
    if rel<11:return 'partial'
    return 'on'


def title_ink(cv,kind,ms=0):
    mask,edge,left,top,bounds=G11.art()
    if kind=='type':
        alpha=Image.new('L',edge.size,0)
        for i in range(9):
            k=G4.ease((ms-TYPE_START-i*GLYPH_INTERVAL)/GLYPH_RISE)
            if k<=0:continue
            x0,x1=bounds[i],bounds[i+1]
            section=edge.crop((x0,0,x1,edge.height))
            if k<1:section=section.point(lambda v:round(v*k))
            alpha.paste(section,(x0,0))
        ink=alpha;col=DIM_LIME
    elif kind=='off':ink,col=edge,DIM_LIME
    elif kind=='on':ink,col=mask,'LIME'
    else:
        # The surviving glyph stems are literal slices of the undistorted mask.
        ink=Image.new('L',mask.size,0)
        for frac in (.055,.15,.29,.42,.55,.69,.82,.95):
            x=round(mask.width*frac)
            sl=mask.crop((x,0,min(x+8,mask.width),mask.height))
            ink.paste(sl,(x,0))
        col='LIME'
    cv.img.paste(Image.new('RGB',ink.size,rgb(col)),(left,top),ink)


def scatter(cv,n):
    # Stable sparse dot clusters with slight colour breathing, far below title
    # lightness. The logo reference uses a faint purple dot field, not a fill.
    rng=random.Random(11039)
    clusters=((300,130,89,65),(180,354,95,67),(402,244,45,76))
    pulse=(.68,.82,1.0,.75)[(n//11)%4]
    for cx,cy,rx,ry in clusters:
        for _ in range(120):
            x=rng.randrange(cx-rx,cx+rx);y=rng.randrange(cy-ry,cy+ry)
            d=((x-cx)/rx)**2+((y-cy)/ry)**2
            if d>1 or rng.random()>.43*(1-d):continue
            k=pulse*rng.choice((.40,.65,1.0))
            color=tuple(round(v*k) for v in FAINT_PURPLE)
            cv.rect(x,y,x+rng.choice((2,3,4)),y+rng.choice((2,3,4)),color)


def gray_marks(cv,n):
    if n<13:return
    hair=(34,37,40)
    for sx in (-1,1):
        for sy in (-1,1):
            h_at,v_at,dot_at,inner_h_at,inner_v_at=MARK_TIMING[(sx,sy)]
            cx=233+sx*145;cy=233+sy*145
            # The source squares move x, pause, then move y. Each 16 px move
            # settles over four frames with the reference's hard deceleration.
            x_in=(16,6,2,0)[min(3,max(0,n-21))]
            y_in=(16,6,2,0)[min(3,max(0,n-38))]
            qx=cx-sx*(40+x_in);qy=cy-sy*(34+y_in)
            # Both fragments register to the square corner facing the centre.
            corner_x=qx-sx*5;corner_y=qy-sy*5
            if inner_h_at<=n<55:
                xa=qx+sx*28;xb=qx+sx*10
                cv.rect(min(xa,xb),corner_y,max(xa,xb)+1,corner_y+1,hair)
            if inner_v_at<=n<58:
                vx=corner_x
                ya=qy+sy*10;yb=qy+sy*30
                cv.rect(vx,min(ya,yb),vx+1,max(ya,yb)+1,hair)
            if 21<=n<61:
                cv.rect(qx-5,qy-5,qx+5,qy+5,GRAY)
            if h_at<=n<64:
                cv.rect(cx-28,cy,cx+29,cy+1,hair)
            if v_at<=n<64:
                cv.rect(cx,cy-28,cx+1,cy+29,hair)
            if dot_at<=n<64:
                cv.poly([(cx,cy-2),(cx+2,cy),(cx,cy+2),(cx-2,cy)],GRAY)


def lime_ticks(cv,n):
    if n<36:return
    s=state(n-FLICKER_START)
    color='LIME' if s=='on' else DIM_LIME
    if s=='partial':color=(111,149,5)
    for x in (23,443):
        cv.rect(x-5,232,x+6,234,color)
        if s!='partial':cv.rect(x,228,x+2,238,color)


def small_logo(cv,n):
    s=state(n-LOGO_START)
    if s in ('pre','off'):return
    x0,y0,module=407,180,1.30
    for j,row in enumerate(MF.ROW['L1']):
        for i,bit in enumerate(row):
            if bit!='1' or (s=='partial' and i not in (0,7,14)):continue
            x=x0+i*module;y=y0+j*module
            cv.rect(x,y,x+module,y+module,'LIME')


def entry_grid(cv,n):
    # At 0:17.27–0:17.53 in the cinematic, a 3×4 array of solid blocks opens
    # a socket while each tongue travels right as a constant-height piece.
    # It becomes detached first, then meets the right neighbor at the end.
    # The source cuts hard from the completed grid into the next typographic shot.
    rng=random.Random(17420)
    for _ in range(185):
        x=rng.randrange(15,445)//8*8;y=rng.randrange(15,445)//8*8
        shade=rng.choice(((1,7,22),(2,11,29),(3,14,40)))
        cv.rect(x,y,x+rng.choice((8,16,24)),y+rng.choice((2,8)),shade)
    for y in (48,418):
        for x in (55,411):
            cv.rect(x-6,y,x+7,y+1,(7,28,63))
            cv.rect(x,y-6,x+1,y+7,(7,28,63))
    for row in range(4):
        for col in range(3):
            x=83+col*100;y=68+row*79
            phase=n-(4+row+col)
            colork=(.96,.82,.91,1.0)[row]
            ink=tuple(round(v*colork) for v in rgb('LIME'))
            cv.rect(x,y,x+48,y+48,ink)
            if phase==0:
                cv.rect(x+25,y+17,x+31,y+31,'BLACK')
            elif phase>=1:
                cv.rect(x+25,y+17,x+49,y+31,'BLACK')
            shift=0 if phase<=0 else (9 if phase==1 else 17 if phase==2 else 24 if phase==3 else 31)
            cv.rect(x+31+shift,y+17,x+69+shift,y+31,ink)
    if n>=2:
        shade=BOOT_BLUE if n>=4 else tuple(round(v*.55) for v in BOOT_BLUE)
        x=407*SS-BOOT_MASK.width//2;y=233*SS-BOOT_MASK.height//2
        cv.img.paste(Image.new('RGB',BOOT_MASK.size,shade),(x,y),BOOT_MASK)


def frame(n):
    ms=n*FRAME_MS
    cv=Canvas()
    if n<13:
        entry_grid(cv,n)
        return finish(cv)
    scatter(cv,n)
    gray_marks(cv,n)
    if n<FLICKER_START:title_ink(cv,'type',ms)
    else:title_ink(cv,state(n-FLICKER_START))
    if n>=21:G2.row(cv,min(99,n-20))
    lime_ticks(cv,n)
    small_logo(cv,n)
    return finish(cv)


def card(n):
    """3 striped / 6 dark-on-lime / 4 lime-on-dark / 4 enlarged /
    1 enlarged dark-on-lime / 1 black, matching the supplied impact cadence."""
    import marks_frame as MF
    if n==18:return Image.new('RGB',(466,466),'black')
    cv=Canvas()
    full=Image.new('L',cv.img.size,255)
    if n<9 or n==17:LC.paint(cv,full,'LIME')
    if n<3:
        # Solid, perforated silhouette for long vertical strokes through the
        # inner tile. The later full stage brings back its diagonal hatch.
        silhouette=MF.mask('L1')
        d=ImageDraw.Draw(silhouette)
        d.rectangle((170*SS,170*SS,296*SS-1,296*SS-1),fill=255)
        d.rectangle((195*SS,195*SS,271*SS-1,271*SS-1),fill=0)
        partial=ImageChops.multiply(silhouette,LC.stripes())
        LC.paint(cv,partial,'BLACK')
    elif n<9:LC.paint(cv,MF.hatched_mask('L1'),'BLACK')
    elif n<13:LC.paint(cv,MF.hatched_mask('L1'),'LIME')
    elif n<17:LC.paint(cv,MF.hatched_mask('L1',LC.POP_BY_MARK['L1H']),'LIME')
    else:LC.paint(cv,MF.hatched_mask('L1',LC.POP_BY_MARK['L1H']),'BLACK')
    return finish(cv)


def sequence():
    frames=[S1.post(n*FRAME_MS) for n in range(S30.LAST+S30.HOLD)]
    frames.extend(frame(n) for n in range(FRAMES))
    frames.extend(card(n) for n in range(CARD_FRAMES))
    frames.extend(S1.clock_entry(n) for n in range(1,17))
    frames.extend([frames[-1]]*18)
    return frames


def storyboard(path):
    picks=[
        ('GRID / SOLID',frame(1)),('GRID / OPENING',frame(6)),
        ('GRID / UNLOCKED',frame(12)),('OUTLINE / TYPE',frame(24)),
        ('GRAY MARKS OUT',frame(38)),('TITLE / 2 ON',frame(42)),
        ('TITLE / 2 OFF',frame(44)),('TITLE / 1 ON',frame(46)),
        ('TITLE / PARTIAL',frame(51)),('TITLE / SETTLED',frame(53)),
        ('LOGO / DELAYED',frame(57)),('LOGO / SETTLED',frame(69)),
        ('CARD / IMPACT',card(17)),('CLOCK / F2',S1.clock_entry(16)),
    ]
    cell,gap,cap,margin=233,14,30,20
    sh=Image.new('RGB',(4*cell+3*gap+2*margin,4*(cell+cap)+3*gap+2*margin),(24,24,27))
    d=ImageDraw.Draw(sh);f=ImageFont.truetype('fonts/MonoB.otf',14)
    for i,(label,im) in enumerate(picks):
        x=margin+(i%4)*(cell+gap);y=margin+(i//4)*(cell+cap+gap)
        sh.paste(im.resize((cell,cell),Image.Resampling.LANCZOS),(x,y))
        d.text((x,y+cell+6),label,font=f,fill=(214,220,226))
    sh.save(path)


def marks_storyboard(path):
    picks=((13,'BL / HORIZONTAL'),(21,'SQUARES / START'),
           (24,'SQUARES / X LOCK'),(29,'TL / VERTICAL'),
           (39,'SQUARES / Y MOVE'),(43,'TR / HORIZONTAL'))
    cell,cap,gap=466,30,14
    sh=Image.new('RGB',(3*cell+2*gap,2*(cell+cap)+gap),(24,24,27))
    d=ImageDraw.Draw(sh);f=ImageFont.truetype('fonts/MonoB.otf',17)
    for i,(n,label) in enumerate(picks):
        x=(i%3)*(cell+gap);y=(i//3)*(cell+cap+gap)
        sh.paste(frame(n),(x,y));d.text((x+12,y+cell+5),label,font=f,fill=(214,220,226))
    sh.save(path)


def card_storyboard(path):
    picks=((1,'3 / STRIPED'),(5,'6 / DARK ON LIME'),
           (10,'4 / LIME ON DARK'),(14,'4 / ENLARGED'),
           (17,'1 / INVERTED'),(18,'1 / BLACK'))
    cell,cap,gap=233,30,14
    sh=Image.new('RGB',(3*cell+2*gap,2*(cell+cap)+gap),(24,24,27))
    d=ImageDraw.Draw(sh);f=ImageFont.truetype('fonts/MonoB.otf',13)
    for i,(n,label) in enumerate(picks):
        x=(i%3)*(cell+gap);y=(i//3)*(cell+cap+gap)
        sh.paste(card(n).resize((cell,cell),Image.Resampling.LANCZOS),(x,y))
        d.text((x+4,y+cell+6),label,font=f,fill=(214,220,226))
    sh.save(path)


if __name__=='__main__':
    OUT.mkdir(parents=True,exist_ok=True)
    storyboard(OUT/'startup-S1-G17-storyboard.png')
    marks_storyboard(OUT/'identity-G17-marks-timing.png')
    card_storyboard(OUT/'identity-G17-card-timing.png')
    identity=[frame(n) for n in range(FRAMES)]
    save_gif30(identity,str(OUT/'identity-G17-unlocking-grid.gif'))
    for n,label in ((1,'solid'),(6,'opening'),(12,'unlocked'),(24,'type'),(42,'filled'),(44,'outline'),(51,'partial'),(57,'logo'),(69,'settled')):
        identity[n].save(OUT/f'identity-G17-{label}.png')
    identity[40].save(OUT/'identity-G17-framing.png')
    identity[40].crop((49,282,190,421)).resize((564,556),Image.Resampling.NEAREST).save(OUT/'identity-G17-gray-marks-detail.png')
    save_gif30([card(n) for n in range(CARD_FRAMES)],str(OUT/'identity-G17-card-impact.gif'))
    save_gif30(sequence(),str(OUT/'startup-S1-G17-success.gif'))
