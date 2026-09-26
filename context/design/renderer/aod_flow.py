"""Timeout flow: clock at rest -> dim -> always-on face (two minute changes, each one shift
step) -> a touch wakes it -> the clock runs its entry. Waiting periods are compressed.
Brightness is simulated by scaling colours (dim 0.35, always-on 0.45); the panel does it with
its brightness command, at no draw cost."""
import lib
lib.RECORD = False
from lib import *
from aod import aod, dim, SHIFTS
from anim import page, entry, save_gif, DT
from startup import still

DIM, AODK = 0.35, 0.45


def frames():
    fr = []
    full = still(page(entry('B', 1000)))
    fr += [full] * 60                                  # at rest
    fr += [dim(full, DIM)] * 60                        # dimmed for 5 s (compressed)
    for i, m in enumerate((7, 8, 9)):                  # always-on, one shift step per minute
        fr += [dim(aod('local', SHIFTS[i], m=m).render(), AODK)] * 50
    # a touch wakes it: brightness steps up over 100 ms while the clock's entry runs
    for t in range(0, 500, DT):
        k = min(1.0, AODK + (1 - AODK) * t / 100)
        e = entry('B', t)
        e.update(dict(time=1.0, date=1.0))
        fr.append(dim(still(page(e, f=dict(__import__('industrial').FIX, m=9))), k))
    fr += [fr[-1]] * 60
    return fr


if __name__ == '__main__':
    save_gif(frames(), 'out/aod-flow.gif', DT)
