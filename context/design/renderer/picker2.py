"""Zone picker, outlined-field treatment, with the 96 px symbol in the face's icon slot.

Rule: filled means reading, outlined means editing. The band opens into two 4 px WHITE
rules with WHITE text on BLACK; the icon can follow the same rule (outlined tile).
"""
from lib import *
from face import X0, BAND

ZONE = '11011 10001 00100 10001 11011'   # offset bracket: the zone setting
ICON_XY = (262, 90)
HINT_TOP = 48
BTN = (72, 104, 182, 143)                 # CANCEL / BACK box, upper left
NB_ABOVE, NB_BELOW = 174, 341
AUTO = (183, 366, 282, 405)
RULE = 4


def box(cv, x0, y0, x1, y1, label, filled):
    if filled:          # a mode in force: GRAY fill, as the face's plate tag
        cv.rect(x0, y0, x1, y1, 'GRAY', name='btn')
        cv.text(label, 'bold', 16, 'BLACK', cx=(x0 + x1) / 2, cy=(y0 + y1) / 2)
    else:
        cv.rect(x0, y0, x1, y1, 'GRAY', name='btn')
        cv.rect(x0 + 2, y0 + 2, x1 - 2, y1 - 2, 'BLACK')
        cv.text(label, 'bold', 16, 'WHITE', cx=(x0 + x1) / 2, cy=(y0 + y1) / 2)


def zone_icon(cv, outlined, rows_shown=5):
    icon_sized(cv, *ICON_XY, 'WHITE', ZONE, module=16, pad=8, outlined=outlined, rows_shown=rows_shown)


def field(cv):
    """The outlined band: rules on its top and bottom rows, locators at the text column."""
    clipped_band(cv, BAND[0], BAND[0] + RULE, 'WHITE', name='rule-top')
    clipped_band(cv, BAND[1] - RULE, BAND[1], 'WHITE', name='rule-bottom')
    for x in (X0 - 14, 400):
        cv.rect(x, BAND[0] + RULE, x + 3, BAND[0] + 16, 'WHITE')
        cv.rect(x, BAND[1] - 16, x + 3, BAND[1] - RULE, 'WHITE')


def step1(auto=True, time_known=True, outlined_icon=True):
    cv = Canvas()
    cv.ring(230, 232, 'GRAY', name='ring')
    cv.text('DRAG TO YOUR LOCAL TIME', 'mono', 14, 'GRAY', cx=233, y=HINT_TOP, name='hint')
    box(cv, *BTN, 'CANCEL', False)
    zone_icon(cv, outlined_icon)
    t = (lambda s: s) if time_known else (lambda s: '--:--')
    field(cv)
    hhmm(cv, t('13:07'), 86, 'WHITE', X0 - 5, 289, 28, name='sel-time')
    cv.text('UTC', 'bold', 16, 'GRAY', x=311, y=228, name='sel-utc')
    cv.text('+01:00', 'bold', 24, 'WHITE', x=311, bottom=289, name='sel-offset')
    cv.text(t('12:07') + '  +00:00', 'mono', 23, 'GRAY', x=X0, cy=NB_ABOVE, name='nb-above')
    cv.text(t('14:07') + '  +02:00', 'mono', 23, 'GRAY', x=X0, cy=NB_BELOW, name='nb-below')
    box(cv, *AUTO, 'AUTO', auto)
    return cv


def step2(sea=False, outlined_icon=True):
    cv = Canvas()
    cv.ring(230, 232, 'GRAY', name='ring')
    cv.text('+01:00  NEAREST FIRST', 'mono', 14, 'GRAY', cx=233, y=HINT_TOP, name='hint')
    box(cv, *BTN, 'BACK', False)
    zone_icon(cv, outlined_icon)
    field(cv)
    if sea:
        cv.text('AT SEA', 'bold', 40, 'WHITE', x=X0, bottom=258, name='sel-city')
        cv.text('ETC/GMT-1  +01', 'bold', 16, 'GRAY', x=X0, y=276, name='sel-iana')
        cv.text('PORTO-NOVO', 'mono', 23, 'GRAY', x=X0, cy=NB_ABOVE, name='nb-above')
        pos = '26/26'
    else:
        cv.text('DUBLIN', 'bold', 40, 'WHITE', x=X0, bottom=258, name='sel-city')
        cv.text('EUROPE/DUBLIN  IST', 'bold', 16, 'GRAY', x=X0, y=276, name='sel-iana')
        cv.text('LONDON', 'mono', 23, 'GRAY', x=X0, cy=NB_BELOW, name='nb-below')
        pos = '1/26'
    cv.text(pos, 'mono', 14, 'GRAY', cx=233, y=381, name='position')
    return cv


if __name__ == '__main__':
    a = step1(outlined_icon=False).render()
    b = step1(outlined_icon=True).render()
    sh = Image.new('RGB', (486 * 2 - 20, 500), (28, 28, 28))
    d = ImageDraw.Draw(sh)
    f = ImageFont.truetype('fonts/MonoR.otf', 16)
    for i, (im, l) in enumerate([(a, 'FILLED WHITE TILE'), (b, 'OUTLINED TILE')]):
        sh.paste(im, (i * 486, 0))
        d.text((i * 486, 476), l, font=f, fill=(210, 211, 214))
    sh.save('out/study-picker-icon.png')
