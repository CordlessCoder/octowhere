# Leaving, messages and removing a member, 2026-10-03

Runs of step 6 of the location mesh on the two bench boards, indoors, with no GNSS fix, on
firmware built with `pair-inject`, `touch-inject` and `rtc-inject`. Commands came from
`tools/pair-inject.py`, and `tools/rtc-inject.py` set the RTCs. The logs are named by the
hardware address each board printed, and are xz-compressed. Times are each board's own, from
its boot. The boards started as a group of two, `1a38` at id 0 ("Dredge") and `1c1c` at id 1
("Roger Roger").

## Leaving

`leave-1a38.log` and `leave-1c1c.log`, on the first build of leaving, before `060efd8`.

- **Telling the group.** `1c1c` left at 758.6 s: the write took 20 ms, and it sent its gone
  record in its next two slots, at 763.5 s and 808.5 s, then forgot the group. `1a38` took the
  first: `member 1 went` at 763.7 s, stored at its next slot (`Gone(1)`, 43 ms).
- **A request for a gone member.** That build's next packet from `1a38` asked for id 1's
  member record, since it held no member there. A gone record now counts as known
  (`members::Requests`), and the later runs ask for nothing.
- **Restart and pairing again** (`pair-1a38.log`, `pair-1c1c.log`, on the fixed build). After
  restarts `1a38` loaded one member, the gone record standing at id 1. `1a38` added `1c1c`
  again, which took id 1, the id its gone record freed (`Added { id: 1, returning: false }`),
  under the pairing's version 2, which carries the key's generation and gone records. The
  packets after carried no records and no requests: the two tables agreed.

## Messages

`messages-1a38.log`, `messages-1c1c.log` and `messages-restart-1c1c.log`, on a build of
the message work before `1a2509f`, without removal.

- **To the whole group.** `1a38` queued text at 281.8 s; its first message stored the end of
  its first block of sequence numbers (`Sequence(65)`, 29 ms) and went out in its next slot,
  at 312.1 s. `1c1c` showed it at 312.1 s.
- **Privately, with an acknowledgement.** `1c1c` sent `1a38` a private message at 313.9 s.
  `1a38` opened it at 337.5 s and acknowledged it, and `1c1c` showed it delivered at 395.0 s.
- **Store and forward.** `1c1c` restarted, losing its store. The two boards met again in a
  sweep round, 406 s into its run. Their messages digests differed, so each sent a summary:
  `1a38`'s at 503.7 s, which `1c1c` had nothing to answer, and `1c1c`'s at 571.2 s, which
  listed nothing. `1a38` answered with all three messages in one 206-byte packet, and at
  579.9 s `1c1c` showed the group message again and its own private one as delivered.

## Removing a member

`remove-1a38.log` and `remove-1c1c.log`, on a build of the removal work before `1a2509f`, which
still sent the removal notice before the switch. With two boards, `1a38` first enrols a
phantom, a member no device stands behind (`pair-inject phantom`), so that `1c1c` is a member
left to tell when `1a38` removes it.

- **The removal.** `1c1c` learned the phantom, id 2, at 150.4 s. `1a38` removed it at
  294.8 s: generation 1, from round 39800063, five rounds on, with one key message for `1c1c`
  and a removal message for the phantom.
- **A lost key message.** `1a38` sent both at 332.4 s in a 155-byte packet, which `1c1c`
  received with a CRC error, as about one packet in a hundred is between boards this close
  (`docs/logs/lora/crc-2026-10-02/`). The digests differed and summaries went both ways, but
  `1c1c`'s summary went out at 385.2 s, by when `1a38` had switched.
- **The switch.** `1a38` switched to generation 1 at 485.4 s, the start of round 39800063,
  and the phantom's id became a gone record. `1c1c` was now a member on the old key.
- **Catching up.** Round 39800072 was a sweep round, and `1a38` heard `1c1c` under the old
  key at 904.5 s and queued its key message. It sent it under the old key at 962.3 s, at its
  own slot in that key's order, a 104-byte packet. `1c1c` took it at 818.5 s on its own clock,
  the same moment: the switch had passed, so it switched to generation 1 at once and the
  phantom's id became a gone record there too. It acknowledged the key message.
- **The old key dropped.** `1c1c` heard `1a38` on the new key and dropped the old one at
  839.4 s, `1a38` at 1010.1 s. Each stored the removal's state at each step: pending, one old
  key, none.
- **The removal notice** went to the phantom before the switch on this build. `1a2509f` sends
  it only after the switch (see below).

## Removing a member, cleanly

`remove-again-1a38.log` and `remove-again-1c1c.log`, on `1a2509f`, which sends the removal
notice after the switch. Both boards restarted with the group at generation 1, and `1a38`
enrolled a second phantom, which took id 2, the id the first one's gone record held.

- **Sequence numbers** went on from 129, the end of the block stored before the restart, so
  none was used twice.
- **The key message** went out at 272.8 s. `1c1c` showed the removal as pending at 128.9 s on
  its own clock, 11.8 s after `1a38` started it, `0 asks to remove 2 ... unless declined`, and
  acknowledged it; `1a38` showed the key message delivered at 333.2 s.
- **The switch.** Both boards switched to generation 2 at the start of round 39800089:
  `1a38` at 448.37 s, and `1c1c` at 304.38 s on its clock, which runs 144.0 s behind.
- **The notice** to the phantom went out 10 s after the switch, under generation 1, at
  `1a38`'s slot in that key's order (`caught=0x00000004`, 57 bytes).
- **The old key** was dropped by `1a38` 24 s after the switch, on hearing `1c1c` under the new
  one, and by `1c1c` at 438.1 s, once it heard `1a38`: a packet `1a38` sent at 430.2 s reached
  it with a CRC error.

## Removing a device

The same logs, from 499 s. `1a38` removed `1c1c` itself, leaving a group of one.

- **Nobody to tell.** With no other member, the removal had no key messages, so the switch
  came five rounds on: generation 3 from round 39800095. `1c1c` heard nothing of it before.
- **The switch** came at 718.4 s on `1a38`, and `1c1c`'s id became a gone record.
- **The notice.** 14 s after the switch, `1a38` sent `1c1c` its removal under generation 2,
  at its slot in that key's order, and `1c1c` showed `0 removed this device from the group` at
  588.6 s on its clock, the same moment. It stays in its old group until its user leaves.

## Declining

The same logs, from 878 s on `1c1c`, after it left its old group and paired again
(`1a38` adding, `1c1c` at id 1). `1a38` enrolled a third phantom and removed it.

- **Declined.** `1c1c` showed `0 asks to remove 2: generation 4 from round 39800104 ... unless
  declined` at 878.3 s, and its user declined at 879.0 s (`pair-inject keep`): it kept the
  phantom and its key. It had acknowledged the key message as it showed it.
- **The switch.** `1a38` switched to generation 4 alone at 1123.4 s and told the phantom 37 s
  later.
- **Catch-ups.** `1a38` heard `1c1c` under generation 3 in sweep rounds at 1300.7 s, 1446.9 s
  and 2030.5 s, and each time sent it its key message under that key, which `1c1c` already
  held and left alone. The commit after these runs sends a member its key message at most
  three times for each old key, and acknowledges a key message only when it shows a removal:
  this build would have sent it in every sweep round for as long as `1c1c` stayed in range.

## After the review

`fixes-1a38.log`, `fixes-restart-1a38.log` and `fixes-1c1c.log`, on `0cfb166`, which closes
the gaps a review of step 6 found. `1c1c` left the group it had kept by declining, and `1a38`
added it again at its old id (`returning: true`).

- **Numbers from the clock.** `1a38`'s first message after the update took sequence number
  1791005948, its timebase second, and the block was stored before the message was made.
- **Adding waits.** An `add` on `1a38` while its removal of a fourth phantom was pending was
  refused: `a removal is under way; adding waits for its switch`.
- **A remover that restarts.** `1a38` was restarted 3 s after starting the removal, before its
  key message went out. 8 s after boot it had its timebase back and posted the key message
  again, from the removal it had stored, under the first number of a fresh block
  (1791006012).
- **Lost again, then caught up.** That 122-byte packet reached `1c1c` with a CRC error.
  `1a38` switched to generation 5 alone at 192.1 s. The switch round, 39800137, was a sweep
  round: `1a38` heard `1c1c` under generation 4 at 216.1 s, and sent it its key message
  under that key at 236.0 s, in one packet with the phantom's removal notice
  (`caught=0x00000006`).
- **Time to decline.** `1c1c` learned of the removal after its switch round, and showed it as
  pending until round 39800140, three rounds on, instead of switching at once. It
  acknowledged the key message and switched at the start of that round, at 536.0 s.
- **The old key dropped.** `1a38` heard `1c1c` on the new key and dropped the old one at
  349.8 s, and `1c1c` dropped its own at 623.3 s.

## CRC errors between these boards

`1c1c` received 12 of the 37 packets `1a38` sent it in these runs with a CRC error, about one
in three of those over 100 bytes, at -17 to -30 dBm. `1a38` received all 35 of `1c1c`'s, at
about -10 dBm. Overload alone would fail the stronger link first. On 2026-10-02 the errors
ran the other way, at about 1% (`docs/logs/lora/crc-2026-10-02/`). Every loss here was made
good by the protocol: summaries, member record requests and catching up a member on its old
key. Whether `1a38`'s transmitter or `1c1c`'s receiver is at fault, or how the boards sit on
the bench, is not looked into.

## Not run on the boards

- Two removals at once, and the lower key winning after a switch to the higher, need three
  devices; the mesh crate's tests cover them (`rekey.rs`).
- A member sent its key message three times under an old key and no more: `1c1c` declining
  showed the catch-ups, on the build before the limit.
- The REMOVING refusal of a pairing was seen in the log only, not on the panel.
- Messages longer than a few words, a full store, and summaries too short for every origin:
  the crate's tests cover them (`messages.rs`).
