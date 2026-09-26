"""Zone picker: step 1 picks the offset by local time, step 2 picks the zone."""
from lib import *
from face import X0, BAND

EDIT = 'YELLOW'          # proposed token for choosing/editing
BAND_MID = (BAND[0] + BAND[1]) / 2   # 257.5
NB_ABOVE = 174           # neighbour row ink centre
NB_BELOW = 341
HINT_TOP = 48
BTN_TOP = (68, 108)      # CANCEL / BACK box rows
BTN_X = (173, 293)
AUTO_Y = (366, 406)
AUTO_X = (183, 283)


def button(cv, x0, y0, x1, y1, label, filled, name):
    if filled:
        cv.rect(x0, y0, x1, y1, 'WHITE', name=name)
        cv.text(label, 'bold', 16, 'BLACK', cx=(x0 + x1) / 2, cy=(y0 + y1) / 2, name=name + '-label')
    else:
        cv.rect(x0, y0, x1, y1, 'GRAY', name=name)
        cv.rect(x0 + 2, y0 + 2, x1 - 2, y1 - 2, 'BLACK')
        cv.text(label, 'bold', 16, 'WHITE', cx=(x0 + x1) / 2, cy=(y0 + y1) / 2, name=name + '-label')


def frame(cv, hint, top_label):
    cv.ring(230, 232, 'GRAY', name='ring')
    cv.text(hint, 'mono', 14, 'GRAY', cx=233, y=HINT_TOP, name='hint')
    button(cv, *BTN_X[:1], BTN_TOP[0], BTN_X[1], BTN_TOP[1], top_label, False, 'top-button')
    cv.rect(0, BAND[0], N, BAND[1], EDIT, name='band')


def step1(auto=True, time_known=True):
    cv = Canvas()
    frame(cv, 'DRAG TO YOUR LOCAL TIME', 'CANCEL')
    t = lambda s: s if time_known else '--:--'
    # selected row: local time at the offset, and the offset
    hhmm(cv, t('13:07'), 86, 'BLACK', X0 - 5, 289, 28, name='sel-time')
    cv.text('UTC', 'bold', 16, 'BLACK', x=311, y=228, name='sel-utc')
    cv.text('+01:00', 'bold', 24, 'BLACK', x=311, bottom=289, name='sel-offset')
    cv.text(t('12:07') + '  +00:00', 'mono', 23, 'GRAY', x=X0, cy=NB_ABOVE, name='nb-above')
    cv.text(t('14:07') + '  +02:00', 'mono', 23, 'GRAY', x=X0, cy=NB_BELOW, name='nb-below')
    button(cv, AUTO_X[0], AUTO_Y[0], AUTO_X[1], AUTO_Y[1], 'AUTO', auto, 'auto')
    return cv


def step2():
    cv = Canvas()
    frame(cv, '+01:00  NEAREST FIRST', 'BACK')
    cv.text('DUBLIN', 'bold', 40, 'BLACK', x=X0, bottom=258, name='sel-city')
    cv.text('EUROPE/DUBLIN  IST', 'bold', 16, 'BLACK', x=X0, y=276, name='sel-iana')
    cv.text('LONDON', 'mono', 23, 'GRAY', x=X0, cy=NB_BELOW, name='nb-below')
    cv.text('1/26', 'mono', 14, 'GRAY', cx=233, y=381, name='position')
    return cv


def step2_sea():
    """The Etc zone of the same offset, at the end of the list."""
    cv = Canvas()
    frame(cv, '+01:00  NEAREST FIRST', 'BACK')
    cv.text('AT SEA', 'bold', 40, 'BLACK', x=X0, bottom=258, name='sel-city')
    cv.text('ETC/GMT-1  +01', 'bold', 16, 'BLACK', x=X0, y=276, name='sel-iana')
    cv.text('PORTO-NOVO', 'mono', 23, 'GRAY', x=X0, cy=NB_ABOVE, name='nb-above')
    cv.text('26/26', 'mono', 14, 'GRAY', cx=233, y=381, name='position')
    return cv


if __name__ == '__main__':
    for n, fn in [('picker-1-offset', step1), ('picker-2-zone', step2), ('picker-2-sea', step2_sea)]:
        cv = fn()
        cv.save('out/%s.png' % n)
        print(n)
        for k, b in cv.log:
            print('  ', k, b)
