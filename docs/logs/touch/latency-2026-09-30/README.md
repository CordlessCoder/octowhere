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

Lift recognition, from `touch-read-log` captures with a finger, summarised by
`uv run tools/touch-lift-summary.py <file>`:

- `finger-reads-counted.txt`: master at 5e8f225, which counted a lift after three empty reads.
  Taps, fast flicks, slow drags held still and glancing swipes. It sized `LIFT_GRACE`: the finger
  came back after 7 of 93 lift reports, 4 to 48 ms later, and two flicks ended with no lift
  report at all.
- `finger-reads-timed.txt`: the timed lift. Still holds of up to 6.6 s, fast flicks, some off
  the edge, and taps. It sized `SILENT_LIFT`: a finger held still went at most 30 ms between
  reports.

With the scripted finger (`touch-latency-synthetic`, 200 s each, not kept), a lift counted a
median 60 ms after its report with three empty reads (p90 94, max 110), and 49 ms with the timed
lift (p90 85, max 101). With a finger the timed lift came to a median 54 ms on the compass and 40
ms on the clock face; the step at the deadline waits behind the compass's redraws.
