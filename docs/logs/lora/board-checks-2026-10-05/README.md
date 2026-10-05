# The mesh on two boards after the clean-up, 2026-10-05

Boards 1A:38 (id 0, Dredge) and 1C:1C (id 1, Roger Roger) ran master at `7311c7c`, built with
`fix-inject`, `touch-inject`, `pair-inject` and `rtc-inject`: the 2026-10-04 security fixes,
the mesh clean-up and the CO5300 crate, none of which a board had run before. Neither had a
fix or RTC time, so `tools/rtc-inject.py` set 1C:1C's RTC and `tools/fix-inject.py --mesh` made
it the group's GPS root. The logs are the boards' captures, cut to the mesh's lines; each counts
from its own board's restart.

| Log | What it holds |
| --- | --- |
| `1a38.log` | 1A:38 from flashing to the second removal: the stranded member, the pairing, the old key dropped, the phantom's removal |
| `1c1c-stranded.log` | 1C:1C until it left: still on generation 11 |
| `1c1c-restarted.log` | 1C:1C after its restart: its group back from flash, the timebase, the removal |
| `1a38-watermark.log`, `1c1c-watermark.log` | Both on `bench/stack-watermark`: a pairing with the group screens drawn, a message each way and a removal |

What the run showed:

- Both boards brought their group back from flash through the clean-up's new decoders, with the
  same members, generations and removal state as before.
- 1A:38 was on generation 12, waiting on the old key for 1C:1C, which was still on 11. In its
  sweep round it heard 1C:1C on the old key, but had no key message to send it (`no key message
  is held for it`): those live in the message store, in PSRAM, and 1A:38 had restarted since
  its switch. It sent an empty packet under the old key instead, and 1C:1C could not open
  1A:38's packets. In a group of two the only node that could hand the key message back is the
  member waiting for it, so 1C:1C would never have switched. It left, and 1A:38 paired it back
  in on generation 12.
- After the pairing, 1A:38 heard 1C:1C every round on the new key but kept waiting for it on
  the old one, and kept sending old-key packets in its sweep rounds. A member sends the record
  that says it is on the group's key only after a switch or a restart, and a pairing is
  neither. Once 1C:1C restarted, 1A:38 dropped the old key within a round.
- 1C:1C, restarted, took the GPS timebase from 1A:38 48 s after booting, at one hop.
- 1A:38 enrolled a phantom and removed it, twice. 1C:1C learned each request a round later. Both
  switched at the same round boundary, to 13 and then 14. The switch fell on a sweep round,
  where 1A:38 sent 1C:1C its key message under the old key. Both dropped the old key within
  30 s.
- Core 0's stack: 42,892 of 105,068 bytes used after the start-up, and 52,472 at most through
  the pairing with the group screens drawn, the messages and the removal, on both boards.
