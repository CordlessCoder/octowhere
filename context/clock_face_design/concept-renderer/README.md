Concept renderer used for the images. Not firmware code.

Expects `fonts/` beside the scripts (Shapiro.ttf, MonoR.otf, MonoB.otf) and writes to `out/`.
`states.py` renders every clock and picker state and `out/ink-log.json`; `verify.py` checks them.
`compass.py` renders the compass in every state at each icon size.
`anim.py` renders the entry and exit animations; `swipeclip.py` the band-clip comparison.
`industrial.py` renders the variants.
`calib.py` redraws the approved compass for comparison with `ref/compass-heading.png`.
Needs Pillow and numpy.
