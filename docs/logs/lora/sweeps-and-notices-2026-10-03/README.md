# Sending while alone, sweep rounds and notices, 2026-10-03

Three runs on the two bench boards, indoors, with no GNSS fix, on firmware built with
`pair-inject` and `touch-inject`, the last also with `rtc-inject`. Both boards were already a
group of two: `1a38` at id 0 and `1c1c` at id 1. Each run reset both boards together, so both
swept for three rounds, heard nothing and became their own roots: `1a38` root 0, which ranks
higher, and `1c1c` root 1. The logs are named by the hardware address each board printed, and
are xz-compressed.

## Sending while alone

`alone-1a38.log` and `alone-1c1c.log`. Neither RTC held a time, so both clocks started from
boot. Each board sent in every round while it heard nobody, round 5 included, which is neither
board's floor round. The run ended before they met.

## Sweep rounds

`sweep-round-1a38.log` and `sweep-round-1c1c.log`. The boards started a few seconds apart, with
clocks from boot, so their timebases' rounds nearly coincided. Round 13 was a sweep round on
both: each listened throughout it and sent in its own slot. `1a38` heard `1c1c` in it, and
`1c1c` heard `1a38` and took its timebase. Neither swept in round 14.

## A notice

`notice-1a38.log` and `notice-1c1c.log`. Their RTCs were set 271 s apart with `rtc-inject`,
`1a38` ahead, so `1a38`'s sweep round came 271 s before `1c1c`'s. 271 s is not a whole number
of slots, so neither board could hear the other in its own windows. Times are each board's own.

| Time | Board | What happened |
| --- | --- | --- |
| 247.2 s | `1a38` | its sweep round opened |
| 250.0 s | `1a38` | heard `1c1c` on root 1 and queued a notice for it |
| 293.5 s | `1a38` | sent the notice |
| 293.3 s | `1c1c` | heard the notice, took no timing from it, and began a three-round sweep |
| 337.2 s | `1c1c` | heard one of `1a38`'s own packets and took its timebase |

`1c1c` joined about 180 s before its own sweep round. The notice waited 43 s: `1c1c` listens for
`1a38`'s slot 1 s after `1a38`'s own, so the first chance came just after `1a38`'s next send.

A first try with the RTCs 100 s apart did not exercise the notice. `1a38`'s sweep round fell in
its first sweep, before it had a timebase, and `1c1c`'s own sweep round heard `1a38`, which was
sending every round while alone.
