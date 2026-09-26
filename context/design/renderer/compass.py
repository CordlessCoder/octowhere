"""The compass redrawn from the brief's table, in every state, with the icon as a parameter.

Constraint for a bigger icon: N E S W orbit on radius 172 at 40 px, so their inner edge is
near radius 157. Everything in the centre stack keeps its corners inside radius 150.
"""
import math
from lib import *

ICONS = {
    'heading': ('BLUE', '00100 01110 10101 00100 00100'),
    'calibrating': ('ORANGE', '01110 10001 10000 10001 01110'),
    'interference': ('ORANGE', '10101 01110 11011 01110 10101'),
    'topedge': ('WHITE', '11111 00100 00100 00100 00100'),
    'nodata': ('RED', '11100 11000 00100 00011 00111'),
}

# vertical layouts: icon module/pad and the rows everything below it moves to
LAYOUTS = {
    # the approved compass
    'current': dict(module=5, pad=4, icon_top=102, caption=142, state_mid=169.5, slab=186, tilt=293, div=321, hint=328),
    # 52 px icon, the stack shifted down 5 px
    'icon52': dict(module=8, pad=6, icon_top=88, caption=147, state_mid=174.5, slab=191, tilt=298, div=326, hint=333),
    # 66 px icon, the stack shifted down 18 px, divider tightened
    'icon66': dict(module=10, pad=8, icon_top=87, caption=160, state_mid=187.5, slab=204, tilt=311, div=336, hint=342),
}


def dial(cv, heading):
    for k in range(36):
        a = k * 10 - heading
        if k % 3 == 0:
            cv.radial_bar(a, 202, 226, 4, 'WHITE')
        else:
            cv.radial_bar(a, 216, 226, 2, 'GRAY')
    for k, L in enumerate('NESW'):
        rtext(cv, L, 'shapiro', 40, 'ORANGE' if L == 'N' else 'WHITE', k * 90 - heading, 172)


def readout(cv, text, suffix, slab_top):
    """Tabular Bold 86 digits with the ink of the first at x 149, suffix at 40 px top-aligned."""
    a = adv('bold', 86)
    x0 = 149 - lsb('0', 'bold', 86)
    base = slab_top + 78
    for i, ch in enumerate(text):
        pen(cv, ch, 'bold', 86, 'BLACK', x0 + i * a, base)
    if suffix:
        pen(cv, suffix, 'bold', 40, 'BLACK', x0 + 3 * a + 2, base - 22)


def compass(state, layout='current', outlined=True, heading=47):
    L = LAYOUTS[layout]
    cv = Canvas()
    fault = state == 'nodata'
    cv.ring(230, 232, 'RED' if fault else 'GRAY', name='ring')
    if state in ('heading', 'interference'):
        dial(cv, heading)
    tile, rows = ICONS[state]
    size = 5 * L['module'] + 2 * L['pad']
    icon_sized(cv, round(233 - size / 2), L['icon_top'], tile, rows, module=L['module'], pad=L['pad'],
               outlined=outlined, name='icon')
    cap = {'calibrating': ('CALIBRATION', 'ORANGE'), 'nodata': ('COMPASS', 'GRAY')}.get(state, ('MAGNETIC', 'GRAY'))
    cv.text(cap[0], 'bold', 16, cap[1], cx=233, y=L['caption'], name='caption')
    st = {'interference': ('INTERFERENCE', 'bold', 24, 'ORANGE'), 'calibrating': ('TURN ALL WAYS', 'mono', 19, 'GRAY'),
          'topedge': ('TOP EDGE UP', 'mono', 20, 'GRAY')}.get(state)
    if st:
        cv.text(st[0], st[1], st[2], st[3], cx=233, cy=L['state_mid'], name='state')
    slab_col = {'heading': 'WHITE', 'topedge': 'WHITE', 'nodata': 'RED'}.get(state, 'ORANGE')
    s0 = L['slab']
    cv.rect(135, s0, 331, s0 + 91, slab_col, name='slab')
    if fault:
        cv.text('NO DATA', 'shapiro', 28, 'BLACK', cx=233, cy=s0 + 45.5)
        return cv
    if state == 'topedge':
        cv.text('---', 'bold', 86, 'BLACK', cx=233, cy=s0 + 45.5)
    elif state == 'calibrating':
        readout(cv, '054', '%', s0)
    else:
        readout(cv, '047', '°', s0)
    cv.text('P +05  R -12', 'mono', 23, 'GRAY', cx=233, y=L['tilt'], name='tilt')
    cv.rect(138, L['div'], 329, L['div'] + 1, 'GRAY', name='divider')
    cv.text('COVER SCREEN TO RECAL', 'mono', 14, 'GRAY', cx=233, y=L['hint'], name='hint')
    return cv


def clearance(cv):
    """Largest corner radius of the centre stack's elements."""
    worst = (0, "")
    for n, b in cv.log:
        if b is None or n == 'ring':
            continue
        x0, y0, x1, y1 = b
        r = max(math.hypot(x + 0.5 - C, y + 0.5 - C) for x in (x0, x1) for y in (y0, y1))
        worst = max(worst, (r, n))
    return worst


if __name__ == '__main__':
    states = ['heading', 'interference', 'calibrating', 'topedge', 'nodata']
    variants = [('current', False, 'APPROVED: 33 PX FILLED'), ('current', True, '33 PX OUTLINED'),
                ('icon52', True, '52 PX OUTLINED, STACK DOWN 5 PX'), ('icon66', True, '66 PX OUTLINED, STACK DOWN 18 PX')]
    f = ImageFont.truetype('fonts/MonoR.otf', 16)
    sh = Image.new('RGB', (486 * 5 - 20, len(variants) * 510), (28, 28, 28))
    d = ImageDraw.Draw(sh)
    for r, (lay, outl, label) in enumerate(variants):
        worst = (0, '')
        for c, st in enumerate(states):
            cv = compass(st, lay, outl)
            worst = max(worst, clearance(cv))
            im = cv.render()
            sh.paste(im, (c * 486, r * 510))
            if lay != 'current' or outl:
                im.save('out/compass-%s-%s.png' % (lay if outl else lay + '-filled', st))
        d.text((0, r * 510 + 476), '%s   STACK CORNERS REACH RADIUS %.0f (%s)' % (label, worst[0], worst[1]), font=f,
               fill=(210, 211, 214))
    sh.save('out/study-compass-icon.png')
