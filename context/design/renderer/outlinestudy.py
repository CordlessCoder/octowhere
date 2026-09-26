"""All status icons filled vs outlined: the clock face at 96 px and the compass at 33 px."""
from lib import *
from face import SYM
import industrial

COMPASS = [('BLUE', '00100 01110 10101 00100 00100'), ('ORANGE', '01110 10001 10000 10001 01110'),
           ('ORANGE', '10101 01110 11011 01110 10101'), ('WHITE', '11111 00100 00100 00100 00100'),
           ('RED', '11100 11000 00100 00011 00111')]


def tile(cv, x, y, col, rows, module, pad, frame, outlined):
    size = 5 * module + 2 * pad
    cv.rect(x, y, x + size, y + size, col)
    if outlined:
        cv.rect(x + frame, y + frame, x + size - frame, y + size - frame, 'BLACK')
    for j, row in enumerate(rows.split()):
        for i, b in enumerate(row):
            if b == '1':
                cv.rect(x + pad + module * i, y + pad + module * j, x + pad + module * (i + 1),
                        y + pad + module * (j + 1), col if outlined else 'BLACK')


# face states with outlined icons: monkeypatch the build helper
_orig = industrial.icon_build


def outlined_build(cv, x, y, tile_col, sym, n, module=16, pad=8):
    tile(cv, x, y, tile_col, sym, module, pad, 4, True)


industrial.icon_build = outlined_build
out = [industrial.v1(s).render() for s in ['gnss', 'rtc', 'stopped', 'nozone', 'nodata']]
industrial.icon_build = _orig
filled = [industrial.v1(s).render() for s in ['gnss', 'rtc', 'stopped', 'nozone', 'nodata']]

f = ImageFont.truetype('fonts/MonoR.otf', 16)
sh = Image.new('RGB', (486 * 5 - 20, 2 * 500 + 190), (28, 28, 28))
d = ImageDraw.Draw(sh)
for i, im in enumerate(filled):
    sh.paste(im, (i * 486, 0))
d.text((0, 474), 'FILLED (CURRENT)', font=f, fill=(210, 211, 214))
for i, im in enumerate(out):
    sh.paste(im, (i * 486, 500))
d.text((0, 974), 'OUTLINED, 4 PX FRAME', font=f, fill=(210, 211, 214))
# compass icons at 33 px, filled and outlined with a 1 px and 2 px frame, shown 3x
y0 = 1010
for r, (label, outl, fr) in enumerate([('COMPASS 33 PX FILLED', False, 0), ('OUTLINED 1 PX', True, 1), ('OUTLINED 2 PX', True, 2)]):
    for i, (col, rows) in enumerate(COMPASS):
        cv = Canvas()
        tile(cv, 216, 216, col, rows, 5, 4, fr, outl)
        im = cv.img.resize((466, 466), Image.BOX).crop((212, 212, 253, 253)).resize((123, 123), Image.NEAREST)
        sh.paste(im, (220 + (r * 5 + i) * 130 - (r * 5 * 130) + r * 700 - 220 + 20 + i * 0, y0)) if False else None
        sh.paste(im, (20 + r * 780 + i * 140 // 1, y0 + 20)) if False else None
        sh.paste(im, (20 + r * 790 + i * 150 // 1 - (i * 150 - i * 140), y0 + 24))
    d.text((20 + r * 790, y0), label + ' (SHOWN 3X)', font=f, fill=(210, 211, 214))
sh.save('out/study-outlined-icons.png')
print(sh.size)
