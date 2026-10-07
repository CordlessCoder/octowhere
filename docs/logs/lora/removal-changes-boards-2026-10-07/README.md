# The removal changes on two boards, 2026-10-07

Boards 1A:38 (id 0, Dredge) and 1C:1C (id 1, Roger Roger) ran master at `de04556`, built with
`pair-inject`, `fix-inject` and `rtc-inject`: the five removal changes, the hourly airtime
ledger, the position rotation dropped and the longer lead for members out of reach, none of
which a board had run before. Their group came back from flash at generation 17. Neither RTC
held a time, so `tools/rtc-inject.py` set both, and `tools/fix-inject.py --mesh` made 1C:1C the
group's GPS root. The logs are the boards' captures, cut to the mesh's lines; each counts from
its own board's restart, and 1A:38's clock is 136 s ahead of 1C:1C's. 1A:38 removed three
phantoms, members no device stands behind, enrolled with `tools/pair-inject.py phantom`.

| Log | What it holds |
| --- | --- |
| `1a38.log` | 1A:38, the remover, from 129 s after its restart to the third switch |
| `1c1c.log` | 1C:1C from its restart to the third switch |

What the run showed:

- 1A:38 first took 1C:1C's clock started from its boot. Once 1C:1C had a fix, 1A:38 held its
  first GPS packet and took GPS time at the second, 43 s later, at one hop.
- With every remaining member in reach, the switch was five rounds off (`removing 2:
  generation 18 from round 39808840 (now 39808835)`). The key message went 0.5 s after it was
  posted, 1C:1C learned of the removal from it, and posted no acknowledgement. Both switched at
  the same round's start. 1A:38 sent the phantom its removal notice under the old key 4.8 s
  later, and each board dropped the old key once it heard the other on the new one.
- With a second phantom left as a member, which 1A:38 never hears, the switch was nine rounds
  off: the four rounds added for a member out of reach. The two key messages, 196 bytes each,
  went 0.54 s apart; with the rest after each they would have been about 3 s apart. Both
  boards switched together.
- 1C:1C, deafened with `pair-inject.py deaf` until 8 s after the third switch, missed the
  removal. 1A:38 heard it on the old key 51 s after switching, and sent it its key message
  under that key 5.0 s later, at a time it drew rather than at once. 1C:1C took it, switched
  three rounds after learning, as a member that learns late does, and 1A:38 dropped the old key
  and the key message it had stored in flash for 1C:1C 0.5 s after that.
- On hearing 1A:38's removal notice under the old key after the second switch, 1C:1C took
  1A:38 to be on that generation still (`0 is on generation 18; no key message is held for
  it`). It held nothing to send, so nothing went.
- The packets were 36 to 231 bytes, most of them under 125. Two nodes hold two positions at
  most, so the rotation's absence does not show here.
- 1C:1C left its own position out of two of its first five packets, with room for it. One
  packet in six did the same on 2026-10-05 (`docs/logs/lora/board-checks-2026-10-05/`), before
  these changes; the cause is not known.
- No panic and no warning on either board. Both were left running this build.
