# Boot's milestones, 2026-10-06

`bench/startup-timing` (feature `startup-timing-bench`) on both boards, five boots each, with
times from the timer's start. `summary.txt` is `tools/startup-timing-summary.py` over
`before/`, `after-final/`, `gnss-warm/` and `gnss-cold/`.

Warm starts are reset reads by `tools/startup-timing.sh`. Every one reports a power-on, but the
GNSS module stays powered through it. Cold starts are the owner unplugging each board and
plugging it back in; `tools/startup-timing-cold.sh` read their marks over the USB JTAG with
`tools/startup-timing-read.py`, which does not reset the board, since a serial capture would.
The boards have no battery, so a cold start also clears the RTC.

- `before/`: `63e1422`. The clock, touch, IMU and magnetometer checked in turn beside the GNSS
  module's 1 s settle, then the GNSS module configured, then the radio.
- `after/`: the four checked together, with the GNSS probe inside the same join. It cost core
  0's stack 4,240 bytes and was not kept.
- `after-final/`: `5211f16`, the four together and the GNSS probe after the join.
- `gnss-warm/`: on top of that, the settle counted from the timer's start instead of after the
  I/O expander's reset, and the GNSS check passing at its first read, with `gnss_task`
  configuring the module afterwards.
- `gnss-cold/`: the same build, from cold starts.

| 1A38 (1C1C) | before | after-final | gnss-warm | gnss-cold |
| --- | --- | --- | --- | --- |
| Four I²C parts done | 1,482 (1,426) | 1,163 (1,109) | 1,164 (1,109) | 1,166 (1,109) |
| GNSS check | 624 (624) | 583 (624) | 11 (11) | 54 (54) |
| Last part decided | 2,650 (2,595) | 2,650 (2,596) | 1,177 (1,123) | 1,222 (1,166) |
| GNSS configured | 2,634 (2,580) | 2,593 (2,581) | 1,434 (1,823) | 1,911 (1,854) |
| Clock face takes over | 7,674 (7,622) | 7,671 (7,620) | 6,372 (6,319) | 6,402 (6,351) |

Medians in ms. Before `gnss-warm/` the GNSS check included the configuration, so the module was
configured when the check ended.

After a cold start the GNSS check took about 54 ms, against 11 after a warm one, on all ten. None
needed a reset.

1C1C's GNSS module refused its address, and was reset, in 5 of its 15 boots up to
`after-final/` (`before/` 1 and 2, `after/` 2 and 4, `after-final/` 5); the self-test then
waited 3.1 s for it. 1A38's never did. Neither needed one in the 20 boots since.
