"""G9: center the title and microtext after removing the right-hand weight.

Run from renderer: PYTHONPATH=. python3 concept/startup_s1_g9.py OUTPUT_DIR
"""
from pathlib import Path
import sys

from PIL import Image, ImageDraw, ImageFont

import identity_g2 as G2
import logo_card as LC
from anim import save_gif30
from concept import startup_s1 as S1
from concept import startup_s1_g8 as G8

OUT=Path(sys.argv[1]).resolve() if len(sys.argv)>1 else None

# The original title and microtext both span x=34..360 (about 326 px).
# Moving their common left edge 36 px centers both at x=233. The original
# fonts, sizes, row contents, and vertical positions are untouched.
G2.LEFT=70


def frame(n):
    return G8.frame(n)


def sequence():
    return G8.sequence()


def storyboard(path):
    picks=[
        ('S1 / SELF TEST',S1.post(800)),
        ('FIRST OUTLINE',frame(20)),
        ('HALF TYPED',frame(41)),
        ('ALL OUTLINED',frame(66)),
        ('FILLED POP',frame(73)),
        ('CENTERED HOLD',frame(113)),
        ('HARD CUT TO CARD',LC.card(0)),
        ('CARD / INVERTED',LC.card(8)),
        ('CLOCK / F2',S1.clock_entry(16)),
    ]
    cell,gap,cap,margin=233,14,30,20
    sh=Image.new('RGB',(3*cell+2*gap+2*margin,3*(cell+cap)+2*gap+2*margin),(24,24,27))
    d=ImageDraw.Draw(sh);f=ImageFont.truetype('fonts/MonoB.otf',14)
    for i,(label,im) in enumerate(picks):
        x=margin+(i%3)*(cell+gap);y=margin+(i//3)*(cell+cap+gap)
        sh.paste(im.resize((cell,cell),Image.Resampling.LANCZOS),(x,y))
        d.text((x,y+cell+6),label,font=f,fill=(214,220,226))
    sh.save(path)


if __name__=='__main__':
    OUT.mkdir(parents=True,exist_ok=True)
    storyboard(OUT/'startup-S1-G9-centered-storyboard.png')
    identity=[frame(n) for n in range(G8.FRAMES)]
    save_gif30(identity,str(OUT/'identity-G9-centered.gif'))
    identity[66].save(OUT/'identity-G9-outlined.png')
    identity[113].save(OUT/'identity-G9-filled.png')
    save_gif30(sequence(),str(OUT/'startup-S1-G9-success.gif'))
