# Founding without the last acknowledgement, and step 4, 2026-10-02

Two runs on the two bench boards, indoors, with no GNSS fix and no RTC time, on firmware built
with `pair-inject` and `touch-inject`. Commands came from `tools/pair-inject.py`. The logs are
named by the hardware address each board printed, and are xz-compressed.

## A founder that loses the last acknowledgement

`founding-1a38.log` and `founding-1c1c.log`. Both boards left their group first. `1a38` added
and `1c1c` joined, and both showed code 013 649.

- **Losing it.** `1c1c` confirmed. `1a38` was then made deaf for 45 s (`pair-inject.py deaf 45`)
  and confirmed, so it sent the group's one part but dropped every acknowledgement. `1c1c`
  stored the group in 51 ms and waited 30 s for done, then ended joined and unconfirmed.
  `1a38` ended unconfirmed and began listening under the founded group's key.
- **Reconciling.** `1c1c` swept for three rounds, heard nobody, became its own root and sent in
  its next floor round. `1a38` heard that packet 245 s after it ended, took its timebase and
  stored the group, with itself at id 0 and `1c1c` at id 1, in 32 ms. Each then heard the other.
- **The overflow.** About 45 s later `1c1c` panicked: esp-hal's stack guard caught a write below core
  0's stack, in `Mesh::publish` under the radio task. That build left core 0 59,740 bytes of
  stack. The frame loop on a group screen had about 35 KB in four frames, and the radio task,
  which interrupts it on the same stack, 17.4 KB in its poll and 7.3 KB to publish the view.
  The founder's state had grown the radio task's frame and its static future. The build in the
  second run fills the view and the group screens' list where they live on the heap, and moves
  48 KiB of the heap to the stack, which leaves it 115,724 bytes.

## Step 4

`listening-1a38.log` and `listening-1c1c.log`, on the build with that fix. Both boards were
reset together, already a group of two: `1a38` at id 0, named `Dredge`, and `1c1c` at id 1.

- **Starting together.** Both swept for three rounds and heard nothing, since a node with no
  timebase does not send. Each became its own root. Their clocks both started from boot, so
  their slots fell close enough together that `1a38` heard `1c1c` in its window 44 s later, and
  `1c1c` took `1a38`'s timebase, root 0, a round after that. Boards started further apart would
  find each other only at a sweep.
- **A changed member record.** `1a38` was renamed `Dredge 2`. It sent in its next slot, which
  was not a floor round, with its own record first. `1c1c` took the name and did not send in
  its own next slot, which was not its floor round either: `1a38`'s packet had already reached
  everyone `1c1c` hears. `1a38` was renamed back to `Dredge`, which reached `1c1c` the same way.
- **The sweep.** 13 rounds after the first sweep ended, each board listened throughout for its
  round 17. `1c1c` went on through round 18 as well: a sweep ended only when the node next
  ticked its clock, once a round, and its tick fell just short of the end. A sweep now ends at
  its time, whenever the node ticks.

## Starting apart

`apart-1a38.log` and `apart-1c1c.log`, on the build with the sweep ending at its time. The
boards were reset about 23 s apart, as their packets' arrivals later showed.

- **Two roots.** Each swept for three rounds, heard nothing and became its own root. Their slots
  were too far apart for either to hear the other in its windows.
- **Finding each other.** At the first one-round sweep, 13 rounds later, `1a38` heard `1c1c`,
  a node new to it, and sent in its next slot. `1c1c`'s own sweep was still under way, so it
  heard that packet and took `1a38`'s timebase. Neither sweep lasted more than a round.
- **What it leaves open.** The two sweeps overlapped because the boards started close together.
  Where sweeps do not overlap, the lower root hears the higher only when one of its own sweeps
  meets a packet of the other's. For an idle node that takes up to three sweeps, about
  30 minutes. Before step 4, a node listened to nearly every slot, and found the other within a
  few of its packets.

Heap use after boot was 17,944 bytes. Nothing here exercises the cancel rule's positions: with
no fix, neither board has a position to send.
