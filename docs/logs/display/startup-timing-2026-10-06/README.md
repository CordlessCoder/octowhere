# Boot's milestones, 2026-10-06

`bench/startup-timing` (`bb3c34e`, feature `startup-timing-bench`) on both boards, five reset
reads each, run by `tools/startup-timing.sh`; `summary.txt` is `tools/startup-timing-summary.py`
over `before/` and `after-final/`. Times are from the timer's start, which follows the
bootloader. Every reset read reports a power-on, but the GNSS module stays powered through it,
so none of these is a cold start.

- `before/`: `63e1422`, the clock, touch, IMU and magnetometer checked in turn beside the GNSS
  module's 1 s settle.
- `after/`: the four checked together, with the GNSS probe inside the same join. It costs core
  0's stack 4,240 bytes and was not kept.
- `after-final/`: the four together, the GNSS probe after the join, as proposed.

The four end at 1,481 ms in turn and 1,163 ms together on 1A38 (1,426 and 1,109 on 1C1C). The
settle ends at 2,010 ms either way, the GNSS probe then takes about 624 ms and the radio about
8 ms, so the last part decides at about 2,650 ms (2,596) in every run, and the clock face takes
over at about 7.67 s (7.62).

1C1C's GNSS module refused its address, and was reset, in 5 of its 15 boots (`before/` 1 and 2,
`after/` 2 and 4, `after-final/` 5); the self-test then waited 3.1 s for it. 1A38's never did.
