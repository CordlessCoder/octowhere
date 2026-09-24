"""Picker step 1 in each candidate treatment, for choosing how 'choosing' looks."""
import lib
from lib import *
import picker
from picker import step1, button, BTN_X, BTN_TOP, HINT_TOP, NB_ABOVE, NB_BELOW, AUTO_X, AUTO_Y
from face import X0, BAND

lib.RECORD = False
ZONE_GLYPH = '01110 10101 11111 10101 01110'   # a globe, for the zone setting


def lum(c):
    def ch(v):
        v /= 255
        return v / 12.92 if v <= 0.03928 else ((v + 0.055) / 1.055) ** 2.4
    r, g, b = rgb(c)
    return 0.2126 * ch(r) + 0.7152 * ch(g) + 0.0722 * ch(b)


def contrast_black(c):
    return (lum(c) + 0.05) / 0.05


def coloured(col):
    picker.EDIT = col
    return step1(auto=True).render()


def white_with_icon():
    """WHITE band, and the setting named by a symbol and a label in the top cap."""
    cv = Canvas()
    cv.ring(230, 232, 'GRAY')
    icon_sized(cv, 150, 60, 'WHITE', ZONE_GLYPH, module=8, pad=4)       # 48 x 48
    cv.text('ZONE', 'bold', 24, 'WHITE', x=210, y=62)
    cv.text('DRAG TO LOCAL TIME', 'mono', 14, 'GRAY', x=210, y=96)
    cv.rect(0, BAND[0], N, BAND[1], 'WHITE')
    hhmm(cv, '13:07', 86, 'BLACK', X0 - 5, 289, 28)
    cv.text('UTC', 'bold', 16, 'BLACK', x=311, y=228)
    cv.text('+01:00', 'bold', 24, 'BLACK', x=311, bottom=289)
    cv.text('12:07  +00:00', 'mono', 23, 'GRAY', x=X0, cy=NB_ABOVE)
    cv.text('14:07  +02:00', 'mono', 23, 'GRAY', x=X0, cy=NB_BELOW)
    button(cv, AUTO_X[0], AUTO_Y[0], AUTO_X[1], AUTO_Y[1], 'AUTO', False, 'auto')
    return cv.render()


def outlined():
    """The band as an open field: WHITE rules above and below, WHITE text on BLACK.
    Filled means reading, outlined means editing."""
    cv = Canvas()
    cv.ring(230, 232, 'GRAY')
    cv.text('DRAG TO YOUR LOCAL TIME', 'mono', 14, 'GRAY', cx=233, y=HINT_TOP)
    button(cv, BTN_X[0], BTN_TOP[0], BTN_X[1], BTN_TOP[1], 'CANCEL', False, 'top')
    cv.rect(0, BAND[0], N, BAND[0] + 4, 'WHITE')
    cv.rect(0, BAND[1] - 4, N, BAND[1], 'WHITE')
    # stepped corner locators at the text column, marking the field's editable span
    for x in (X0 - 14, 400):
        cv.rect(x, BAND[0] + 4, x + 3, BAND[0] + 16, 'WHITE')
        cv.rect(x, BAND[1] - 16, x + 3, BAND[1] - 4, 'WHITE')
    hhmm(cv, '13:07', 86, 'WHITE', X0 - 5, 289, 28)
    cv.text('UTC', 'bold', 16, 'GRAY', x=311, y=228)
    cv.text('+01:00', 'bold', 24, 'WHITE', x=311, bottom=289)
    cv.text('12:07  +00:00', 'mono', 23, 'GRAY', x=X0, cy=NB_ABOVE)
    cv.text('14:07  +02:00', 'mono', 23, 'GRAY', x=X0, cy=NB_BELOW)
    button(cv, AUTO_X[0], AUTO_Y[0], AUTO_X[1], AUTO_Y[1], 'AUTO', False, 'auto')
    return cv.render()


opts = [
    (coloured('#ECDB0B'), 'YELLOW #ECDB0B', '#ECDB0B'),
    (coloured('#C0FE04'), 'LIME #C0FE04 (TOKEN)', '#C0FE04'),
    (coloured('#01E67C'), 'GREEN #01E67C', '#01E67C'),
    (coloured('#81EBB1'), 'PALE GREEN #81EBB1', '#81EBB1'),
    (coloured('#B32BE5'), 'VIOLET #B32BE5', '#B32BE5'),
    (coloured('#E8337C'), 'DEEP PINK #E8337C', '#E8337C'),
    (white_with_icon(), 'WHITE + SYMBOL', 'WHITE'),
    (outlined(), 'OUTLINED FIELD', None),
]
cols = 4
W, Hh = 466 + 24, 466 + 64
sh = Image.new('RGB', (cols * W + 24, 2 * Hh + 24), (28, 28, 28))
d = ImageDraw.Draw(sh)
f = ImageFont.truetype('fonts/MonoR.otf', 16)
for i, (im, label, col) in enumerate(opts):
    x, y = 24 + (i % cols) * W, 24 + (i // cols) * Hh
    sh.paste(im, (x, y))
    d.text((x, y + 474), label, font=f, fill=(210, 211, 214))
    if col:
        d.text((x, y + 496), 'BLACK TEXT CONTRAST %.1f:1' % contrast_black(col), font=f, fill=(136, 142, 152))
sh.save('out/study-picker-treatment.png')
for c in ['#ECDB0B', '#C0FE04', '#01E67C', '#81EBB1', '#B32BE5', '#E8337C', 'WHITE', 'ORANGE', 'RED', 'BLUE', '#FFFAC3']:
    print(c, round(contrast_black(c), 1))
