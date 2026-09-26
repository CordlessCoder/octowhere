# OCTOWHERE: scanned glitch fill, motion, and AOD without rules

Design iteration, 25 September 2026. This builds on the round 4 clock face in the supplied five-part backup. The existing clock layout, foreground colours, and 2 px black halos are intact; the background is exploratory. Fixture: 13:07:42, Thursday 24 September 2026, Europe/Dublin, 87% battery.

## Clock

- **G1 quiet:** large dim blue plates and fewer small fragments.
- **G2 compression:** more distinct lightness levels and sizes, 1 px scanlines on a 4 px pitch, dark dropouts, notched corners, short trailing cells. My pick for the next panel test.
- **G3 high signal:** the same structure one lightness step brighter. More lively, more likely to compete with the blue status icon and lower microtext.

The `clock-G2-motion.gif` is 18 concept frames at 125 ms each (8 fps), plus an endpoint hold. Four block groups drift 2 px between steps and jump 6–12 px on staggered beats. A few small fragments change cell. Scanlines advance within the plates. Time, ring, icon, wordmark, and band remain still. The pattern changes only in the black field and stays under the established foreground halos. The GIF demonstrates motion, not a firmware redraw strategy or an approved idle animation rate. The round 4 spec currently says the face rests without motion apart from charging; adopting this effect would explicitly revise that decision and needs a device performance and readability check.

## AOD

Both horizontal rules are removed. **H1** keeps only 24 dim blue cells near the bottom edge and a lime current-hour cell. **H2** retains that rail and adds broken, scanlined blocks in the gap between the hour and minute groups, with a few small lower blocks. The secondary blocks jump only when the minute changes in the demo. The current hour cell changes on the hour. No continuous AOD animation is proposed here.

At 13:07, the original ruled AOD has **5.67%** lit pixels; a rule-free foreground alone has **5.15%**, H1 has **5.28%**, and H2 has **6.81%**. At 13:08 H2 has **7.42%**, mostly because the digit 8 lights more pixels: the rule-free foreground alone is **5.76%**. These figures count pixels above the concept renderer's RGB threshold. They are not panel power measurements. Test at real AOD brightness and across future pixel-shift positions.

The AOD design shown is the normal LOCAL state only. STOPPED, NO ZONE, and NO DATA would need separate treatments before any spec is handed to implementation. In particular, the blue field must not weaken fault colour or information hierarchy.

## Reproduce

Extract the five-part backup, put the two `.py` files into its `renderer/concept/` directory, and from `renderer/` run:

```sh
PYTHONPATH=. python3 concept/glitch_motion.py path/to/output
PYTHONPATH=. python3 concept/aod_norules.py path/to/output
```

They use the fonts and primitives in the supplied renderer. The visible pattern is drawn in code and contains none of the source reference images.
