"""Swipe frames with the band unclipped vs clipped to the page's circle."""
import sys
import lib
lib.RECORD = False
from lib import *
from anim import page, exit_, hidden, composite, W

OFFS = [60, 140, 240]


def frames():
    out = []
    for off in OFFS:        # swiping out to the left
        p = min(1, (off / W) / 0.35)
        out.append(composite(page(exit_('B', p)), -off))
    out.append(composite(page(hidden('B')), 180))   # coming in from the right
    return out


if __name__ == '__main__':
    tag = sys.argv[1]
    fr = frames()
    sh = Image.new('RGB', (486 * len(fr) - 20, 466), (28, 28, 28))
    for i, im in enumerate(fr):
        sh.paste(im, (i * 486, 0))
    sh.save('out/swipe-band-%s.png' % tag)
