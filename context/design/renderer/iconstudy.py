from lib import *
from face import SYM, digits, X0, BIG, HOURS_BASE, MIN_BASE, SEC_PEN, SEC_PX

def base(cv):
    cv.ring(230, 232, 'GRAY')
    digits(cv, '13', BIG, 'WHITE', X0, HOURS_BASE)
    cv.rect(0, 198, N, 318, 'WHITE')
    digits(cv, '07', BIG, 'BLACK', X0, MIN_BASE)
    digits(cv, '42', SEC_PX, 'BLACK', SEC_PEN, MIN_BASE)
    cv.text('THU 24 SEP 2026', 'mono', 23, 'WHITE', cx=233, y=334)
    cv.text('AUTO  IST  +01:00', 'mono', 14, 'GRAY', cx=233, y=361)
    cv.text('EUROPE/DUBLIN', 'mono', 14, 'GRAY', cx=233, y=377)

out = []
for module, pad, label in [(5, 4, '33 PX  M5'), (8, 6, '52 PX  M8'), (10, 8, '66 PX  M10'), (16, 8, '96 PX  M16')]:
    cv = Canvas(); base(cv)
    s = icon_sized(cv, 262, 90, 'BLUE', SYM['gnss'], module, pad)
    if s < 90:
        cv.text('LOCAL', 'bold', 16, 'GRAY', x=262, bottom=184)
    else:
        cv.text('LOCAL', 'bold', 16, 'BLACK', x=262, y=214)
    im = cv.render()
    out.append((im, label))
sh = Image.new('RGB', (486 * 4 - 20, 500), (28, 28, 28)); d = ImageDraw.Draw(sh)
f = ImageFont.truetype('fonts/MonoR.otf', 16)
for i, (im, l) in enumerate(out):
    sh.paste(im, (i * 486, 0)); d.text((i * 486, 476), 'ICON ' + l, font=f, fill=(136, 142, 152))
sh.save('out/study-icon-size.png')
