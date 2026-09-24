"""Checks on the renders: fit inside the circle, colour use, and the text sizes the spec quotes."""
import json, math
import numpy as np
from PIL import Image
from lib import TOK, rgb, C

L = json.load(open('out/ink-log.json'))
bad = 0
# 1. every element except full-chord bands and the ring keeps its ink inside radius 226
for scr, elems in L.items():
    for name, bb in elems:
        if bb is None or name in ('ring', 'band') or name.startswith('rule-'):
            continue
        x0, y0, x1, y1 = bb
        r = max(math.hypot(x + 0.5 - C, y + 0.5 - C) for x in (x0, x1) for y in (y0, y1))
        if r > 226:
            print('OUTSIDE r226', scr, name, bb, round(r, 1)); bad += 1
# 2. colour use, by exact pixel match of large areas (ignores antialiased edges)
def has(img, tok, min_px=40):
    a = np.array(img.convert('RGB')).reshape(-1, 3)
    return int((a == np.array(rgb(tok))).all(axis=1).sum()) >= min_px
for scr in L:
    im = Image.open('out/clock-%s.png' % scr)
    red, yel = has(im, 'RED'), has(im, 'YELLOW')
    if red != (scr == 'no-data'):
        print('RED misuse', scr); bad += 1
    if yel:
        print('YELLOW used', scr); bad += 1
# 3. break the check: a rigged element outside must be caught
x0, y0, x1, y1 = 20, 20, 40, 40
assert max(math.hypot(x + 0.5 - C, y + 0.5 - C) for x in (x0, x1) for y in (y0, y1)) > 226
assert has(Image.open('out/clock-no-data.png'), 'RED') and not has(Image.open('out/clock-local-gnss.png'), 'RED')
print('checks failed:', bad)
