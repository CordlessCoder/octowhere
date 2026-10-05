# Contention on two boards, 2026-10-05

Boards 1A:38 (id 0, Dredge) and 1C:1C (id 1, Roger Roger), in one group at generation 16, with
their RTCs set from earlier runs and no fix. Both ran the inject build (`pair-inject`,
`fix-inject`, `touch-inject`, `rtc-inject`) with the node's debug log, so that each busy
back-off shows. The logs are the boards' captures, cut to the mesh's lines; each counts from
its own board's restart, and 1C:1C's clock started about 8 s after 1A:38's.

| Log | What it holds |
| --- | --- |
| `busy-1a38.log`, `busy-1c1c.log` | `5e9ecd0`: every channel check found the channel busy, and neither board sent |
| `busy-status-1a38.log` | The same on 1A:38, with each check's `DIO0`, interrupt flags, modem status, RSSI and mode (`[DIAG]`) |
| `1a38.log`, `1c1c.log` | `23ff8d9`'s check, with the `[DIAG]` lines: the boards meet, a private and a group message, a phantom's removal |

What the runs showed:

- At `5e9ecd0` the modem status read `0x04` at every check, RX on-going, with nothing on the
  air: that bit holds throughout continuous receive. Every check backed off, about 490 times
  on each board in four minutes, and no packet went. `23ff8d9` takes only a preamble detected,
  the modem synchronised or a header read as a packet under way.
- With that, each board sent its first packet as its first sweep ended, 137 s after booting,
  and 1C:1C took 1A:38's clock from it. Idle, the status read `0x04` or `0x24`, the second
  with the coding rate of the last header in its top bits.
- 1C:1C found the channel busy once, with a packet under way (`0x2F`), and backed off.
- A private message from 1A:38 to 1C:1C, and the acknowledgement back, took 1.13 s from
  queued to heard. A group message from 1C:1C went out 0.33 s after it was queued.
- 1A:38 enrolled a phantom, and 1C:1C had its record within about a second. 1A:38 then removed
  it. 1C:1C learned of the removal within a second. The switch came 198 s after the removal
  started, at the start of the fifth round on, and both switched there. Each dropped the old
  key within about a second of the other's word that it was on the new one.
- 1C:1C heard its root's packets within ±4 ms of where its clock put them. 1A:38 heard 1C:1C's
  5 to 9 ms late: a sender starts a few milliseconds after the time its header names, after
  the check, the load and the switch to transmit, and 1C:1C's clock carries 1A:38's delay too.
  The timebase serves seconds, so this does not matter.

Two boards side by side cannot show what contention risks at size: nodes hidden from each
other, and two nodes starting inside the modem's detection time. The simulator's
`contention.rs` stages those.
