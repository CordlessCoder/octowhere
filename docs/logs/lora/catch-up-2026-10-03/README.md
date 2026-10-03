# Catching up across two missed switches, 2026-10-03

Runs on the two bench boards, indoors, with no GNSS fix, on firmware built with `pair-inject`,
`touch-inject` and `rtc-inject`. Commands came from `tools/pair-inject.py`. The logs are named by
the hardware address each board printed, and are xz-compressed. Times are each board's own, from
its boot. The boards started as the group the signing runs left (`../signing-2026-10-03/`): `1a38`
at id 0, `1c1c` at id 1, and a phantom at id 2, a member no device stands behind, at generation 0.

## Booting with a stored signed group

On the build of `00db111`, both boards booted with that group and played the start-up through,
where the build of `6750e8d` had panicked 5.44 s after boot. Each held 61,400 bytes of the
internal heap once its parts were up, the glyph raster's 35,436 among them. Their RTCs had lost
the time while the boards were unplugged (`rtc-1a38.log`, `rtc-1c1c.log`), and
`tools/rtc-inject.py` set it on both before the run. Without a time, a node roots its clock near
1970 and refuses records stamped in 2026 (the security review's oscillator-stop item in
`context/BACKLOG.md`).

## Two missed switches

`two-1a38.log` and `two-1c1c.log`, on the same build.

- **Joining up.** Both rooted their own timebases at 137 s, and `1c1c` adopted `1a38`'s at
  232 s.
- **A second phantom.** `1a38` enrolled one at id 3 at 237 s, and `1c1c` took its record at
  259 s.
- **Deaf through both.** From 260 s `1c1c` dropped everything it heard, in windows of 255 s
  renewed every 200 s, the last ending at 915 s. Meanwhile `1a38` removed id 2, switching to
  generation 1 at 576 s, and then id 3, switching to generation 2 at 846 s.
- **Generation 1.** `1a38` found `1c1c` still on generation 0 at 1340 s and sent it the key
  message of generation 1 under the old key at 1412 s. `1c1c` took it at 1408 s by its own clock,
  switched at 1517 s and stored the group, 602 s after it could hear again. It could decline the
  removal until round 39803180, a day after the switch.
- **Generation 2.** `1a38` heard `1c1c` on generation 1 at 1900 s and sent it the next key
  message at 1928 s. `1c1c` switched to generation 2 at 2058 s, 540 s after the first.

Each catch-up began only when `1a38` next heard `1c1c` under an old key, about 7 and 6 minutes
after `1c1c` could hear again or had switched, and each switch came three rounds, about 2 minutes,
after its key message. Two missed switches took 19 minutes to recover.

## A key kept for a removed member

Once both were on generation 2, `1a38` never logged that every member was on the new key. Both
boards kept generation 0's key and sent a packet under it in their sweep rounds, `1a38` at
2509 s and `1c1c` at 3072 s, and each logged hearing the other "on generation 0".

The cause was in `Rekey::switch`. Each old key waits for the members not yet heard on a newer
one. When `1a38` switched to generation 1, the phantom at id 3 was still a member, so generation
0's key waited for ids 1 and 3. The switch to generation 2 removed id 3 but left its place in that
wait, and a removed member is never heard again. `d6b9389` drops a removed member from every old
key's wait at the switch that removes it, and from what was stored when the mesh starts.

`fix-1a38.log` and `fix-1c1c.log`, on the build of `d6b9389`: both boards booted at generation 2
holding no old key (`old keys=0`). Over 14 minutes they heard each other and passed through five
sweep rounds between them, and neither sent under an old key.
