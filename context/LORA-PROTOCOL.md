# LoRa mesh protocol

The product is a closed group of up to 32 equivalent nodes that share positions and carry messages
between members, including private ones. GPS supplies both position and the time reference; LoRa
carries the traffic. This document is the agreed design. Steps 1 to 4 of the build order below
are implemented, and "Build order" says what comes next; [`AGENTS.md`](../AGENTS.md), "Radio",
says how.

The first version of this design (commit `e57fe0a`) was eight nodes and positions only. The owner
extended it on 2026-09-29 to 32 nodes, messages and private messages, moved it to band O, and kept
fixed slots over contention.

## Radio

| Setting | Value |
| --- | --- |
| Band | O, 869.40–869.65 MHz: 500 mW e.r.p., ≤ 10% duty cycle or polite spectrum access |
| Channel | 869.4625 MHz, 125 kHz |
| Modulation | SF7, CR 4/5, explicit header, CRC on, 8-symbol preamble, sync word `0x6C` |
| Output | +17 dBm on `PA_BOOST`, about 90 mA while transmitting |
| Access | duty cycle, not polite spectrum access |

Band names are those of ETSI EN 300 220-2 V3.3.1 (2025-03), Table 4. ERC Recommendation 70-03 (edition
of February 2025), Annex 1, calls the same band h1.7. In EN 300 220-2, band P is 869.7–870 MHz at 5 mW;
do not confuse the two.

- **Channel.** At 125 kHz band O holds two channels, about 869.4625 and 869.5875 MHz. 869.525 MHz
  is LoRaWAN's default second receive window, where gateways transmit at high power, and
  Meshtastic's EU default at 250 kHz. A LoRaWAN downlink there covers the upper half of our
  channel, and Meshtastic's covers all of it. The sync word filters their packets out, but their
  airtime still collides with ours. Two boards monitoring the channel for three hours found
  nothing else on it (2026-10-02, `docs/logs/lora/crc-2026-10-02/`).
- **Sync word.** `0x6C` (owner, 2026-10-02). The SX127x's reset value `0x12` is what most private
  LoRa setups use, and a network on this channel sends with it: two boards decoded its 34- and
  66-byte packets near -112 dBm. LoRaWAN uses `0x34` and Meshtastic `0x2B`. The sync word is two
  symbols between the preamble and the header, not a byte of the payload, so a node cannot sync
  on a `0x12` inside another network's packet. It only keeps other networks' packets from being
  decoded; their airtime still collides with ours.
- **Power.** The SX1272 allows +17 dBm continuously and limits +20 dBm to 1% duty (datasheet section
  5.4.3). The antenna is TE 2195835-3 (Digi-Key `17-2195835-3-ND`, datasheet
  `docs/datasheets/ENG_DS_2195835_A1.pdf`), a flexible PCB antenna for 863–928 MHz on a 150 mm
  cable: 1.4 dBi peak gain, -4.5 dB average, 35% average efficiency, VSWR under 2.3:1. Digi-Key
  lists 3 dBi; TE's figure governs. Near 869 MHz its plots show VSWR about 1.35 and efficiency
  about 33%. 1.4 dBi is -0.75 dBd, so +17 dBm conducted is about 16 dBm e.r.p. at the peak, far
  under band O's 27 dBm. Band M's 14 dBm e.r.p. allows up to 14 dBm conducted.
- **Close neighbours.** Two nodes under a metre apart at +17 dBm hear each other at about
  -21 dBm, which overloads the receiver: about 1% of packets fail their CRC, against none at
  -45 dBm (`docs/logs/lora/crc-2026-10-02/`), with the AGC on. A node still sends at full power
  to neighbours it hears strongly (owner, 2026-10-02). Otherwise two clusters of users, each
  standing close together, would each turn down for their own close neighbours, and the two
  clusters would never hear each other. And a neighbour that turns down lowers the RSSI the
  decision is made from, so two nodes would steer by each other's settings; undoing that needs
  each packet to carry its sender's power.
- **PA pin.** The antenna is on `PA_BOOST` (2026-10-01, `docs/logs/lora/pa-2026-10-01/`). Two
  boards on one bench hear each other at about -22 to -32 dBm at +17 dBm on it, and its strength
  follows its setting; `RFO` arrives about 70 dB lower whatever its setting. The module exposes
  one `ANT` pin and chooses the PA inside, so only a transmit could answer it. The link test sent
  at `RegPaConfig`'s reset value, +14 dBm on `RFO`, which is why it heard about -100 dBm.
- **Why duty cycle.** Polite spectrum access (listen before talk with adaptive frequency agility)
  caps cumulative transmit time at 100 s an hour per 200 kHz (EN 300 220-2 Table 18), which is 2.8%.
  Band O is 250 kHz wide, so agility cannot add a second 200 kHz. The duty-cycle option allows 10%.
  Listen before talk also defers a transmission when the channel is busy, which breaks a fixed slot.
- **Headroom.** One packet per node per round caps each node at 0.89% duty (below). The 10% is
  unused headroom; what band O buys today is power.
- **Fallback.** Band M, 868.0–868.6 MHz, allows 1% at 25 mW e.r.p. The schedule fits it unchanged,
  at 14 dBm e.r.p. Moving a running group to it is deferred.
- **Today.** The driver default, 868.0 MHz at 125 kHz, straddles the boundary between bands L and M.
  The protocol moves off it.

## Shape

Every node broadcasts what it knows, not just its own position. That removes the routing layer: if A
cannot hear C but both hear B, B's packets carry C's entries to A, over any number of hops, with no
relay logic, seen-set or hop counter for positions. Nodes are expected to go out of range in normal
use.

With 32 nodes the whole table no longer fits a packet (23 entries do), and sending all of it every
round would grow channel load with the square of the group. So a packet carries a partial digest:
the sender's own entry and the entries that most need spreading, chosen as described under "Packet".

A relay passes an entry on in its own next slot. Slots are shuffled every round (see "Medium
access"), so that takes a little over half a round a hop on average, and at most about two.

Messages cannot ride the digest's merge, since each one must arrive, and arrive once. They are
flooded through the same slots with their own ids and seen-set; see "Messages".

## Medium access

GPS-anchored TDMA. Each round's 32 slots are the paired ids, 0–31, in an order of the group's
own, new every round (see "Shuffled slots"). Slots are anchored to absolute UTC,
so two partitions that cannot hear each other compute the same schedule and are already in phase
when they rejoin. A node without a fix takes its time from the arrivals of another node's packets;
see "Keeping time without a fix".

- **Round.** 45 s, 32 slots, 1.40625 s apart. A UTC day holds exactly 1920 rounds. Slot `k` starts
  at the round's start plus `k × 1.40625 s`.
- **Slot.** The packet, at most 400 ms (255 bytes), two ~80 µs TCA9554 writes, and a ±250 ms guard:
  at most 900 ms, leaving about 0.5 s before the next slot. Plain NMEA time is enough for that
  guard, so no PPS is needed; "Time sync" below says where the ±250 ms comes from.
- **Budget.** One packet per node per round: 0.89% duty at the largest packet, 0.40% at an
  eight-entry digest. If all 32 nodes sent an eight-entry digest every round, the channel would be
  busy 13% of the time.

A node transmits in its slot when it has something to say, and stays silent otherwise:

- its own position moved more than ~25 m since the last one it sent, which is above GPS noise
- it learned an entry materially newer than what it last relayed for that node
- a node appeared for the first time
- it holds a message, acknowledgement or member record not yet sent
- the round is one of its floor rounds (below)
- the round is a sweep round (see "Keeping time without a fix")
- it hears no other node, so that another node's one-round sweep finds it wherever their slots
  fall (owner, 2026-10-02). An 85-byte packet every round is about 7 mAh a day, paid only while
  the node is alone

Triggers promote a transmission to the node's next slot rather than sending immediately, so trigger
latency is at most about two rounds, since slots are shuffled. Off-slot transmission has
no audience, because the power saving depends on every node sleeping outside slots.

The floor is every third round, at fixed rounds: node `k` transmits in every round `r` (UTC seconds
divided by 45) with `r mod 3 == k mod 3`, whether or not it has anything new. So every node in range
is heard at least every 135 s, and every node knows in which rounds each id is sure to transmit.

A pending transmission is cancelled when an arriving packet already carries everything it would have
said and that packet's sender reports every neighbour this node has (the neighbours record, below).
Without the cancel, all nodes react to the same event at once. Without the neighbour condition, a
cancel starves a node that only this one reaches. A floor transmission is never cancelled.

As built, the rule works entry by entry. Each entry such a packet carried at the stamp this node
holds counts as sent, and so does its member record, so the node still sends whatever news is
left. A node heard for the first time stays news, since the packet cannot say this node heard it.

### Shuffled slots

Owner, 2026-10-03 (`schedule::Schedule`).

- **Order.** Each round, the 32 ids are sorted by AES-256 of the round's number on the node's
  timebase and the id, under a key HKDF-SHA256 derives from the group key. An id's slot is its
  place in that order. Members on one timebase share the round number, and only members hold
  the key. The round number cannot be the key: every group with a fix shares it, so the order
  would be every group's, and anyone's with GPS time.
- **Exactly.** The key is HKDF-SHA256's 32 bytes from the group key, with no salt and the info
  `octowhere slots`. Each id's block is the round number as eight big-endian bytes, then the
  id, then seven zeros. The ids sort by the first eight bytes of their encrypted blocks, read
  big-endian, then by id. Every device has to compute this alike.
- **Why.** Every group's ids start at 0 and take the lowest free, so with slot = id, member k
  of every group with a fix sends at the same instant, in the same floor rounds, as member k of
  every other. Two groups in one place collide in every slot they share, and a receiver that
  locks onto the other group's packet misses its own. Under their own keys, groups of n₁ and n₂
  share about n₁·n₂/32 slots a round, rarely the same pair two rounds running. A transmitter
  outside the group whose period divides the round hits a different member each round rather
  than the same one for ever. And a listener with GPS time cannot read the sender's id from its
  slot, which it could while a node's slot was its id (see "Packet").
- **What stays.** Floor and sweep rounds go by the round number and the id. The header names
  the slot as before: its base timestamp gives the round, and the order the slot. A notice's
  sender works out the round of the node it notifies from that node's arrival, and its own slot
  in that round from the order.
- **Cost.** A node with news waits up to about two rounds for its slot, a little over half a
  round on average, against at most one round and half on average with slot = id. A relay
  chain in id order no longer crosses in one round. 32 AES blocks a round, cached for the two
  rounds asked about last.
- **Rekeying.** The order's key changes with the group key at the switch time, so a member
  left on the old key sends in other slots from then on. Removal's design has to cover that.

### Time sync

The ±250 ms guard is an assumption carried from the first version of this design, not a
measurement. What limits it is how late the firmware learns that a UTC second began:

- **No PPS.** The LC76G is on I2C and the board marks the GPS UART routes `NC`, so the pulse that
  marks each second's start never reaches the ESP32. Only NMEA says which second it is.
- **NMEA latency.** The module writes RMC after computing the fix. How long after the second that
  is on the LC76G is unmeasured, and the adaptive low-power mode may vary it.
- **Polling.** `sensor_task` used to read GNSS once every 250 ms, so a sentence could wait up to
  about that long. `gnss_task` now reads around each expected burst (below). A read takes about
  11 ms, since the module's protocol puts a 10 ms wait inside it, and a 2 ms pause between reads
  keeps the module from refusing the next one, which costs another 10 ms. So one sighting is late
  by up to about 13 ms.
- **RTC resolution.** The PCF85063A counts whole seconds. Phase within a second between fixes has
  to come from a CPU timer anchored to a sentence's arrival.

Every term makes time late, never early, and every node runs the same hardware and firmware, so two
nodes should disagree by less than either one's worst case.

Ways to tighten it, none of which needs a PPS:

- Read the module often around the expected second instead of every 250 ms. Built: reads start
  100 ms before the burst is due and repeat until it shows.
- Timestamp each RMC's arrival and keep the earliest over many fixes. Arrival jitter only adds
  delay, so the minimum sits closest to the true edge. Built: the earliest of the last 16 fixes,
  with each burst's reads starting 1.6 ms later than the last over 8 bursts, so the read grid
  does not see every burst equally late. A burst counts only after a read found the buffer empty.
- Once a neighbour is heard, align to the group from the `DIO0` arrival times of its packets. That
  is what slots need, and what CAD depends on. Not built; it needs the protocol.

Every I2C task runs on an interrupt executor, so the frame loop's draws do not delay a sighting.
With the GNSS task still sharing the thread-mode executor, its reads took up to 139 ms; now the
slowest read before a burst is about 14 ms. Indoors without a fix, the spacing of bursts as the
task sees them fell from a 20 ms standard deviation to 12 ms, most of which is the module: its
RMCs arrive up to about 30 ms either side of a second after the last. With a fix it is
unmeasured, and the UTC estimate has only been exercised on the host.

The local timer drifts from UTC by its crystal's error between fixes, which the estimate does not
correct.

Two measurements settle how tight it gets, and with it the floor (see "CAD is required at this
size"): RMC's arrival after the second on one board, with its spread, from a fast GNSS read; and
the agreement between two boards synced this way, logged together. Both belong on a
`bench/gnss-time` branch.

### Keeping time without a fix

A node needs a timebase to place slots: GPS time from its own fix, or another node's, taken from
when that node's packets arrive. The owner chose this over running slots on the RTC, whose whole
seconds two nodes can disagree on (2026-10-01). Every header names the sender's timebase:

- **Source.** GPS, or a node's own clock.
- **Root.** For a node's clock, the id of the node that started it.
- **Hops.** How many receptions the sender is from the root: 0 for a node timing from its own fix,
  and for a root.

GPS ranks above any node's clock, and between node clocks the lower root id ranks higher.

- **Taking a timebase.** A node that hears a packet from a timebase ranked above its own adopts
  it: it sets its clock from the packet's arrival, and takes the sender's root and its hops plus
  one. Within its timebase it refines its clock only from packets with fewer hops than its own, so
  two nodes never set their clocks from each other and a hop's error cannot circulate. A node
  timing from its own fix never sets its clock from another's. A node that hears a clock rooted
  at its own id takes it back as its root: it is the node's own clock, kept by the others while
  it restarted. Without that, it would follow its own clock through them as a ghost root, until
  each found the root lost.
- **Arrival timing.** A sender starts its packet at its slot's start, and its id and the header's
  base timestamp name the slot. The receiver takes the time `DIO0` signals RxDone, subtracts the
  packet's airtime, and has that slot's start on its own timer. The latencies on both sides, the
  transmitter's start and the receiver seeing RxDone, would make each hop's clock late by about
  1.05 ms, so the receiver takes that out (measured on two boards, 2026-10-01). A root then hears
  the nodes timing from it within about 0.15 ms, which is their crystals' drift since they last
  heard it.
- **Sweeps.** A node with no timebase listens continuously for three rounds, 135 s, which spans
  every node's floor round. So does a node that has heard no packet with fewer hops in its
  timebase for 7 rounds, and if that sweep hears none either, the node becomes its own root,
  keeping its clock. Every round of a timebase whose index is a multiple of 13, about every
  10 minutes, is a sweep round (owner, 2026-10-02): every node on it listens throughout, from a
  guard before the round to a guard before its end, and sends in its own slot. A node not timing
  from its own fix looks for a timebase ranked above its own, and every node looks for members
  it does not know of (see "Listening"). Nodes on timebases close to one another sweep in rounds
  that overlap, and hear each other there: nodes timing from GPS anywhere, and nodes whose clocks
  started from RTCs a fix once set, which drift apart by a second or two a day. The RTC keeps
  time while the device is off. 13 rounds is not a whole number of floors, so a sweep round
  falls on each of the floor's three rounds in turn. A fix ends a node's first sweep at once. A
  node keeps transmitting in its own slots during a sweep.
- **Starting one.** A node that hears nobody in its first sweep starts its own timebase from its
  RTC's time, as its root. Groups started this way merge as their sweeps find each other, to the
  lowest root. A node that gets a fix moves to GPS time, and the nodes timing from it find it again
  at their next sweep.
- **Refreshing.** REFRESH DEVICES on the screens starts the same three-round sweep at once
  (owner, 2026-10-03), and keeps it to its end though a timebase taken up or a fix would end a
  sweep. It counts the members it heard directly apart from those the group gained while it
  ran, and a pairing stops it. It brought two boards on clocks 271 s apart together from either
  side (`docs/logs/lora/refresh-and-recovery-2026-10-03/`).
- **Notices.** A node that hears a member on a timebase ranked below its own sends it a notice
  (owner, 2026-10-03): a header alone, flagged, 24 bytes, at the time that member listens for
  the sender's slot on its own timebase, which the packet's arrival told it. The notice is sent
  off the sender's slot, so its arrival says nothing of the sender's timebase and nobody times
  from it. A node that hears a notice from a timebase ranked above its own sweeps for three
  rounds, and takes the timebase from the sender's ordinary packets. So two parts of a group find
  each other as soon as either one's sweep hears the other, whichever ranks higher
  (`docs/logs/lora/sweeps-and-notices-2026-10-03/`).
- **Ageing.** A node's own GPS time counts as GPS while a fix has refined it within 30 minutes.
  After that the node ranks as its own root, so a node with a live fix takes the group over.

A node's clock is UTC only as well as its root's RTC was. Its own entries need a fix, so they are
always stamped in GPS time; it relays another's entry only when the entry's stamp fits the 12-bit
window below its base timestamp.

### Listening

A node listens, every round, to the slot of every member its group holds and of every neighbour
(owner, 2026-10-02). A neighbour is a node heard within the last 7 rounds (315 s): long enough to
span two floor transmissions, so one lost packet does not drop it. A member out of range costs
its window every round, and is heard again at its first transmission back in range.

A member added elsewhere, which the node does not know of yet, is found in a sweep round (see
"Keeping time without a fix"), where timebases are found too. Every node on a timebase near the
node's own sends in a sweep round, so a new member in range is heard at the next one, within
about 10 minutes. Once heard it is a neighbour, and the node that heard it asks for its member
record (see "Member records on request").
The sweep rounds keep the receiver on 7.7% of the time, about 18 mAh a day, and the packet each
node sends in them costs about 0.5 mAh a day.

Listening costs the guard and the packet per slot, at 9.7 mA (125 kHz, LNA boost off, its reset
state):

| Slots listened | Receive time | Per day |
| --- | --- | --- |
| 8 | 12% | about 28 mAh |
| 16 | 24% | about 57 mAh |
| 32 | 49% | about 114 mAh |

The table counts a typical packet. As built, a window stays open for the longest packet and
past the one it was for, about 0.9 s, so 8 slots keep the receiver on about 16% of the time.
Ending a window at its guard when no packet started, with the radio's single receive and a
symbol timeout, and at the end of the packet it was for, would bring it below the table.

### CAD is required at this size

Channel activity detection finds a preamble in a couple of symbols, at 5.6 mA for its processing
phase. An empty slot would cost that instead of the whole guard window. It pays only when slot phase
is good to milliseconds, since a ±250 ms guard needs a CAD every few symbols across it. So:

- refine each neighbour's slot phase from the `DIO0` arrival times of its packets
- run CAD at the predicted preamble time, and open the full window only on a detection
- use the header's slot-phase field, reserved for this

With eight nodes this was an optimisation. With 32, listening to every slot without it is half the
day in receive.

CAD shortens discovery only indirectly. How soon a node coming into range is heard is set by how
often it transmits, not by how often others listen. What CAD does is make listening to every slot,
every round, cheap. Then the floor alone sets discovery, and a shorter floor buys it: a floor of
every round costs each node about one 92 ms packet a round, about 4 mAh a day at 90 mA, plus its
neighbours' receive. Two things are unmeasured: what CAD costs across a slot whose phase is known
only to the ±250 ms guard, which a node never heard has, and how closely phase refined against GPS
agrees between nodes that have never heard each other. Decide the floor once both are measured.

Two levers found on 2026-10-03, for when this step comes:

- **A guard per neighbour.** A root hears the nodes timing from it within about 0.15 ms of
  where it expects them (see "Keeping time without a fix"), and `late_us` measures every
  arrival against the node's clock, two nodes timing from their own fixes included. Sized from that, a heard neighbour's window
  shrinks to milliseconds without CAD, which is left for the windows that stay wide: members not
  heard yet, and sweeps. The receive time is read when the radio task runs after `DIO0`, not at
  its edge, which a window of milliseconds has to account for.
- **Sweeps.** They then cost the most: 18 mAh a day, against about 14 for listening to three
  members as built. A member added elsewhere most likely has the lowest id free in this node's
  table, so listening to that id's slot finds it at its next floor round, and nodes timing from
  a fix could sweep less often.

### Why not contention

Contention needs continuous receive, about 9.7 mA or 233 mAh a day, or low-power listening, where
senders stretch the preamble and receivers wake briefly with CAD, paying for it in airtime. It also
brings hidden nodes: A and C both find the channel clear and collide at B. Unique slots rule that
out inside the group.

Pairing is the exception and stays contention-based, because a node being paired has no id and
therefore no slot. The two devices are side by side with a user watching, so the stronger signal
wins a collision and a lost exchange costs a retry. The duty-cycle option needs no channel check.

## Packet

```text
synthetic IV (16) | ciphertext: header (8) + records
```

AES-SIV (RFC 5297, AES-CMAC-SIV with a 256-bit key) under the group key, with no associated data.
The synthetic IV is computed from the key and the whole plaintext, and is both the IV and the
authentication tag, so the packet carries no nonce and nothing in the clear. A sender id in the
clear would tell a direction-finding listener which node transmitted. Shuffled slots (see
"Medium access") keep the slot from telling it, to a listener with GPS time. SIV's
determinism reveals only that two packets are identical, and a packet's timestamp keeps that
from happening. Sending needs no random numbers, so a faulty random source leaks nothing. The
owner chose it over ChaCha20-Poly1305 with a random 12-byte nonce, which cost 12 bytes a packet
more, about 18 ms of airtime (2026-10-01).

Header, 8 bytes, encrypted:

| Field | Bits |
| --- | --- |
| version | 4 |
| sender id | 5 |
| timebase source: 0 GPS, 1 a node's clock | 1 |
| timebase root, for a node's clock | 5 |
| hops from the timebase's root | 5 |
| flags: bit 0 a notice, the rest reserved | 4 |
| base timestamp, timebase seconds | 32 |
| slot phase, reserved for CAD | 8 |

After the header comes a list of records, each a type byte, a length byte and a body. A node skips
any record type it does not know, so a record type can be added without a wire break. Changing the
header still is one.

| Type | Body |
| --- | --- |
| positions | packed entries, padded to a byte |
| neighbours | 32-bit set of the ids the sender heard in its recent rounds |
| members digest | 32 bits of a hash of the sender's member table (see "Member records on request") |
| request | 32-bit set of the ids whose member records the sender asks for |
| member | id, X25519 public key, join time, change time, hardware address, then a name of 1 to 16 printable ASCII characters; 47 to 63 bytes |
| message | see "Messages" |
| acknowledgement | origin id and sequence number of a message delivered to the sender |

Position entry, 73 bits:

| Field | Bits | Note |
| --- | --- | --- |
| id | 5 | |
| latitude | 25 | ~1 m |
| longitude | 26 | ~1 m at the equator |
| time delta below base | 12 | 1 s units, 68 min horizon |
| fix quality | 2 | |
| hdop | 3 | log scale |

With the neighbours and members digest records, which every packet carries:

| Entries | Packet | Airtime |
| --- | --- | --- |
| 0 | 36 B | 77 ms |
| 1 | 48 B | 98 ms |
| 2 | 57 B | 108 ms |
| 8 | 111 B | 190 ms |
| 16 | 184 B | 297 ms |
| 23 | 248 B | 389 ms |

A packet is filled in this order until it is full or nothing is left: the sender's neighbours, its
members digest, a request if it has one, up to three member records it has not sent, the sender's
own entry, acknowledgements, messages oldest first, entries learned since this node last sent them
(newest first), and the rest of the table in rotation. The member records go ahead of the
positions so a busy table cannot crowd them out, but leave room for the sender's own entry; each
costs a full packet about seven entries.

A member record's join time and change time are UTC seconds. The change time is the join's, or a
later rename's, and the newer record of a member wins a merge. A node never takes a record about
itself, matched by key or hardware address.

Sizing fields to their true ranges is the compression. Entropy coding gains nothing on top: the
residual bits are close to uniform and the packets are far too short for a dictionary method. Delta
coordinates against a reference position would save more but need an escape path for a node outside
the delta range; not worth it before the base design flies.

### Member records on request

Owner, 2026-10-03. It replaced a rotation of one member record in every packet
(`members::Requests`).

- **No rotation.** A packet carries a member record only when the sender holds one not yet
  sent: its own after a rename, one that changed in a merge, one enrolled or renumbered, or one
  asked for.
- **Digest.** Every packet carries a members-digest record: 4 bytes of a hash over each
  member's id, public key and change time, 6 with the record's type and length. A packet with
  no positions drops from 85 bytes to 36, and from 149 ms of airtime to 77. A full one has room
  for about seven more entries.
- **Request.** A request record holds the set of ids whose records the sender wants, and rides
  in the sender's own next packet. Unlike a notice's target, its neighbours already listen to
  its slot, and a new record type needs no header change. A node asks for one id when it hears
  a sender it holds no record for. It asks for the whole table when a neighbour's digest has
  differed from its own in two of that neighbour's packets running. Waiting for the second
  gives an ordinary change, which goes out in the next slot, time to arrive.
- **Answer.** A node that hears a request marks each record asked for that it holds as not yet
  sent, unless its digest matches the requester's. The cancel rule marks a record sent once a
  covering packet carried it, so usually one neighbour answers. Records go up to three a packet,
  ahead of the positions but leaving room for the sender's own entry. A whole table of 32 is
  about 11 packets, one a round, about 8 minutes.
- **What it covers.** Everything the rotation did: a member added elsewhere, a member whose
  last acknowledgement the adding device lost, a rename missed out of range, and the duplicate
  id two partitions can hand out (see "Identity and storage").

## Messages

A message record carries:

| Field | Bytes | Note |
| --- | --- | --- |
| origin id | 1 | 5 bits used |
| destination | 1 | an id, or the whole group |
| sequence number | 4 | per origin, never reused |
| timestamp, UTC seconds | 4 | set by the origin, copied by relays |
| body | rest | text, or for a private message its ciphertext and a 16-byte tag |

- **Flooding.** Every node relays each message once, in its next slot, while it is younger than the
  message horizon, 24 hours. The seen-set is keyed by origin and sequence number and holds
  entries until the horizon passes, about 9 bytes each. The origin repeats a message until it is
  acknowledged or expires, doubling the gap between repeats up to 15 minutes.
- **Acknowledgement.** The destination floods an acknowledgement back the same way. A message to the
  whole group is not acknowledged.
- **Latency.** Each hop waits for the relaying node's slot, 1.4 s to one round.
- **Capacity.** Every node carries every message once, so the group's message throughput is what fits
  in one node's packet beside its positions, whatever the group's size. Beside an eight-entry digest
  and the neighbours record, that is about 140 bytes of message body a round, or about 11 kB an hour.
- **Pruning, later.** Gossiping every node's neighbour set gives every node the group's graph. A node
  can then skip relaying a direct message when it is not on a shortest path to the destination, and
  a group message when the sender already reaches all its neighbours. That is where routing starts
  to pay; it builds on the neighbours record rather than adding a layer.

### Private messages

A private message's body is sealed with AES-SIV under a key only its two members hold.
Other members relay it without reading it. Origin, destination and timing remain visible to members,
since the header and record are under the group key.

- **Key.** 256 bits of HKDF-SHA256 over the X25519 shared secret of the two members' keys, bound to
  both public keys. Every member learns the others' public keys from pairing and from member
  records.
- **Associated data.** The origin id and the sequence number, which bind the body to its record.
  SIV needs no nonce. The sequence number must still never repeat for an origin, because it names
  the message in every seen-set. It is persisted in flash in reserved blocks, and a boot skips to
  the next block, so a crash wastes numbers rather than reusing them.
- **Rekeying.** Removing a member is a new group key with a UTC switch time, sent to each remaining
  member as a private message. Before the switch nodes send under the old key, and after it under
  the new one. Every node tries both keys on receive. A node on the new key that hears a member still
  on the old one sends it the new key as a private message in a packet under the old key: the
  removed member can see that packet but cannot open the pairwise-sealed key inside. The old key is
  dropped once every remaining member has been heard under the new one, or after a limit not chosen
  yet.

## Time and freshness

Entries and messages carry absolute UTC seconds, set once by the originator and copied verbatim by
every relay. Relays never recompute them, so error does not accumulate across hops and a clockless
node can forward entries whose freshness it cannot evaluate. Merge keeps the larger timestamp.

A fix and a UTC stamp arrive in the same RMC sentence, so there is no state with fresh position and
no clock to stamp it. A node with no fix has nothing of its own to report anyway.

Absolute timestamps make replay self-defeating rather than something to defend against. A replayed
digest carries its original timestamps, so its entries lose the merge against anything newer, and
where nothing newer exists they are accepted and displayed as what they are: old. This is why there
is no epoch, no per-sender counter and no accept-window policy for positions. Relative ages would
need all three, because a replayed "5 seconds old" is a lie an hour later. A replayed message is
caught by the seen-set within the horizon and dropped by age after it.

What absolute timestamps do introduce is an unbounded top end. An entry stamped far in the future
wins every merge permanently, and a node with a bad clock causes that by accident, not just an
attacker. So:

- Reject entries and messages more than an hour ahead of local time. The gate is the PCF85063A
  oscillator-stop flag, already read in [`src/peripherals/rtc.rs`](../src/peripherals/rtc.rs) and
  exposed as `oscillator_stopped()`. A node with OS set skips the check and re-evaluates on its
  first fix.
- Drop entries past the retention horizon. Old positions are not worth relaying.

An hour of margin passes a node whose RTC has free-run for a year. The margin only has to exceed
worst-case disagreement between two honest clocks; the round period does not enter.

## Security

One group key, which every member encrypts and decrypts packets with, and a pairwise key for each
pair of members, for private message bodies.

A group key proves membership, not identity: a compromised node can forge any other node's entry.
Per-node authenticity would need Ed25519 signatures at 64 bytes each, which does not fit the payload
budget. Revocation is a group rekey.

### Pairing

Display-confirmed X25519, the numeric-comparison pattern. Enroller and joiner exchange public keys,
both screens show a six-digit code derived from the transcript, the user confirms they match, and
the enroller sends, under the ECDH secret, the group key, the joiner's id and the member list with
each member's public key. A MITM has to guess six digits and a failed attempt is visible on screen.
The touchscreen on every node is what makes this available; most LoRa devices have to settle for
trust-on-first-use.

Any member can enrol a new one, so nobody has to carry the founder around. The member list is larger
than one packet, so the exchange spans several.

A node must ignore pairing packets unless the user has explicitly entered pairing mode on the
screen. A device always willing to pair is a permanent unauthenticated attack surface, which would
defeat the point.

The ESP32-S3 has no ECC accelerator, so X25519 runs in software. It runs at pairing and once per
new member for the pairwise key. On the board the frame that runs it, and derives the code and
the key, takes 22 ms (`docs/logs/lora/pairing-2026-10-02/`), so no screen needs to wait on it.

#### The exchange as built

The design hand-off of 2026-10-02 (`design/handoffs/octowhere-pairing-handoff-2026-10-02/`) and
the owner's answers that day settle what this section leaves open. `crates/octowhere-mesh/src/pair.rs`
implements it, with the radio left to the caller.

- **Channel.** Pairing has band O's upper 125 kHz channel, 869.5875 MHz, to itself, with sync word
  `0xA6` and the mesh's modulation (owner). Both devices leave the mesh for the pairing, at most
  about four minutes, so the exchange runs back to back: the transfer of a full group takes
  seconds. Both send at +2 dBm, PA_BOOST's lowest. That is to avoid the receiver overload two
  devices side by side meet at +17 dBm, not a range limit: the screens make no promise of
  distance, and the code is what secures the pairing.
- **In the clear** go only public keys, hardware addresses and the commitment (owner). Names and
  the group's size travel sealed, after both users confirm.
- **Commitment.** The adding device commits to its nonce before it sees the joining device's, as
  Bluetooth's numeric comparison does. Showing a hash of the two public keys alone would let a
  device in the middle try keys offline until the two codes agreed, about a million X25519 key
  generations. With the commitment each attempt it makes has one chance in a million, and a failed
  one shows as differing codes.

| Frame | From | Holds |
| --- | --- | --- |
| announce | joining, every 2 s | its public key, its hardware address |
| offer | adding, until the nonce | the joining device's key, its own, HMAC-SHA256 of its nonce over both keys, its hardware address |
| nonce | joining, until the reveal | the session, its nonce |
| reveal | adding, on each nonce | the session, its nonce |
| sealed | either | the session, then AES-SIV under the pairing key: an accept with the joining device's name, an end with its reason, a part of the group, an acknowledgement, or done |

Every frame starts with a version byte and its kind. The session is the first eight bytes of
SHA-256 over both keys. The code is six digits from SHA-256 over the transcript, the two keys and
both 16-byte nonces, and the pairing key is HKDF-SHA256 of the X25519 secret salted with that
transcript. A key whose shared secret is not contributory ends the pairing.

- **Discovery.** The adding device lists every device announcing, up to four, by hardware address,
  and drops one unheard for 10 s; its user chooses. Search ends after 120 s.
- **Confirmation.** Each user confirms on their own device within 60 s, or declines, or reports that
  the codes differ. An end frame tells the other device which, or that the code timed out, a
  cancel, or a failed store; it is sent three times, a second apart. A device the other never hears
  from again reaches its own deadline.
- **Transfer.** Once the joining device's accept and its own user's confirmation are both in, the
  adding device sends the group key, the joining device's id and every member's record, 226 bytes
  a part. Each part waits for its acknowledgement and is resent a second later without it; 30 s
  without progress loses contact. A full group of 32 is ten parts.
- **Commit order.** The joining device stores the group before it acknowledges the last part. The
  adding device stores the new member on that acknowledgement, then sends done, and stays 5 s to
  answer a repeated acknowledgement. A joining device that stored but never hears done says so:
  it is in the group, with the adding device's receipt unconfirmed. An adding device whose last
  part is never acknowledged has not added the member, and says the outcome is unknown. If the
  joining device did store the group, its own member record reaches the adding device through the
  mesh, at the id it was given. A device that was founding the group has no group to hear that
  under, so it listens throughout for 10 minutes under the founded group's key instead. A packet
  under that key shows the joining device stored the group, and the founding device then stores
  it, and takes it up only once the write lands. A failed write is tried again every 10 s while
  the wait lasts. The joining device is heard within about 3½ minutes: it waits 30 s for
  done, sweeps for three rounds, then sends in its next slot, since it hears nobody. Starting
  another pairing or leaving ends the wait.
- **Capacity.** A full group refuses to add before it searches, a returning device included (design
  hand-off). Below 32, a returning device keeps its id.
- **Founding.** A device in no group that adds one founds a group, with a random key and itself at
  id 0, only when the pairing completes. The first joining device gets id 1.
- **Joining** needs a device in no group. Leaving is its own step, which the screens put first.

Nothing is protected against a cancelled pairing's half-stored state, because nothing is stored
before the commit order above.

### Identity and storage

The protocol id is 5 bits, 0–31. The enroller gives the joiner the lowest id free in its own table.
Two members enrolling in separate places at once can hand out the same id; member records reveal it
when the partitions meet. The member whose public key has the lower SHA-256 keeps the id, and the
other takes the lowest id free in its table and announces it with a member record. The user sees
nothing. An id is freed only by removing its member. Removing is not built: leaving forgets the
group on the leaving device only, so the others keep its record, listen for its slot every
round and never free its id.

The eFuse base MAC is the stable hardware identity, used to recognise a re-pair of the same physical
device rather than issuing a second id.

Group key, id, member table and the sequence-number block persist in a flash partition, not the
SD card, which is removable. They are in the settings' ekv database (`src/settings.rs`): the group
key and this device's id under one key, each member's record under its own, and this device's
X25519 secret and name apart from the group, each value behind a one-byte version. A device keeps
its name and key pair when it leaves a group, and CLEAR SETTINGS keeps all of it (owner,
2026-10-02). A member that changes is stored with the whole group in one transaction. The
sequence-number block comes with messages.

### Flash encryption is deferred

Not in the prototype. It costs an irreversible eFuse burn on boards that still need debugging,
Release mode disables the plaintext UART download that `cargo run --release` relies on, and it
probably breaks `sequential-storage`: XTS encrypts each 16-byte block with its offset as tweak, so
a written block cannot be partially rewritten, while `sequential-storage` does multi-pass writes
within a page. The store is ekv now, and whether it writes a block twice has not been checked.

It also protects less than it appears to. It is not firmware authentication, which is Secure Boot,
and it does nothing against a running device, since the controller decrypts transparently for
anyone with code execution.

The cheaper route when this is revisited: esp-hal exposes the HMAC peripheral in upstream mode,
computing HMAC-SHA256 with a key burned into an eFuse block that software never reads. Derive a
storage key through it and encrypt the blob, and `sequential-storage` stores opaque bytes. Equal
protection against reading a desoldered flash chip, one eFuse burn, no workflow change.

Worth being clear about what any of this buys. A stolen node displays the map on its own screen, so
theft does not need a key and is handled by rekeying. At-rest protection defeats brief undetected
access: someone borrows a node, reads the flash, hands it back, and reads everything afterwards
because nobody knows to rekey.

## Firmware structure

Two changes the protocol needed, both made.

The radio has its own task, `radio_task`. It was a register poll inside the 250 ms sensor loop,
which left the receiver deaf for most of every cycle. That is why the round-trip log shows every
other sequence number.

The I2C bus is a mutex that excludes across cores. It was a `NoopRawMutex`, which would have
failed silently if someone spawned a user on core 1. A task owning the bus was considered and
rejected: it only serialises access, as the mutex does, with a priority layer on top. Priority is
not expected to matter, and a priority-aware mutex adds it if it does. Bus contention is not a
timing risk for slots: the TCA9554 write is about 80 µs and entirely predictable, and the lock can
be taken before the decision to transmit. The longest transaction another user holds it for is a
GNSS read of about 12 ms. Holding it across the packet is unnecessary, since the switch write before
and the restore after are each short with the bus free between them.

## Build order

1. The radio in its own task, and a cross-core I2C lock. Done.
2. Radio settings above, and slots on GPS time with the header and record format, carrying
   positions and neighbours, with a timebase taken from other nodes without a fix. Done, with ids
   from the MAC and a development key until step 3.
3. Pairing, the member table and ids. Done: the exchange, the member table, member records in
   the mesh's packets, their storage, and the screens (`SCREEN-DESIGN-BRIEF.md`, "Group and
   pairing as built").
4. The cancel rule and neighbour-only listening. Built as listening to members and neighbours
   with periodic sweeps (see "Listening"), the cancel rule entry by entry (below), and a changed
   member record sent in the node's next slot.
5. CAD with slot phase refined from arrival times.
6. Messages, then private messages.
7. Pruning relays from the gossiped graph.

What is left goes in this order (owner, 2026-10-03):

- Shuffled slots and member records on request (see "Medium access" and "Packet"), together
  and first. Both change what goes on the air, which is cheapest while there are two boards.
  Both are built and ran on the two boards (`docs/logs/lora/refresh-and-recovery-2026-10-03/`).
- Step 6, ahead of step 5. A new group key goes to each member as a private message, so
  removing a member needs the message machinery: flooding, the seen-set, acknowledgements,
  sequence numbers kept in flash, and the pairwise seal. It is built with removal as its first
  use, then messages on top. Removal and messages need screens, which need a design round; the
  mesh's side goes first, driven over the USB JTAG as pairing's was.
- Step 5, then step 7.

## RTC calibration

Record `(gps_utc, rtc_reading)` pairs on each fix, fit the drift over a long baseline, and program
the PCF85063A offset register. One LSB is 4.34 ppm over a ±273 ppm range, and ±2 ppm is reachable.

Resolution sets the baseline. Without PPS the comparison is good to perhaps half a second, so
resolving one LSB needs about 32 hours and confidence needs a week. It is a background process that
only improves.

Tuning-fork crystals follow roughly -0.034 ppm/°C² away from 25 °C, so one offset calibrated
indoors is about 20 ppm wrong at freezing. Three temperature sources are already on the board
(BMM350, QMI8658, and the SX1272's `raw_temperature`), and the datasheet points at AN11247. The
protocol does not need this.

## Open

- Two parts of a group whose clocks share no origin, such as RTCs that hold no time, sweep in
  rounds that need not overlap. They find each other when a sweep round in either meets a
  packet of the other, since a notice brings the lower one over: up to about 30 minutes for
  idle nodes (`docs/logs/lora/founding-and-listening-2026-10-02/`). A node that hears nobody
  sends every round, which leaves it one sweep.
- The clock takes every packet whose timebase ranks above its own or is closer to its root
  (`Clock::arrival`), with no check that the packet fits. Outside a sweep, the window holds the
  error to the guard. In a sweep, a replayed packet can set the clock anywhere, and the node
  recovers only at its lost sweep, about 10 rounds later; a replayed notice forces a sweep.
  Refining only within the guard, and adopting only on two packets that agree, would close it.
  Jamming does more harm more easily, so it waits.
- The limit after which a rekey drops the old key.
- A shorter floor once CAD is measured (see "CAD is required at this size").
- Measuring GNSS time sync (see "Time sync").

## Deferred

- Tuning for range, the spreading factor and with it the slot length, once the protocol
  carries everything (owner, 2026-10-03). Range has not been measured. SF8 fits today's slot:
  a 255-byte packet takes about 707 ms, which with the 500 ms guard is inside 1,406 ms.
- Moving a running group to the fallback band.
- Messages longer than one packet.
- Flash encryption, as above.
- Delta coordinates against a reference position.
- Temperature-compensated RTC calibration.

## UI obligation

[`AGENTS.md`](../AGENTS.md) requires that a screen not imply data the system does not have. Entry
age has to be visible on the map, not buried in a detail view. A five-minute-old position drawn as a
plain marker is a false claim about where someone is. Likewise a message shows as delivered only
once its acknowledgement arrives; before that it is sent, or heard relayed by a neighbour.
