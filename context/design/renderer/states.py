"""Renders every face state, the manual face, a swipe frame and the contact sheets."""
from face import *
from industrial import v1
from picker2 import step1, step2
import json

LOG = {}


def keep(name, cv):
    cv.save('out/clock-%s.png' % name)
    LOG[name] = cv.log


from industrial import FIX as IFIX
keep('local-gnss', v1('gnss'))
keep('local-rtc', v1('rtc'))
keep('local-manual', v1('gnss', f=dict(IFIX, mode='MANUAL')))
keep('stopped', v1('stopped'))
keep('no-zone', v1('nozone'))
keep('no-data', v1('nodata'))
keep('picker-offset', step1(auto=True))
keep('picker-offset-manual', step1(auto=False))
keep('picker-offset-stopped', step1(auto=True, time_known=False))
keep('picker-zone', step2())
keep('picker-zone-sea', step2(sea=True))

# swipe: page 120 px left of rest, accents faded by offset (nothing at 35 % of width)
off = 120
k = max(0.0, 1 - (off / 466) / 0.35)
from anim import page, exit_
import lib as _l
_l.RECORD = True
cv = None
im = page(exit_('B', min(1, (off / 466) / 0.35)))
sw = Image.new('RGB', (466, 466), (0, 0, 0))
sw.paste(im.crop((off, 0, 466, 466)), (0, 0))
m = Image.new('L', (466 * 4, 466 * 4), 0)
ImageDraw.Draw(m).ellipse([0, 0, 466 * 4 - 1, 466 * 4 - 1], fill=255)
bg = Image.new('RGB', (466, 466), (28, 28, 28))
bg.paste(sw, (0, 0), m.resize((466, 466), Image.BOX))
bg.save('out/clock-swiping.png')

json.dump({k: v for k, v in LOG.items()}, open('out/ink-log.json', 'w'), indent=1)


def sheet(names, path):
    ims = [Image.open('out/clock-%s.png' % n) for n in names]
    sh = Image.new('RGB', (486 * len(ims) - 20, 466), (28, 28, 28))
    for i, im in enumerate(ims):
        sh.paste(im, (i * 486, 0))
    sh.save(path)


sheet(['local-gnss', 'local-rtc', 'local-manual', 'swiping'], 'out/sheet-face-normal.png')
sheet(['stopped', 'no-zone', 'no-data'], 'out/sheet-face-withheld.png')
sheet(['picker-offset', 'picker-offset-manual', 'picker-offset-stopped'], 'out/sheet-picker-1.png')
sheet(['picker-zone', 'picker-zone-sea'], 'out/sheet-picker-2.png')
print('ok', k)
