Summaries from `tools/touch-latency-summary.py`, 2026-09-30.

- `finger-before.txt`: a finger on master at 4db68bc, which waited for INT's low level and handed
  reads over through a one-slot channel. Its log predates the `seq`, `idle`, `since_fall`, `x`,
  `y` and `at` fields, so its "at rest" group includes taps that landed while the frame loop was
  drawing, and it has no slop-crossed line.
- `finger-after.txt`: a finger on this branch, about 30 s of taps and drags on the clock and the
  compass.
- `synthetic-old-handoff.txt`, `synthetic-new.txt`: `tools/touch-latency-sweep.sh`, 200 s each.
  Two more variants in that sweep handed every frame to core 1, including frames that showed
  nothing new; they matched these within a few milliseconds, and that rule is no longer here.
