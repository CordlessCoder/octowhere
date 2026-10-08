# A board leaving its own position out of packets, 2026-10-08

On 2026-10-05 and 2026-10-07, 1C:1C, the board given a fix with `tools/fix-inject.py --mesh`,
left its own position out of some packets with room for it
(`docs/logs/lora/removal-changes-boards-2026-10-07/`). The cause was the stand-in fix, not the
mesh. The firmware pinned the mesh's GPS time to the RTC's whole second once, when the fix was
first given, and then stamped each fix with the RTC's current second. So the mesh's clock ran
behind the RTC by whatever fraction of a second had passed at that moment, and a fix could be
stamped a second after the second its packet started in. A packet carries no entry newer than
its own base timestamp, so the position was left out. A real fix cannot do this: its stamp is
set at the sighting that moves the mesh's clock to that second.

Board 1C:1C ran `bench/own-position`, master at `60ffb36` with one log line per sent packet
giving its own position's stamp, the packet's base and whether the position went in, built with
`pair-inject`, `fix-inject` and `rtc-inject`. `fe86be6` is the build before the fix, `fdcd845`
the build with it, which stamps a stand-in fix with the mesh's own second. 1A:38 ran `de04556`
and only listened. `tools/rtc-inject.py now` set 1C:1C's RTC and `tools/fix-inject.py --mesh`
gave it a fix. The fix moved 50 m and back every 40 s, so that the board had news for a packet
each round. Turning the fix off and on again pinned the offset afresh, at another point in the
RTC's second; each such phase is counted apart.

| Log | What it holds |
| --- | --- |
| `before-1c1c.log` | `fe86be6`: three phases, re-pinned at 752 s and 1,209 s |
| `after-1c1c.log` | `fdcd845`: three phases, re-pinned at 211 s and 377 s |

What the run showed:

- Before the fix, 1C:1C sent 18 packets and left its own position out of 2, both in the third
  phase (`own stamp=1791449401 base=1791449400 carried=false`). Those two are the only packets
  whose fix was stamped after the packet's second. In the other 16 the stamp was the packet's
  second or the one before, and the position went in. The first two phases sent 8 and 6
  packets and lost none: how often a position is left out depends on where in the RTC's second
  the offset was pinned, so a phase can lose none.
- With the fix, it sent 18 packets over three phases and every one carried its position. No
  stamp was after its packet's second; 9 were the second before.
- 1C:1C failed the CRC of two packets it received in the first run. Nothing else warned, and
  neither board restarted.

Master stamps the stand-in fix with the mesh's second from `3430b0c` on.
