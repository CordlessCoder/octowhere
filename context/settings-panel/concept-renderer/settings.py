"""Settings panel, first pass.

Fixture: zone automatic, IST; brightness 120 of 255 (47 %); compass calibration 54 %;
firmware 0.1.0; battery 87 % (synthetic: battery does not reach the screens yet);
GNSS fix with 9 satellites in use (synthetic, grade 2).
"""
from lib import *
from face import X0, BAND
from picker2 import box, field, BTN, HINT_TOP, ICON_XY

GLYPH = {
    'zone': '11011 10001 00100 10001 11011',     # offset bracket, as the picker
    'bright': '00001 00011 00111 01111 11111',   # a rising ramp: level
    'cal': '01110 10001 10000 10001 01110',      # the compass's calibrating glyph
    'device': '00100 00000 01100 00100 01110',   # i
    'clear': '10001 01010 00100 01010 10001',    # a cross: erase
}

# 2 x 2 grid: 96 px tiles, 24 px gutter, symmetric about x 233
TX = (125, 245)
TY = (84, 232)
NAME_GAP, VALUE_GAP = 8, 6


def tile(cv, x, y, key, colour, name, value, value_col='WHITE', tag=None):
    icon_sized(cv, x, y, colour, GLYPH[key], module=16, pad=8, name='tile-' + key)
    cx = x + 48
    cv.text(name, 'bold', 14, 'GRAY', cx=cx, y=y + 96 + NAME_GAP, name='name-' + key)
    vy = y + 96 + NAME_GAP + 10 + VALUE_GAP
    if tag:
        # a mode in force is filled GRAY, as the face's plate tag
        w_tag = adv('bold', 14) * len(tag) + 10
        w_val = cv.ink_size(value, 'mono', 16)[0]
        total = w_tag + 6 + w_val
        x0 = round(cx - total / 2)
        cv.rect(x0, vy - 3, round(x0 + w_tag), vy + 15, 'GRAY')
        cv.text(tag, 'bold', 14, 'BLACK', cx=x0 + w_tag / 2, cy=vy + 6)
        cv.text(value, 'mono', 16, value_col, x=x0 + w_tag + 6, cy=vy + 6, name='value-' + key)
    else:
        cv.text(value, 'mono', 16, value_col, cx=cx, cy=vy + 6, name='value-' + key)


def panel(cal=54, bright=47, zone_mode='AUTO', abbr='IST'):
    cv = Canvas()
    cv.ring(230, 232, 'GRAY', name='ring')
    cv.text('SETTINGS', 'shapiro', 16, 'WHITE', cx=233, y=50, name='title')
    tile(cv, TX[0], TY[0], 'zone', 'WHITE', 'ZONE', abbr, tag=zone_mode)
    tile(cv, TX[1], TY[0], 'bright', 'WHITE', 'BRIGHTNESS', '%d%%' % bright)
    tile(cv, TX[0], TY[1], 'cal', 'ORANGE' if cal < 100 else 'WHITE', 'COMPASS', 'CAL %d%%' % cal,
         value_col='ORANGE' if cal < 100 else 'WHITE')
    tile(cv, TX[1], TY[1], 'device', 'WHITE', 'DEVICE', '0.1.0')
    cv.text('DRAG UP TO CLOSE', 'mono', 14, 'GRAY', cx=233, y=398, name='hint')
    return cv


# ---- brightness editor: the picker's layout, a stepped track in the field ---------------------
STEPS = 10          # 10 % to 100 %, level = round(255 * step / 10)
TRACK = (66, 276, 400, 300)    # x0, y0, x1, y1


def brightness(level_pct=50, dragging=False):
    cv = Canvas()
    cv.ring(230, 232, 'GRAY', name='ring')
    cv.text('DRAG TO SET BRIGHTNESS', 'mono', 14, 'GRAY', cx=233, y=HINT_TOP, name='hint')
    box(cv, *BTN, 'CANCEL', False)
    icon_sized(cv, *ICON_XY, 'WHITE', GLYPH['bright'], module=16, pad=8, name='icon')
    field(cv)
    # value, left, on the field's text column
    cv.text('%d%%' % level_pct, 'bold', 56, 'WHITE', x=X0, bottom=262, name='value')
    # stepped track: 10 cells, filled up to the level
    x0, y0, x1, y1 = TRACK
    gap = 6
    w = (x1 - x0 - gap * (STEPS - 1)) / STEPS
    on = round(level_pct / 10)
    for i in range(STEPS):
        a = round(x0 + i * (w + gap))
        b = round(a + w)
        if i < on:
            cv.rect(a, y0, b, y1, 'WHITE')
        else:
            cv.rect(a, y0, b, y1, 'GRAY')
            cv.rect(a + 2, y0 + 2, b - 2, y1 - 2, 'BLACK')
    cv.text('10', 'mono', 14, 'GRAY', x=x0, y=326, name='min')
    cv.text('100', 'mono', 14, 'GRAY', right=x1, y=326, name='max')
    cv.text('TAP TO KEEP', 'mono', 14, 'GRAY', cx=233, y=372, name='hint2')
    return cv


# ---- device page: information, then clear settings --------------------------------------------
INFO = [('VERSION', '0.1.0'), ('BATTERY', '87%'), ('GNSS', 'FIX  9 SATS')]
ATTRIB = ['TIME ZONE BOUNDARIES:', 'TIMEZONE-BOUNDARY-BUILDER 2026D', '© OPENSTREETMAP CONTRIBUTORS',
          'ODBL 1.0', 'ZONE RULES: IANA TZDATA 2026D']


def device(scroll=0):
    """A list that scrolls under a fixed top cap. scroll is px the content has moved up."""
    cv = Canvas()
    content = Canvas()   # draw content, then clip it below the cap
    y = 206 - scroll
    for k, v in INFO:
        content.text(k, 'bold', 14, 'GRAY', x=X0 + 8, y=y)
        content.text(v, 'mono', 16, 'WHITE', right=466 - X0 - 8, y=y - 1)
        y += 16
        content.rect(X0 + 8, y, 466 - X0 - 8, y + 1, fade('GRAY', 0.5))
        y += 14
    y += 10
    for line in ATTRIB:
        content.text(line, 'mono', 14, 'GRAY', cx=233, y=y)
        y += 20
    y += 18
    box(content, 143, y, 323, y + 44, 'CLEAR SETTINGS', False)
    # clip content to rows 150..420 and to the circle
    m = Image.new('L', (N * SS, N * SS), 0)
    ImageDraw.Draw(m).rectangle([0, 196 * SS, N * SS - 1, 440 * SS], fill=255)
    cv.img.paste(content.img, (0, 0), m)
    cv.ring(230, 232, 'GRAY', name='ring')
    cv.text('DEVICE', 'mono', 14, 'GRAY', cx=233, y=HINT_TOP, name='hint')
    box(cv, *BTN, 'BACK', False)
    icon_sized(cv, *ICON_XY, 'WHITE', GLYPH['device'], module=16, pad=8, name='icon')
    cv.ring(230, 232, 'GRAY')
    return cv


# ---- clear settings, stage 2: drag to confirm --------------------------------------------------
HANDLE = 64


def clear_confirm(progress=0.0):
    cv = Canvas()
    cv.ring(230, 232, 'GRAY', name='ring')
    cv.text('DRAG ACROSS TO CLEAR', 'mono', 14, 'GRAY', cx=233, y=HINT_TOP, name='hint')
    box(cv, *BTN, 'CANCEL', False)
    icon_sized(cv, *ICON_XY, 'ORANGE', GLYPH['clear'], module=16, pad=8, name='icon')
    field(cv)
    x0, x1 = X0, 466 - X0            # travel of the handle's left edge: X0 .. x1 - HANDLE
    y0 = 226
    hx = round(x0 + (x1 - HANDLE - x0) * progress)
    # track: the swept part fills ORANGE; the rest is a 2 px GRAY rail with a target at the end
    cv.rect(x0, y0 + HANDLE // 2 - 1, x1, y0 + HANDLE // 2 + 1, 'GRAY')
    tx = x1 - HANDLE
    cv.rect(tx, y0, x1, y0 + HANDLE, 'GRAY')
    cv.rect(tx + 2, y0 + 2, x1 - 2, y0 + HANDLE - 2, 'BLACK')
    if progress > 0:
        cv.rect(x0, y0, hx + HANDLE, y0 + HANDLE, 'ORANGE')
    # handle: ORANGE block with a black arrow built from rectangles
    cv.rect(hx, y0, hx + HANDLE, y0 + HANDLE, 'ORANGE', name='handle')
    ax, ay = hx + 16, y0 + HANDLE // 2
    cv.rect(ax, ay - 3, ax + 26, ay + 3, 'BLACK')
    for k in range(4):
        cv.rect(ax + 18 + k * 3, ay - 12 + k * 3, ax + 21 + k * 3, ay + 12 - k * 3, 'BLACK')
    cv.text('ERASES THE ZONE CHOICE', 'mono', 14, 'GRAY', cx=233, y=338, name='what1')
    cv.text('AND BRIGHTNESS', 'mono', 14, 'GRAY', cx=233, y=356, name='what2')
    return cv


def pulldown(offset):
    """The panel entering from the top over the clock face; both move down by `offset`."""
    import lib as L
    from anim import page, exit_
    face = page(exit_('B', min(1, (offset / 466) / 0.35)))
    pan = panel().render(offpanel=False)
    fr = Image.new('RGB', (466, 466), (0, 0, 0))
    fr.paste(face.crop((0, 0, 466, 466 - offset)), (0, offset))
    fr.paste(pan.crop((0, 466 - offset, 466, 466)), (0, 0))
    m = Image.new('L', (466 * 4, 466 * 4), 0)
    ImageDraw.Draw(m).ellipse([0, 0, 466 * 4 - 1, 466 * 4 - 1], fill=255)
    bg = Image.new('RGB', (466, 466), (28, 28, 28))
    bg.paste(fr, (0, 0), m.resize((466, 466), Image.BOX))
    return bg


if __name__ == '__main__':
    shots = [
        ('panel', panel().render(), 'PANEL'),
        ('pulldown', pulldown(200), 'PULLING DOWN OVER THE CLOCK, 200 PX'),
        ('brightness', brightness(50).render(), 'BRIGHTNESS'),
        ('device', device(0).render(), 'DEVICE, TOP'),
        ('device-end', device(96).render(), 'DEVICE, SCROLLED TO THE END'),
        ('clear', clear_confirm(0).render(), 'CLEAR: DRAG TO CONFIRM'),
        ('clear-drag', clear_confirm(0.55).render(), 'CLEAR: PART WAY'),
        ('panel-cal', panel(cal=100).render(), 'PANEL, COMPASS CALIBRATED'),
    ]
    f = ImageFont.truetype('fonts/MonoR.otf', 16)
    cols = 4
    W, Hh = 466 + 24, 466 + 44
    sh = Image.new('RGB', (cols * W + 24, 2 * Hh + 24), (28, 28, 28))
    d = ImageDraw.Draw(sh)
    for i, (n, im, label) in enumerate(shots):
        im.save('out/settings-%s.png' % n)
        x, y = 24 + (i % cols) * W, 24 + (i // cols) * Hh
        sh.paste(im, (x, y))
        d.text((x, y + 474), label, font=f, fill=(136, 142, 152))
    sh.save('out/settings-overview.png')
