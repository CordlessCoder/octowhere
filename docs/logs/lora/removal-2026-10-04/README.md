# Removals on two boards, 2026-10-04

The 2026-10-04 hand-off's removal screens, between board 1A:38 (id 0, Dredge) and 1C:1C (id 1,
Roger Roger), with a phantom member that `tools/pair-inject.py phantom` enrolled on 1A:38 as a
third. Both ran `d4e759c` built with `fix-inject`, `touch-inject` and `pair-inject`.
`tools/fix-inject.py --mesh` gave 1C:1C GPS time from its RTC, and 1A:38 took it at 575 s.
1A:38 removed the first phantom and then 1C:1C through its screens, and the second phantom by
command; 1C:1C was driven through its screens. `tools/touch-inject.py` sent the touches and
read the screens back. The logs are the two boards' captures, cut to the mesh's removal,
pairing and timebase lines.

| Shot | What it shows |
| --- | --- |
| `restored.png` | 1C:1C 17 s after its timebase came, before it first sent: the switch an earlier run made, which it could still decline, brought back from flash after the restart, and read |
| `member.png` | 1A:38: the phantom's details, REMOVE now orange |
| `removing.png` | 1A:38 after the slide: its own request, 04:20 to the switch |
| `request-events.png` | 1C:1C's Events: the request, unread and running, with its switch time |
| `request.png` | 1C:1C: the request before the switch |
| `details.png` | 1C:1C: DETAILS, the device's fingerprint as 1A:38's command named it |
| `switched.png` | 1C:1C 8 s after its switch: a day and a round left to decline |
| `own-switched.png` | 1A:38 after its switch: its own request, which it cannot decline |
| `removing-roger.png` | 1A:38 removing 1C:1C, with no member left to tell |
| `removed-events.png`, `removed.png` | 1C:1C told it was removed, and the notice |
| `leave.png` | LEAVE GROUP on the notice: the group screens' own confirmation, before anything is left |
| `pending-member.png` | 1C:1C, a second phantom awaiting its switch, with VIEW REQUEST |
| `decline.png`, `declined.png` | 1C:1C declining that removal before its switch, and the request afterwards |

What the run showed:

- A switch that can still be declined comes back after a restart. 1C:1C held one from an
  earlier run, with almost 19 hours left to decline it; once its timebase came, it listed it
  as an event, read, with DECLINE.
- The build before `d4e759c` had not listed it half a minute after the timebase came, before
  the board had sent or heard a packet: nothing published the view when the timebase
  arrived. `d4e759c` publishes it then. The build before `f95af2f` showed it unread, as news;
  `f95af2f` keeps a removal first seen switched quiet.
- REMOVE's slide sent `RemoveDevice` with the phantom's fingerprint (`1a38.log`, 749 s), and
  1C:1C learned the request (`1c1c.log`, 747 s; each log counts from its own board's
  restart). Both switched to generation 6.
- After the switch 1C:1C showed 24:01 left: the protocol's last round to decline ends a day
  and a round after the switch's.
- Removing 1C:1C, the notice reached it about ten minutes after the request, in a sweep round
  under the old key. LEAVE GROUP opened the leave confirmation, and only its slide left the group.
  Paired again, 1C:1C took id 1 back.
- Declining before the switch sent `KeepKey` with the request's key fingerprint. 1C:1C kept
  generation 7 and the phantom, and 1A:38 switched to 8 without it, so the two parted, as a
  decline in a group of two must. They were paired again afterwards (`returning: true`).
- As in earlier runs, every shot halts the core for about 11 s, which the GNSS task counts as
  the module stopping: the GNSS events and toasts in the shots come from that. The shots
  also show pixels past the glass's edge that the panel never shows.
