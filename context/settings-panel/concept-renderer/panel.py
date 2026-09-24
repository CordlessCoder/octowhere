"""Settings panel: the registration grid.

Six cells in three columns of two, column-major:
  01 ZONE      03 COMPASS   05 BATTERY
  02 BRIGHT.   04 GNSS      06 DEVICE
Two columns show at a time; the third is cropped by the circle. BATTERY and GNSS are
proposals: their data is grade 2 (not passed to the screens yet).

Every accent takes a progress value so the same code renders stills and animation frames.
"""
import lib
from lib import *
from industrial import cells_text, pen_for

GLYPH = {
    'zone': '11011 10001 00100 10001 11011',
    'bright': '00001 00011 00111 01111 11111',
    'cal': '01110 10001 10000 10001 01110',
    'nodata': '11100 11000 00100 00011 00111',
    'gnss': '00100 01010 10101 01010 00100',
    'battery': '01110 11111 10001 11111 11111',
    'device': '00100 00000 01100 00100 01110',
}

# ---- geometry -------------------------------------------------------------------------------
RULES = (72, 218, 364)       # horizontal rules; cells are rows 73..217 and 219..363
COLW = 146                   # column pitch; at rest two columns span x 87..379
NCOL = 3
MAXSCROLL = NCOL - 2
PAD = 10                     # text inset from the cell's left rule
ICON = dict(module=10, pad=8)   # 66 px, 3 px frame
TITLE_TOP = 38
PIPS_Y = 386
HINT_TOP = 406

FIX = dict(zone_mode='AUTO', zone='IST', bright='47%', cal=54, compass_live=True, gnss_fix=True, sats=9,
           battery='87%', usb=True, battery_present=True, version='0.1.0')


def cells(f=FIX):
    """(key, name, glyph, colour, value, value colour, tag) per cell, in index order."""
    if f['compass_live']:
        cal_col = 'ORANGE' if f['cal'] < 100 else 'WHITE'
        compass = ('cal', 'COMPASS', 'cal', cal_col, 'CAL %d%%' % f['cal'], cal_col, None)
    else:
        compass = ('cal', 'COMPASS', 'nodata', 'RED', 'NO DATA', 'RED', None)
    zone_val = f['zone'] if f['zone'] else 'NO ZONE'
    zone_vcol = 'WHITE' if f['zone'] else 'GRAY'
    gnss = ('gnss', 'GNSS', 'gnss', 'BLUE', 'FIX  %d' % f['sats'], 'WHITE', None) if f['gnss_fix'] else \
           ('gnss', 'GNSS', 'gnss', 'WHITE', 'NO FIX', 'GRAY', None)
    if f['battery_present']:
        battery = ('battery', 'BATTERY', 'battery', 'WHITE', f['battery'], 'WHITE', 'USB' if f['usb'] else None)
    else:
        battery = ('battery', 'BATTERY', 'battery', 'WHITE', 'NONE', 'GRAY', 'USB' if f['usb'] else None)
    return [
        ('zone', 'ZONE', 'zone', 'WHITE', zone_val, zone_vcol, f['zone_mode']),
        ('bright', 'BRIGHTNESS', 'bright', 'WHITE', f['bright'], 'WHITE', None),
        compass,
        gnss,
        battery,
        ('device', 'DEVICE', 'device', 'WHITE', f['version'], 'WHITE', None),
    ]


def cell_origin(i, scroll):
    c, r = divmod(i, 2)
    left = 233 + (c - 1 - scroll) * COLW
    top = RULES[r] + 1
    return left, top


FULL = dict(ring=1.0, title=1.0, rules=1.0, rows=[5] * 6, index=[1.0] * 6, name=[1.0] * 6, hint=1.0, pips=1.0)


def panel(f=FIX, scroll=0.0, e=FULL):
    cv = Canvas()
    if e['ring'] > 0:
        cv.ring(230, 232, fade('GRAY', e['ring']), name='ring')
    # rules: drawn out from x 233 to the grid's extent, then clipped to the circle
    k = e['rules']
    if k > 0:
        g0 = 233 + (-1 - scroll) * COLW
        g1 = 233 + (NCOL - 1 - scroll) * COLW
        a = round(233 - (233 - g0) * k)
        b = round(233 + (g1 - 233) * k)
        for y in RULES:
            cv.rect(a, y, b + 1, y + 1, 'GRAY', name='rule-%d' % y)
        vlen = (RULES[2] - RULES[0]) * k
        for c in range(NCOL + 1):
            x = round(233 + (c - 1 - scroll) * COLW)
            y0 = round(RULES[1] - vlen / 2)
            y1 = round(RULES[1] + vlen / 2)
            cv.rect(x, y0, x + 1, y1 + 1, 'GRAY')
    for i, (key, name, g, col, value, vcol, tag) in enumerate(cells(f)):
        left, top = cell_origin(i, scroll)
        icon_sized(cv, round(left + COLW - PAD - 66), top + 9, col, GLYPH[g], rows_shown=e['rows'][i],
                   name='icon-%d' % i, **ICON)
        cells_text(cv, '%02d' % (i + 1), 'mono', 14, 'GRAY', left + PAD, top + 9 + cap_height('mono', 14), e['index'][i])
        cells_text(cv, name, 'bold', 14, 'GRAY', left + PAD, top + 100 + cap_height('bold', 14), e['name'][i])
        # value: data, never animated
        vy = top + 128
        x = left + PAD
        if tag:
            w_tag = adv('bold', 14) * len(tag) + 10
            cv.rect(round(x), vy - 9, round(x + w_tag), vy + 9, 'GRAY', name='tag-%d' % i)
            cv.text(tag, 'bold', 14, 'BLACK', cx=x + w_tag / 2, cy=vy)
            x += w_tag + 6
        cv.text(value, 'mono', 16, vcol, x=x, cy=vy, name='value-%d' % i)
    # title, pips, hint
    cells_text(cv, 'SETTINGS', 'shapiro', 16, 'WHITE', pen_for('SETTINGS', 'shapiro', 16, cx=233),
               TITLE_TOP + cap_height('shapiro', 16), e['title'])
    if e['pips'] > 0:
        size, gap = 8, 6
        x0 = round(233 - (NCOL * size + (NCOL - 1) * gap) / 2)
        first = round(scroll)
        for i in range(NCOL):
            x = x0 + i * (size + gap)
            if first <= i < first + 2:
                cv.rect(x, PIPS_Y, x + size, PIPS_Y + size, 'WHITE', name='pip-%d' % i)
            else:
                cv.rect(x, PIPS_Y, x + size, PIPS_Y + size, 'GRAY', name='pip-%d' % i)
                cv.rect(x + 2, PIPS_Y + 2, x + size - 2, PIPS_Y + size - 2, 'BLACK')
    hint = 'DRAG UP TO CLOSE'
    cells_text(cv, hint, 'mono', 14, 'GRAY', pen_for(hint, 'mono', 14, cx=233), HINT_TOP + cap_height('mono', 14), e['hint'])
    # crop everything to the page's circle (cells and rules run past it)
    m = Image.new('L', (N * SS, N * SS), 0)
    ImageDraw.Draw(m).ellipse([(C - 232) * SS, (C - 232) * SS, (C + 232) * SS - 1, (C + 232) * SS - 1], fill=255)
    cv.img.paste(Image.new('RGB', cv.img.size, (0, 0, 0)), (0, 0), Image.eval(m, lambda v: 255 - v))
    if e['ring'] > 0:
        cv.ring(230, 232, fade('GRAY', e['ring']))
    return cv


# ---- device page: detail for DEVICE, BATTERY and GNSS -----------------------------------------------
from picker2 import box, BTN, ICON_XY
from face import X0

DEVICE_ROWS = [('VERSION', '0.1.0'), ('BATTERY', '87%  4.02 V'), ('POWER', 'USB  CHARGING'),
               ('GNSS', 'FIX'), ('SATELLITES', '9 OF 14')]
ATTRIB = ['TIME ZONE BOUNDARIES:', 'TIMEZONE-BOUNDARY-BUILDER 2026D', '© OPENSTREETMAP CONTRIBUTORS',
          'ODBL 1.0', 'ZONE RULES: IANA TZDATA 2026D']
LIST_TOP = 196


def device(scroll=0):
    cv = Canvas()
    content = Canvas()
    y = 206 - scroll
    for k_, v in DEVICE_ROWS:
        content.text(k_, 'bold', 14, 'GRAY', x=X0 + 8, y=y)
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
    end = y + 44
    m = Image.new('L', (N * SS, N * SS), 0)
    ImageDraw.Draw(m).rectangle([0, LIST_TOP * SS, N * SS - 1, 440 * SS], fill=255)
    cv.img.paste(content.img, (0, 0), m)
    clipped_band(cv, LIST_TOP - 1, LIST_TOP, 'GRAY')     # the scroll edge, marked
    cv.text('DEVICE', 'mono', 14, 'GRAY', cx=233, y=48)
    box(cv, *BTN, 'BACK', False)
    icon_sized(cv, *ICON_XY, 'WHITE', GLYPH['device'], module=16, pad=8)
    cv.ring(230, 232, 'GRAY')
    return cv, end + scroll


if __name__ == '__main__':
    lib.RECORD = True
    import json
    log = {}
    out = []

    def keep(name, cv, label):
        cv.save('out/panel-%s.png' % name)
        log[name] = cv.log
        out.append((name, label))

    keep('rest', panel(), 'PANEL, FIRST TWO COLUMNS')
    keep('end', panel(scroll=1), 'PANEL, SCROLLED TO THE END')
    keep('scrolling', panel(scroll=0.4), 'PANEL, MID-SCROLL')
    keep('manual', panel(dict(FIX, zone_mode='MANUAL')), 'ZONE MANUAL')
    keep('no-zone', panel(dict(FIX, zone=None)), 'ZONE AUTO, NO ZONE YET')
    keep('calibrated', panel(dict(FIX, cal=100)), 'COMPASS CALIBRATED')
    keep('compass-no-data', panel(dict(FIX, compass_live=False)), 'COMPASS NO DATA')
    keep('no-fix', panel(dict(FIX, gnss_fix=False, usb=False), scroll=1), 'NO FIX, ON BATTERY (END)')
    json.dump(log, open('out/panel-ink-log.json', 'w'), indent=1)
    cvd, end = device(0)
    cvd.save('out/panel-device.png')
    cvd2, _ = device(end - 420)
    cvd2.save('out/panel-device-end.png')
    print('device list end at scroll 0:', end, 'scroll needed', end - 420)
    out += [('device', 'DEVICE PAGE'), ('device-end', 'DEVICE PAGE, SCROLLED TO THE END')]
    f = ImageFont.truetype('fonts/MonoR.otf', 16)
    cols = 5
    W, Hh = 466 + 24, 466 + 44
    rows = (len(out) + cols - 1) // cols
    sh = Image.new('RGB', (cols * W + 24, rows * Hh + 24), (28, 28, 28))
    d = ImageDraw.Draw(sh)
    for i, (n, label) in enumerate(out):
        x, y = 24 + (i % cols) * W, 24 + (i // cols) * Hh
        sh.paste(Image.open('out/panel-%s.png' % n), (x, y))
        d.text((x, y + 474), label, font=f, fill=(136, 142, 152))
    sh.save('out/panel-overview.png')
