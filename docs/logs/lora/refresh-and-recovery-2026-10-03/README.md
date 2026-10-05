# Shuffled slots, refreshing devices and a founder's wait, 2026-10-03

Five runs on the two bench boards, indoors, with no GNSS fix, on firmware built with
`pair-inject`, `touch-inject` and `rtc-inject`: master `6f21f3a` for the first two runs, and
with `9c8949e` for the rest. Commands came from `tools/pair-inject.py`, the screens were driven
and read back with `tools/touch-inject.py`, and `tools/rtc-inject.py` set the RTCs. The logs are
named by the hardware address each board printed, and are xz-compressed. Times are each board's
own, from its boot. `screens.png` is the boards' own framebuffers, in the order below.

## Starting together

`start-1a38.log` and `start-1c1c.log`. Both RTCs had lost their time while unplugged, so both
clocks started from boot, about 18 ms apart, and both boards were reset together.

- **Self-test.** GNSS answered in 623 ms and decided about 2.3 s after power-on; the radio's
  check started just after and answered in 7 ms.
- **Shuffled slots.** Each board swept for three rounds and became its own root at 137.3 s.
  `1c1c` heard `1a38` in one of its windows at 160.3 s and took its timebase, and from then on
  each heard the other in the order the group key shuffles each round.
- **Member records on request.** Every packet was 36 bytes: header, neighbours and the members
  digest, with no position, since neither board has a fix. Neither sent a record or asked for
  one: their digests agreed.

## Refreshing, from either side

The RTCs were then set 271 s apart, `1c1c` ahead, and both boards reset together, so each became
its own root, `1a38` root 0 and `1c1c` root 1, on clocks too far apart to hear each other.

- **First try** (`refresh-first-*.log`). A refresh on `1c1c` heard `1a38` 8 s in and took its
  timebase, and reported `heard=0x00000001`. Taking up the timebase ended its sweep, as it ends a
  first sweep, so it stopped listening throughout after 8 s, and its end was shown 20 s late,
  at the next window. `9c8949e` keeps a refresh's sweep to its end and wakes for it.
- **From the lower side** (`refresh-lower-*.log`). A refresh on `1c1c` ran from 146.845 s to
  281.846 s, 135.000 s. It took `1a38`'s timebase at 172.5 s and swept on to its end:
  `heard=0x00000001`, nothing learned.
- **From the higher side** (`refresh-higher-and-founding-*.log`, to 280 s). A refresh on `1a38`
  heard `1c1c` on root 1 20.6 s in and queued a notice, which went out at 179.9 s. `1c1c` swept
  from the notice, heard one of `1a38`'s own packets at 235.3 s and took its timebase. The
  refresh ended on time with `heard=0x00000002`. `1a38` then heard a packet `1c1c` had sent just
  before it moved and sent a second notice, 24 bytes, which `1c1c`, now on root 0, ignored.

## A founder's wait

The same logs, from 330 s. Both boards left the group; `1a38` added and `1c1c` joined (code
479 195). `1c1c` confirmed, then `1a38` was made deaf for 45 s and confirmed, so it never heard
the last acknowledgement.

- **Waiting.** At 372.5 s `1a38` ended unconfirmed and listened under the founded group's key.
  Its group screen showed GROUP PENDING, `Roger Roger`, `441BF6861C1C` and `LISTENING / 08:57
  LEFT`. `1c1c` ended joined, unconfirmed, swept, became root 1 at 507.6 s and sent at 529.9 s.
- **Stored.** `1a38` heard that packet at 530.14 s, 2 min 38 s into its wait, and stored the
  group, a write of 25.8 ms, before taking it up 29 ms after hearing it. The screen showed GROUP
  STORED in place. Its first packet on the group carried `1c1c`'s record, which enrolling a
  member marks to be sent, and nothing was asked.
- **The screens** then went through GROUP to MEMBERS and REFRESH DEVICES: the start screen, the
  refresh running (`01:55`, `01 DEVICE HEARD`), and MEMBERS' strip counting down (`LISTENING
  01:18`). The refresh ran 616.29 s to 751.29 s, `heard=0x00000002`. After it ended, the strip
  opened its result once, `REFRESH / JUST ENDED`, and after VIEW MEMBERS the start screen.
- **Expired.** A second founding (code 149 320) went the same way, then `1c1c` left 1.5 s after
  its pairing ended, so nothing was sent under the founded key. `1a38` stopped listening at
  1499.84 s, 600.00 s after it started, and founded no group. Its group screen showed NO GROUP /
  WAIT ENDED / NOTHING HEARD once, and VIEW GROUP went on to the screen of a device in no group.

## The self-test's drawing

`selftest-timing-1c1c.log`, one boot on a build with `timing-log`. While the list scrolls to the
radio, each frame flushes the list and the counter, 126,752 to 128,068 pixels, against 217,156
for the whole panel: about eight frames, drawing in 16 to 31 ms and flushing in 9 to 11 ms. The
radio's check runs in the boot task, so the scroll does not hold it back. The build's logging
slows everything: its radio check took 63 ms.

## Pairing again

`pairing-again-*.log`. Both boards reset together and paired normally, `1a38` adding: both
stored the group by 15.9 s and the receipt was confirmed. They restarted their clocks from RTCs
still 271 s apart, so each became its own root. `1c1c` heard `1a38` at 458.7 s in one of its
ordinary windows, in no sweep round of either board, and took its timebase. They are left a
group of two, `1a38` at id 0 and `1c1c` at id 1.
