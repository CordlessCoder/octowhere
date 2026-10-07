# LoRa mesh protocol

The product is a closed group of up to 32 equivalent nodes that share positions and carry messages
between members, including private ones. GPS supplies both position and the time reference; LoRa
carries the traffic. This document is the agreed design. Steps 1 to 4 and 6 of the build order
below are implemented, on contention since 2026-10-05, which dropped step 5; step 7 closed on
2026-10-07. [`AGENTS.md`](../AGENTS.md), "Radio", says how it is built.

The first version of this design (commit `e57fe0a`) was eight nodes and positions only. The owner
extended it on 2026-09-29 to 32 nodes, messages and private messages, moved it to band O, and kept
fixed slots over contention. On 2026-10-05 the owner moved it to contention; the `tdma` branch
holds the slots as they were built (see "Medium access").

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
  A node checks the channel before it sends; under the duty-cycle option that check is neither
  required nor forbidden.
- **Headroom.** A node rests nine airtimes after each packet but its own key messages, and holds
  its airtime in any hour under 360 s. EN 300 220-2 V3.3.1 measures band O's 10% over an hour
  (clause 4.4.3) and sets no limit on a single transmission there. A packet a round, at the largest, is 0.89%; messages and records use the rest.
- **Fallback.** Band M, 868.0–868.6 MHz, allows 1% at 25 mW e.r.p., at 14 dBm e.r.p. The rest after
  each packet would have to be 99 airtimes there. Moving a running group to it is deferred.
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

A relay passes an entry on in its next packet: at most a round after its last, or at once beside
a record (see "Medium access").

Messages cannot ride the digest's merge, since each one must arrive, and arrive once. They are
flooded in the same packets with their own ids, and every node holds them for a day and passes
on any a neighbour lacks; see "Messages".

## Medium access

Contention, CSMA/CA, on continuous receive (owner, 2026-10-05). It replaced GPS-anchored slots,
for message latency and to drop the slot timing. The slots, their shuffle, the listening windows
and the notices are on the `tdma` branch, at `146fd7a`, with this document as it described them.
A node in a group keeps its receiver on throughout and sends when it has something to say and
the channel is clear (`octowhere-node`'s `access`).

- **When a packet is due.** At once when the node holds records: a message, key messages
  among them, or a member or gone record, not yet sent (owner, 2026-10-05). At a time drawn up to 5 s ahead when it holds
  a request or a summary, which ask for what it lacks: nodes that cannot hear each other often
  answer one packet together, and the backoff below keeps a node apart only from those it
  hears. Two such nodes, both answering a packet from a node between them, collided in the
  simulator. A round after its last
  packet when it holds only news: its own position moved more than ~25 m, which is above GPS
  noise, it learned an entry materially newer than what it last relayed, a node appeared, or
  its word that it is on a new key is to go in its first packets after a switch. And at the
  floor, 130 s after its last packet, with nothing new. The round and the floor each come up to
  10 s early, drawn at each packet, so that nodes that started together drift apart rather than
  contend at every floor.
- **The backoff.** Once due, a packet waits a random 0 to 31 steps of 10 ms, listening. A packet
  heard meanwhile ends the wait, and the node looks again at what is due, since the cancel rule
  below may have taken its news. At the end of the wait it checks the channel, and sends at once
  if it detects no packet under way. Otherwise it waits a random 0 to 63 steps, past the longest
  packet, and tries again.
- **A step** is long enough for the modem to detect another node's preamble, about five symbols,
  and for this node's own transmission to start. Two nodes due at once collide only when they
  draw the same step.
- **Duty.** After each transmission a node is silent for nine times its airtime, which keeps it
  under band O's 10% at any moment. A packet carrying the node's own key messages is the
  exception (owner, 2026-10-07): a removal's switch waits on every key message leaving the
  remover, and in reach the rest after each was what made a removal take minutes. The band
  measures its 10% over an hour, so a node keeps its airtime in 5-minute slices and holds any
  packet that would take the last 13 slices past 360 s. A removal from a group of 32 is about
  12 s of key messages.
- **The floor** is a little under three rounds, so that a node listening for three rounds, as
  in a first sweep, hears every node in reach.
- **Sweep rounds** are every round of a timebase whose index is a multiple of 13, about every
  10 minutes. Every node listens throughout anyway; what is left of them is a cadence for a
  removal (see "Removing a member"), and a clock that refines from a packet more than 250 ms off
  only in one (see "Replays"). In one, a node sends its word that it is on the group's key for a
  day after a switch, and a header under the old key while it waits for a member, each at a time
  it draws in the round's first half.

A pending transmission is cancelled when an arriving packet already carries everything it would
have said and that packet's sender reports every neighbour this node has (the neighbours record,
below). Without the cancel, every node that heard a message would pass it on. Without the
neighbour condition, a cancel starves a node that only this one reaches. A floor is never
cancelled.

As built, the rule works entry by entry. Each entry such a packet carried at the stamp this node
holds counts as sent, and so do its member records and messages, so the node still sends whatever
news is left. A node heard for the first time stays news, since the packet cannot say this node
heard it. The rule is only as good as the neighbour table: a node that has not yet heard one of
its neighbours cancels a relay that neighbour needed, and the message summaries bring it back
later.

Measured in the simulator with every node holding a fix
(`docs/logs/lora/rotation-2026-10-07/`). `crates/octowhere-sim/tests/contention.rs` asserts
looser bounds on nodes without fixes.

- A group message reaches 31 other nodes in reach in 0.2 to 0.5 s over four seeds, in the
  origin's one packet: it covers everyone, so nobody relays it.
- Along a line of twelve, each node in reach of the next only, it takes 3.4 to 4.3 s from end to
  end, about 0.35 s a hop.
- 32 nodes in reach that stay put keep the channel 2% busy, and lose 0.3% of receptions to two
  nodes sending at once. Moving 44 m every 30 s, they keep it 8% busy and lose 0.9%.
- In grids and scattered groups, where most nodes reach only a few others, 2 to 3% of
  receptions are lost to packets that overlap. Two nodes that cannot hear each other both find
  the channel clear, and the longer their packets, the more often they overlap at a node
  between them.

Every header names its sender's timebase and when the packet started on it (see "Keeping time
without a fix"). Nothing else ties a packet to a time.

### Time sync

Contention needs UTC only to seconds: for record and message stamps, the message horizon and a
removal's switch round. What limits how well GNSS gives it is how late the firmware learns that a
UTC second began:

- **No PPS.** The LC76G is on I2C and the board marks the GPS UART routes `NC`, so the pulse that
  marks each second's start never reaches the ESP32. Only NMEA says which second it is.
- **NMEA latency.** The module writes RMC after computing the fix. How long after the second that
  is on the LC76G is unmeasured, and the adaptive low-power mode may vary it.
- **Polling.** `gnss_task` reads around each expected burst, from 100 ms before it is due until it
  shows. A read takes about 11 ms, since the module's protocol puts a 10 ms wait inside it, and a
  2 ms pause between reads keeps the module from refusing the next one, which costs another
  10 ms. So one sighting is late by up to about 13 ms.
- **RTC resolution.** The PCF85063A counts whole seconds. Phase within a second between fixes has
  to come from a CPU timer anchored to a sentence's arrival.

Every term makes time late, never early. The estimate keeps the earliest of the last 16 fixes'
arrivals, with each burst's reads starting 1.6 ms later than the last over 8 bursts, so the read
grid does not see every burst equally late; arrival jitter only adds delay, so the minimum sits
closest to the true edge. A burst counts only after a read found the buffer empty. Indoors without
a fix, the spacing of bursts as the task sees them has a 12 ms standard deviation, most of which
is the module: its RMCs arrive up to about 30 ms either side of a second after the last. With a
fix it is unmeasured, and the UTC estimate has only been exercised on the host. The local timer
drifts from UTC by its crystal's error between fixes, which the estimate does not correct.

### Keeping time without a fix

A node needs a timebase for UTC: GPS time from its own fix, or another node's, taken from when
that node's packets start. The owner chose this over the RTC alone, whose whole seconds two nodes
can disagree on (2026-10-01), and kept it when slots went (2026-10-05), so that a node without a
fix still stamps its records and times a removal's switch. Every header names the sender's
timebase:

- **Source.** GPS, a node's clock started from its RTC's UTC, or a node's clock started from
  its boot, by a node whose RTC held no time.
- **Root.** For a node's clock, the id of the node that started it.
- **Hops.** How many receptions the sender is from the root: 0 for a node timing from its own fix,
  and for a root.

GPS ranks above any node's clock, a clock started from UTC above one started from a boot, and of
two clocks alike the lower root id ranks higher (owner, 2026-10-04). A clock started from a boot
counts its seconds from 1970, so ranked by its root alone it took a whole group there, and every
node, those whose RTCs held the time too, refused records stamped in 2026 as an hour ahead.

- **Taking a timebase.** A node that hears a packet from a timebase ranked above its own adopts
  it: it sets its clock from the packet's start, and takes the sender's root and its hops plus
  one. Within its timebase it refines its clock only from packets with fewer hops than its own, so
  two nodes never set their clocks from each other and a hop's error cannot circulate. A node
  timing from its own fix never sets its clock from another's. A node that hears a clock rooted
  at its own id takes it back as its root: it is the node's own clock, kept by the others while
  it restarted. Without that, it would follow its own clock through them as a ghost root, until
  each found the root lost.
- **Replays.** A packet's timing says where its sender's clock stood when it was sent, so a
  recording replayed later moves a clock by its age (owner, 2026-10-04):
  - A node whose RTC holds the time refuses a timebase more than 5 minutes from it, and one
    started from a boot. An RTC drifts a couple of seconds a day, so the bound holds for months
    without a fix.
  - A packet that would move a clock the node already has by more than 250 ms waits, held, for
    a second that agrees within that: from another sender, or from the same in a later round,
    within a floor and a round. Outside a sweep, a packet from closer to the root that far off
    is not even held. A replay of two recorded packets still gets through, within the bound
    where the node has one.
  - A node's first timebase is no move of a clock it has, so only the bound guards it.
  Replayed an hour on, a packet had set a node's clock an hour back, which the node then kept as
  its own root; in a sweep round it lost the sender for about 340 s.
- **Arrival timing.** A sender names in its header when its packet starts on its timebase: the
  whole second, and where in it in 256ths, about 4 ms each. The receiver takes the time `DIO0`
  signals RxDone, subtracts the packet's airtime and 1.05 ms of latency on both sides, measured
  on two boards when packets went in slots (2026-10-01), and has that start on its own timer. A
  sender names the time before it checks the channel and loads the packet, which adds about a
  millisecond, more when the I2C bus is held.
- **Sweeps.** A node with no timebase listens for three rounds, 135 s, which spans every node's
  floor, before it starts its own. So does a node that has heard no packet with fewer hops in its
  timebase for 7 rounds, and if that sweep hears none either, the node becomes its own root,
  keeping its clock. A fix ends a node's first sweep at once. A node keeps sending during a
  sweep once it has a timebase.
- **Starting one.** A node that hears nobody in its first sweep starts its own timebase from its
  RTC's time, as its root, or from its boot if its RTC holds no time. A node whose clock was
  started from its boot takes UTC from a timebase started from UTC, for its own records' and
  messages' stamps; the RTC is GNSS's alone to set (owner, 2026-10-04). Without one it stamps
  its records 0, which lose every merge. Groups started this way merge to the lowest root as
  soon as they hear each other. A node that gets a fix moves to GPS time, and the nodes timing
  from it follow at its next packet when their clocks are within 250 ms of it, and otherwise
  once a second packet agrees, another sender's or its own in a later round.
- **Refreshing** is retired (owner, 2026-10-05; `design/DECISIONS.md` 33 and 34). Every node
  listens throughout, so every member in reach is heard within a floor without one.
- **Ageing.** A node's own GPS time counts as GPS while a fix has refined it within 30 minutes.
  After that the node ranks as its own root, so a node with a live fix takes the group over.

A node that restarted with no UTC while a removal was pending hears a group that has switched
only under the key it is to switch to. It takes nothing else from such a packet, but moves to its
clock if it outranks its own: without that it stayed on its boot clock, where the switch round it
stored never came.

A node's clock is UTC only as well as its root's RTC was. Its own entries need a fix, so they are
always stamped in GPS time; it relays another's entry only when the entry's stamp fits the 12-bit
window below its base timestamp.

### Listening

A node in a group receives throughout, so a member in reach is heard at its first packet, within
a floor. Continuous receive at 9.7 mA is about 233 mAh a day, 47% of the two-day budget read as
the whole device; [`POWER-INVESTIGATION.md`](POWER-INVESTIGATION.md) sets that against the rest
of the device and has low-power listening, with CAD and long preambles, as the route if it is
too much. A node in no group keeps the radio asleep.

### Why contention

The owner moved the mesh to contention on 2026-10-05, after the power investigation of
2026-10-04 found the radio's share likely small beside the rest of the device. Slots made a
message wait about half a 45 s round a hop, and placing them took the timebase's millisecond
timing, the shuffle, windows and guards, sweeps as a way of listening, and notices.

What contention costs:

- **Power.** Continuous receive, above.
- **Hidden nodes.** Two nodes that cannot hear each other both find the channel clear and collide
  at a node between them, and broadcasts have no RTS/CTS. Capture needs the wanted packet 6 dB
  stronger. The message summaries repair what is lost.
- **Late sensing.** The modem detects a preamble only a few symbols in, so a check misses a packet
  that started just before it. Between the check and the transmission the node loads its packet
  and switches the RF path, an I2C write that a GNSS read could hold up for about 12 ms, and a
  packet that started meanwhile went unseen. The node now takes the I2C bus before the check and
  releases it once the transmission has started, so the gap is the SPI load and one I2C write.

Pairing sends without checking the channel: the two devices are side by side with a user
watching, so the stronger signal wins a collision and a lost exchange costs a retry. The
duty-cycle option needs no channel check.

## Packet

```text
synthetic IV (16) | ciphertext: header (8) + records
```

AES-SIV (RFC 5297, AES-CMAC-SIV with a 256-bit key) under the group key, with no associated data.
The synthetic IV is computed from the key and the whole plaintext, and is both the IV and the
authentication tag, so the packet carries no nonce and nothing in the clear. A sender id in the
clear would tell a direction-finding listener which node transmitted, and on contention a
packet's time says nothing of its sender either. SIV's determinism reveals only that two packets
are identical, and a packet's timestamp keeps that from happening. Sealing needs no random
numbers, so a faulty random source leaks nothing. The
owner chose it over ChaCha20-Poly1305 with a random 12-byte nonce, which cost 12 bytes a packet
more, about 18 ms of airtime (2026-10-01).

Header, 8 bytes, encrypted:

| Field | Bits |
| --- | --- |
| version, 2 since contention | 4 |
| sender id | 5 |
| timebase source: 0 GPS, 1 a node's clock | 1 |
| timebase root, for a node's clock | 5 |
| hops from the timebase's root | 5 |
| flags: bit 1 a node's clock started from its boot, the rest reserved (bit 0 was a notice) | 4 |
| base timestamp: the second the packet starts in, timebase seconds | 32 |
| phase: where in that second it starts, in 256ths | 8 |

After the header comes a list of records, each a type byte, a length byte and a body. A node skips
any record type it does not know, so a record type can be added without a wire break. Changing the
header still is one.

| Type | Body |
| --- | --- |
| positions | packed entries, padded to a byte |
| neighbours | 32-bit set of the ids the sender heard in its recent rounds |
| members digest | 32 bits of a hash of the sender's member table (see "Member records on request") |
| request | 32-bit set of the ids whose member records the sender asks for |
| member | id, Ed25519 public key, join time, change time, hardware address, signature, then a name of 1 to 16 printable ASCII characters; 112 to 127 bytes |
| gone | id, Ed25519 public key, change time and signature of a member that left; 101 bytes |
| on key | id, generation and the member's signature that it is on that generation's key; 67 bytes |
| message | see "Messages" |
| messages digest | 32 bits of a hash of the messages the sender holds (see "Messages") |
| summary | the messages the sender holds from each origin, to be sent what it lacks (see "Messages") |

Position entry, 73 bits:

| Field | Bits | Note |
| --- | --- | --- |
| id | 5 | |
| latitude | 25 | ~1 m |
| longitude | 26 | ~1 m at the equator |
| time delta below base | 12 | 1 s units, 68 min horizon |
| fix quality | 2 | |
| hdop | 3 | log scale |

With the neighbours and members digest records, which every packet carries but those a leaving
device sends and those under an old key:

| Entries | Packet | Airtime |
| --- | --- | --- |
| 0 | 36 B | 77 ms |
| 1 | 48 B | 98 ms |
| 2 | 57 B | 108 ms |
| 8 | 111 B | 190 ms |
| 16 | 184 B | 297 ms |
| 23 | 248 B | 389 ms |

A packet is filled in this order until it is full or nothing is left: the sender's neighbours, its
members digest, its messages digest while it holds a message, a request if it has one, its
on-key record (see "Removing a member"), a summary if it has one and no message to send, up to
three member and gone records it has not sent, the sender's own entry, messages (see
"Flooding"), and the entries learned or changed since this node last sent them, newest first.
The member records go ahead of the positions so a busy table cannot crowd them out, but leave
room for the sender's own entry. A member record costs a full packet 12 to 14 entries, and a
gone record 11.

A packet carried the rest of the table in rotation after those until 2026-10-07, when the owner
dropped it: with every node holding a fix, it filled every packet. In the simulator a group in
reach then kept the channel 10% busy standing still and 31% moving, against 2 and 8% without
it, and in grids and scattered groups 10 to 18% of receptions were lost to overlapping packets,
against 2 to 3%. Every node still showed every other's position within each run; a few distant
pairs in grids showed theirs later at first (`docs/logs/lora/rotation-2026-10-07/`).

A member record's join time and change time are UTC seconds. The change time is the join's, or a
later rename's or move to another id's, and the newer record of a member wins a merge. A node
never takes a record about itself, matched by public key.

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
  in the sender's own next packet, at the time it draws for it (see "Medium access"). A new
  record type needs no header change. A node asks for one id when it hears a sender it holds no
  record for. It asks for the whole table when a neighbour's digest has differed from its own
  for a round since the last of that neighbour's packets that agreed or brought it a record new
  to it (owner, 2026-10-05). A change on its way has that long to arrive, and a neighbour still
  bringing records is not asked. Counted in packets, as slots did, two came seconds apart on
  contention, and every node asked at once.
- **Answer.** A node that hears a request marks each record asked for that it holds as not yet
  sent, unless its digest matches the requester's. The cancel rule marks a record sent once a
  covering packet carried it, so usually one neighbour answers. Records go up to three a packet,
  ahead of the positions but leaving room for the sender's own entry; signed, about one fits.
  A whole table of 32 is about 32 packets, each once the rest after the last allows: about 80 s.
- **What it covers.** Everything the rotation did: a member added elsewhere, a member whose
  last acknowledgement the adding device lost, a rename missed out of range, and the duplicate
  id two partitions can hand out (see "Identity and storage").

## Messages

Owner, 2026-10-03: store and forward, the body, and keeping messages in PSRAM. The design it
replaced had relays pass each message on once and the origin repeat it until acknowledged. A
repeat carried the same sequence number, so relays dropped it, and it reached only the origin's
own neighbours.

A message record carries:

| Field | Bytes | Note |
| --- | --- | --- |
| origin id | 1 | 5 bits used |
| destination | 1 | an id; bit 7 the whole group instead; bit 6 a group key (see "Removing a member") |
| sequence number | 4 | per origin, never reused, never 0 |
| previous | 4 | the origin's sequence number before this one, 0 for none |
| timestamp | 4 | timebase seconds, set by the origin, copied by relays |
| body | rest | a kind byte and what it holds; to one member, sealed (see "Private messages"); a key message's seal is followed by its generation and its remover's signature (see "Signatures") |

A message to one member is always private. The kinds are text, an acknowledgement, a new group
key, and a removal.

- **Body.** Text is printable ASCII, the fonts' characters, up to 160 of them. A private message
  that long fills one packet with the sender's own entry and the records every packet carries.
- **Flooding.** A node relays each message new to it once, as soon as the channel lets it,
  oldest first. It counts as sent once a covering packet carried it, as an entry does (the
  cancel rule). A message the node sends again goes behind every one its own packets never
  carried (owner, 2026-10-07): a removal's switch is timed on each key message leaving the
  remover once, and oldest first put the remover's sends again ahead of key messages still to
  go, past the switch.
- **Hearing it passed on.** A neighbour that has neighbours of its own outside the ones a
  packet's sender reports passes the packet's new messages on, by the cancel rule, so hearing it
  do so is the sign it heard them (owner, 2026-10-05; `octowhere-mesh`'s `relays`). After
  sending a message, a node waits 10 s to hear each such neighbour carry it, or a later message
  from its origin, or another relay reach that neighbour's neighbours. A neighbour already heard
  carrying it is not waited for. A message not heard passed on goes again once (owner,
  2026-10-07; three times before). Where two neighbours hidden from each other relay at once,
  they collide at the sender, and its sends again went to neighbours that held the message
  already. The summaries repair a loss a send again misses, more slowly: along a chain that
  loses one packet in five, a removal took longer.
  Along a chain of relays a loss on one hop stops the flood there; this repairs it in seconds,
  where waiting for the loss to show took a floor.
- **Store and forward.** Every node holds every message for the message horizon, 24 hours
  from its timestamp, private ones included, and passes on any a neighbour lacks. The origin
  does not repeat it. A message reaches a member who comes back into range of anyone holding it
  within the horizon.
- **Digest.** A packet carries a messages digest while its sender holds a message: 32 bits of
  a hash over the origin and sequence number of each one not yet past the horizon. A message
  passes the horizon at a round's start, the same for every node on a timebase, so two nodes'
  digests agree when they hold the same messages. A node judges that against its own clock,
  not the time in the header of the packet that carried the message.
- **Summary.** A node sends a summary of the messages it holds: for each origin, the oldest and
  newest sequence numbers it holds, and those it knows it lacks between them. A message names
  the origin's one before it, which is how a node knows. Sequence numbers skip at a restart, so
  a gap in the numbers alone says nothing. The first message after a restart names none, since
  the origin's store went with the restart, so a node counts every number below it, down to the
  one it holds before, as possibly lacked (2026-10-06). The origin's last message before the
  restart then comes back from a neighbour that holds it; a range nobody holds anything in
  costs eight bytes of the summary and brings nothing. A neighbour that hears a summary marks to be sent
  every message it holds outside those ranges or among those lacked, unless its digest matches
  the summary's sender's. A summary takes at most 120 bytes; one too short for every origin says
  which it covers, and the next starts where it stopped. It is made before the backoff, since
  with a full store that takes milliseconds. Four things make one due (owner, 2026-10-05):
  - a neighbour's digest has differed from its own for a round since the last of that
    neighbour's packets that agreed or brought it a message new to it, as records are asked for;
  - it takes a message whose origin's one before it, which it names, it does not hold: a message
    lost on the way, which a node hidden from the sender's next relay loses;
  - a neighbour's summary shows it holds the newest message from an origin that this node
    lacks, which no gap shows;
  - no message new to it has arrived for 10 s, and the last packet a neighbour brought one in
    left its digest unlike this node's: the neighbour held more than it gave. In a flood a relay
    holds the next message while it sends this one, so this shows a lost last message.

  A summary waits while the node has messages to send. Before it did, a remover with 30 key
  messages to send filled its packets with summaries instead, while every other node sent it
  summaries too: after almost nine minutes, 17 of 30 members held their key messages.
- **Acknowledgement.** The destination of a private message answers with an acknowledgement,
  itself a private message, which travels and is held the same way. Acknowledgements, key
  messages and messages to the whole group are not acknowledged. A key message stays out of
  the inbox, so nothing would read its acknowledgement, and a member's signed word that it is on
  the new key is what ends the wait for it (owner, 2026-10-07). Acknowledged, they were half of
  a removal's messages.
- **Storage.** Messages are held in PSRAM and lost at a restart, to begin with. A restarted
  node gets the horizon's messages back from its neighbours, but not which it had read. The
  store holds the newest 256; a node holding that many takes no message older than all of them,
  so every node keeps the same ones. The sequence number is kept in flash (see "Private
  messages").
- **Latency.** A hop takes the backoff and the airtime, about 0.35 s in the simulator (see
  "Medium access").
- **Capacity.** Every node carries every message once, so each message costs the channel a
  packet's room for every node that relays it. A node's rest holds it under 10% of the air; the
  channel, shared by every relay in reach, is the bound. Positions alone keep it 2% busy in
  reach, and 8% with every node moving (see "Medium access"). What messages add is
  unmeasured.
- **Pruning.** A node skips relaying what a packet it heard already carried to all its
  neighbours: that is the cancel rule (step 7, closed by the owner on 2026-10-07). In the
  simulator, every node holding a fix, a flood then costs the fewest packets possible where
  every node hears every other, along a line, and in two clusters joined by one node. In grids
  and scattered groups it costs two and a half to nine times the fewest, a third to a half of
  it in sends again: two neighbours hidden from each other relay at once, their packets
  collide at the node they had it from, and that node sends again to neighbours that already
  hold it (`docs/logs/lora/rotation-2026-10-07/`). Skipping a private message off the
  shortest path to its destination was dropped with store-and-forward: a node skipped takes the
  message from a summary later, at more cost than the relay saved. Designated relays, each
  sender naming the neighbours to pass its messages on, cut a flood's packets by a third to
  three quarters with slower worst cases, and stay on `bench/relay-pruning`
  (`docs/logs/lora/relay-pruning-2026-10-06/`).

### Private messages

A private message's body is sealed with AES-SIV under a key only its two members hold.
Other members relay it without reading it. Origin, destination and timing remain visible to members,
since the header and record are under the group key.

- **Key.** 256 bits of HKDF-SHA256 over the X25519 shared secret of the two members' keys, bound to
  both public keys. Every member learns the others' public keys from pairing and from member
  records.
- **Associated data.** The origin id, the destination and the sequence number, which bind the
  body to its record. SIV needs no nonce. The sequence number must still never repeat for an
  origin, because it names the message in every node's store. It is persisted in flash in
  reserved blocks of 64, and a boot skips to the next block, so a crash wastes numbers rather
  than reusing them. A new block starts no lower than the clock's second, so a device given a
  freed id starts above every number its last holder used. A clock past 2100 counts as 2100
  there, so that a timebase set far ahead cannot use up the numbers left. It belongs to the
  device, not the group, so leaving keeps it.

### Removing a member

Owner, 2026-10-03, except where it says otherwise.

- **Who.** Any member can remove any other, as any member can add one. The new key reaches
  each member sealed under the key it shares with the remover, so each knows who asked.
- **Confirming.** Each device's user is shown who asked to remove whom, and can decline until
  the switch, or for a day after it (owner, 2026-10-03). A device whose user declines ignores
  the removal and stays on the old key, so a member removing another out of malice can be
  overruled; its group can then remove the remover. A device whose user does not answer
  switches with the group. One that learns of the removal late switches three rounds after it
  learns of it, at the earliest.
- **Declining after the switch.** A device keeps the key it switched from for a day after its
  own switch, and declining goes back to it. It forgets what it learned since, so that it does
  not pass the removed member what the group shared without it: every other node's position,
  the messages stamped from the group's switch round, and the records of every id that changed
  since its own switch, other than its own, the removed member's among them. The removed member
  and the nodes that never switched send those again as they hold them. Only the last removal
  switched to can be declined this way, and not while another is under way. The user names the
  member, so a removal that arrives meanwhile is not declined in its place. The remover cannot
  decline its own, but can decline a rival that wins over it. A rival that wins after the
  switch keeps the key before both, and the day the first switch gave; one that removes nobody
  gives the member back and leaves nothing to decline. Any other key that removes nobody leaves
  the last removal to decline, so that no member can take the day away with an empty removal.
  A rival that goes back past later switches (see "Two at once") can be declined only before
  its switch: the key the node leaves is not the one the rival replaced, so neither is a state
  to go back to (owner, 2026-10-06). The members that switched have heard the device on the new key, so they send it no key
  message again.
- **The new key.** The remover makes a random group key and sends it to each remaining member as
  a private message, a key message, with its generation, one past the current key's, the round
  the group switches at, counted on its timebase, the id and SHA-256 fingerprint of the
  member removed, and the fingerprint of the key it replaces (owner, 2026-10-04). The remover
  signs each (see "Signatures") and sends one a packet. The switch is as far off as the remover
  would need to send its key messages and the removal message, one a packet with the rest after
  each, about 4 s each, and four rounds more: three for relays and repair, and the round it is
  in (owner, 2026-10-05). That is about 3.75 minutes for 8 members and 5.25 for 32, against 8
  and 27 when slots sent one a round. The remover sends its key messages without the rest
  since 2026-10-07 (see "Duty"); whether a spread-out group needs a longer lead waits on a
  removal measured with that (owner, 2026-10-07; see "Open"). In the simulator, every node
  holding a fix, over eight seeds (`docs/logs/lora/removal-changes-2026-10-07/`), 32 in reach
  had every key message within 17 s and a line of 12 within 82 s, and every member switched
  with the group. In grids and scattered groups of 32 it took 7 to 10 minutes at the median,
  and up to about a third of the members learned of the removal within three rounds of the
  switch or after it. Each switched three rounds after learning, as a member that learns late
  does, and was cut off from the group until then, for up to 12 minutes.
  Until then the removed device still reads everything. The remover reserves every sequence
  number its key messages need before it starts, and a remover that restarts before they have
  gone sends them again. Adding a device is refused while a removal is under way, since it would
  get the key the group is leaving. A node ignores a key message whose switch round is further
  off than a removal from a group of 32 needs, which would leave the removal pending for good,
  or whose member removed is neither a member nor a gone record it holds (owner, 2026-10-03).
  A node takes a key only on the key it names as replaced. It keeps one that names another
  unread, and tries it again once it has switched: a member that missed several switches takes
  them in order, and one still on a key a rival won over waits for the winner (owner,
  2026-10-04).
- **The switch.** Before it nodes send under the old key, and from it under the new one. Every
  node tries both keys on receive, but
  after the switch merges nothing that arrives under the old key but the key messages of its
  own generation, rivals of its key. Anything else in such a packet only shows that its sender
  missed the change. Before its own switch a node takes nothing from a packet under the new
  key either but its sender's clock, where that outranks its own (see "Keeping time without a
  fix"): it still sends under the old key, which the removed device reads. A node holding the
  key message of the generation after the one a member is on sends it again, in a packet under
  that member's key, at a time it draws within 5 s of hearing that member under it (owner,
  2026-10-07): every node that heard the member answers it, and two hidden from each other
  collided at it, after which the next try waited 13 rounds. A member
  on a key a rival won over is sent the winner's key message instead, of the same generation. The removed device can
  see that packet but cannot open the key inside. Key messages are kept for this past the
  message horizon while the old key is, but are left out of the digest after it. A member that
  missed the switch goes on sending under the old key, and nodes on the new key hear it within
  its floor. A node sends a member its key
  message this way again only after a gap of sweep rounds that doubles with each send, up to
  64 sweep rounds, about ten hours, and never stops while it keeps the old key (owner,
  2026-10-04): one that declined never takes it, and is not acknowledged, so that it would
  otherwise draw one each time it is heard. A node sent it three times at most before, so that
  anyone replaying one packet the member sent under the old key, which needs no key, spent
  every send before the member was back. A packet under an old key whose base timestamp is
  more than 5 minutes from the node's clock sends nothing at all, its key message or the
  removal notice, when both clocks are UTC.
- **The old key** is kept with no time limit, until every remaining member has said it is on
  the new one; a member a later removal takes, or that leaves, is no longer waited for. A
  member says so in an on-key record it signs over its id, the generation and the new key
  itself, which it sends in its first three packets after the switch, and again at start-up,
  and in its sweep rounds' packets for a day after either (owner, 2026-10-04). A device that
  adds the member again in a pairing stops waiting for it there, since it has just handed it
  the key (owner, 2026-10-05). A node checks one only while it waits for that member, about
  32 ms on the board. A packet's sender id
  proved nothing: any member could send one empty packet under the new key with an absent
  member's id, and every node stopped waiting for it and dropped the key its catch-up needed. A
  node keeps the four newest such keys. A member can be away for any length of time and come
  back without pairing again. While some are not heard, a node sends a header under the old key
  in each sweep round, so parts of the group that switched to different keys still hear each
  other.
- **The removed device** is sent a private message saying it was removed and by whom. Its
  screen shows that, and it does not leave the group by itself, so a stolen device that removes
  everyone else cannot take them out of their group. The remover sends it only after the
  switch, at once and under the old key, which the removed device still holds, and again when
  it hears the device under the old key, three times in all.
  Told before the switch, the device could answer by removing its remover, and the two keys
  would be rivals, and it would win with a lower id than its remover's.
- **Its record.** At the switch every node replaces the removed member's record with a gone
  record, as of the start of the switch round, so every node's is the same. Its id is free
  for the next pairing at once.
- **Two at once.** Two removals made apart at the same time make two keys of one generation.
  The one whose remover has the lower id wins wherever both are known, and of one remover's
  two, the one with the lower SHA-256 (owner, 2026-10-04; the lower hash alone before). It
  wins even after a switch to the other: a node takes a rival from a packet under the key both
  replace, and the nodes that switched to the winner send it to the members on the loser. The
  other remover makes its removal again under the winner once the member it removed is back.
  A node that switched to the losing key puts back the record of the member that key removed,
  as the node held it before that switch (owner, 2026-10-04). A rival can also win over a key
  a node has switched past since, where its part of the group removed again before the parts
  met, or switched for a second removal before the rival reached it. The node goes back past
  every switch since: it puts back the members those keys removed, removes the rival's, and
  makes its own removals again under the rival (owner, 2026-10-06). It ranks a rival only
  against a key it still keeps, so up to four switches back; a part further ahead stays apart.
  The messages the losing part sent while apart are kept and passed on under the rival's key,
  which the members the losing part removed may hold until they are removed again (owner,
  2026-10-06). A rival older than the keys kept is stale. The simulator's rival scenarios stage
  these (`crates/octowhere-sim`).
- **What is kept across a restart.** A pending removal, the old keys and which members each
  still waits for and the generation of the key message that catches each one's members up,
  and the key kept to decline the last removal after its switch; for each of the last four
  switches, who removed, the key it replaced and the member it removed, with that member's
  record for the last switch alone; and the removals this device makes again after a rival won
  over them. A member put back without its record has it again from the nodes that never
  removed it. The key messages themselves are in the message store, so a restarted node
  gets them back from its neighbours within the horizon. A node also keeps in flash the key
  messages for each member still waited for once the round after the switch has passed, and
  deletes them once it waits for that member no more (owner, 2026-10-05): in a group of two,
  the member waiting is the only neighbour, and could never hand its own key message back. A
  member at the switch says it is on the new key within that round, so only the ones that
  missed it cost a write.

## Time and freshness

Entries carry absolute UTC seconds, and messages their timebase's seconds, which are UTC as well
as its root's RTC was. Each is set once by the originator and copied verbatim by every relay. Relays never recompute them, so error does not accumulate across hops and a clockless
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

- Reject entries and messages more than an hour ahead of local time. A node on a clock started
  from a boot skips the check for entries and records, since its clock is not UTC; it judges
  messages, which carry its timebase's seconds, by its clock as any node does. The RTC's oscillator-stop flag
  (`oscillator_stopped()` in
  [`crates/octowhere-peripherals/src/rtc.rs`](../crates/octowhere-peripherals/src/rtc.rs)) is
  what makes the firmware report no RTC time, and so start such a clock.
- Drop entries past the retention horizon. Old positions are not worth relaying.

An hour of margin passes a node whose RTC has free-run for a year. The margin only has to exceed
worst-case disagreement between two honest clocks; the round period does not enter.

## Security

One group key, which every member encrypts and decrypts packets with, and a pairwise key for each
pair of members, for private message bodies.

A group key proves membership, not identity: a compromised node can forge any other node's entry.
Revocation is a group rekey.

### Signatures

Decided by the owner on 2026-10-03, after a review showed that any key holder could replace a
member's public key with its own. The review's case: such a key holder is then sent that
member's private messages, and is the one sealed that member's key message when a removal
comes. Built, and run on the two boards (`docs/logs/lora/signing-2026-10-03/`).

- **One key.** A device's identity is an Ed25519 key pair, from a 32-byte seed it stores. Its
  X25519 key for pairwise keys and pairing is derived from it, as libsodium does: the secret
  from the seed's expanded scalar, and the public key by converting the Ed25519 public key,
  which anyone can do. So a record names one public key, and nobody can pair another device's
  X25519 key with a signing key of their own (owner, choosing this over a separate signing key
  beside the X25519 one, which left that gap). Using one key for both is analysed in IACR
  eprint 2021/509. Every device's key pair changed once with this, so devices paired before it
  pair again.
- **Member records.** A member signs its own record: its id, public key, joining and change
  times, MAC and name, after the domain string `octowhere member`. A node checks the signature
  against the record's public key before the record changes anything. A device is the same
  device only by its public key, so a record replaces a held one only if the held key signed
  it, and a MAC proves nothing. A device signs its record again whenever it changes: a rename,
  or moving to another id. A record is up to 127 bytes, so a packet carries one instead of
  three, and a full table resync takes about three times as many packets.
- **Pairing.** The joining device's record is the first one made of it, so the joining device
  signs it: the welcome gives it its id and the time the adding device dates the record, it
  builds the same record the adding device built, and it returns the signature with its last
  acknowledgement. The adding device checks it before storing the member. The welcome carries
  every other record with its signature, and the gone records of earlier members whose ids it
  gave again.
- **Gone records.** One that a device sends as it leaves is signed by that device, after the
  domain string `octowhere gone`, over its id, public key and time. One that a removal makes is
  made by every node at its own switch, carries no signature, and is never sent. A node takes a
  gone record from another node only if it is signed; a pairing's welcome carries both kinds.
- **Lost keys.** A device that lost its keys, through a flash erase, pairs again as a new
  member at a new id. Its old record stays until a member removes it.
- **Key messages.** A key message carries its generation in the clear and its remover's
  signature, after the domain string `octowhere key`, over the message's origin, destination,
  sequence number, previous, timestamp, generation and sealed body. A relay checks it against
  the origin's record, and keeps for catch-up only those from the member that removed for that
  generation, as the relay learned it from its own key message. One signed key message takes a
  packet's room for two unsigned ones, so a remover sends one a packet.
- **Catch-up.** A node keeps, for each member still waited for, the key message of each
  generation after the one it is on, past the message horizon, while it keeps the old key. A
  member heard under an old key is sent the key message for the generation after it, under that
  key, so it takes one generation at a time, and is shown and can decline each removal in turn.
  The kept messages go to flash once the round after the switch has passed (see "What is kept
  across a restart").

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
  adding device sends the group key, the joining device's id and every member's record, and
  the gone records of up to eight earlier members whose ids it gave again, 226 bytes a part.
  Without those, a device paired in at such an id could take the earlier member's older record
  for a rival and move to another id. They came with version 3 of the frames (2026-10-06), and
  a device refuses frames of another version, so both devices of a pairing need it. Each part waits for its acknowledgement and is resent a second later without it; 30 s
  without progress loses contact. A full group of 32 is 17 to 19 parts, and up to 22 with eight
  gone records.
- **Commit order.** The joining device stores the group before it acknowledges the last part. The
  adding device stores the new member on that acknowledgement, then sends done, and stays 5 s to
  answer a repeated acknowledgement. A joining device that stored but never hears done says so:
  it is in the group, with the adding device's receipt unconfirmed. An adding device whose last
  part is never acknowledged has not added the member, and says the outcome is unknown. If the
  joining device did store the group, its own member record reaches the adding device through the
  mesh, at the id it was given. A device that was founding the group has no group to hear that
  under, so it listens throughout for 10 minutes under the founded group's key instead, as it
  does when its own write of the group failed after it sent done. A packet
  under that key shows the joining device stored the group, and the founding device then stores
  it, and takes it up only once the write lands. A failed write is tried again every 10 s while
  the wait lasts. The joining device is heard within about 2¾ minutes: it waits 30 s for
  done, sweeps for 135 s, then starts its own timebase and sends at once. Starting
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
nothing.

An id is freed when its member leaves or is removed (see "Removing a member"). A device that
leaves sends a gone record for itself in two packets 10 s apart, from memory, and then forgets
the key; the others replace its record with the gone record and free
its id, with no new key (owner, 2026-10-03). A pairing started meanwhile waits for the first
of the two packets, within the three rounds a device tries for, and then ends the telling
(2026-10-06; JOIN ANOTHER GROUP sends the pairing straight after the leave). A device with no
timebase has nobody to tell; the others can still remove it. A device that may
still hold the key is removed instead. A gone record wins a merge against the same device's
record when it is newer, and a node that hears a record for a device it holds as gone sends the
gone record back. A member or gone record stamped more than an hour ahead of a node's clock is
refused: it would win every merge until then. When a new member takes a gone member's id, the gone record moves to a list
of the last eight, kept in RAM, which still answers for it. Of two gone records for one id,
every node keeps the newer. Pairing a device again gives it a newer record, which wins, and a
pairing carries the gone records with the members.

The eFuse base MAC is the stable hardware identity, used to recognise a re-pair of the same physical
device rather than issuing a second id.

Group key, id, member table and the sequence-number block persist in a flash partition, not the
SD card, which is removable. They are in the settings' ekv database (`firmware/src/settings.rs`): the group
key and this device's id under one key, each member's record under its own, and this device's
Ed25519 seed and name apart from the group, each value behind a one-byte version. A device keeps
its name and key pair when it leaves a group, and CLEAR SETTINGS keeps all of it (owner,
2026-10-02). A member that changes is stored alone. The whole group goes in one transaction
when the key changes, when this device moves to another id, and when a pairing adds a member.
The
sequence-number block is stored with the identity, and a pending removal and the old keys with
the group.

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
storage key through it and encrypt the values, and ekv stores opaque bytes. Equal
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
not expected to matter, and a priority-aware mutex adds it if it does. The TCA9554 write is about
80 µs, but the longest transaction another user holds the bus for is a GNSS read of about 12 ms.
So the radio holds the bus from the channel check until its transmission has started (see "Why
contention"). Holding it across the packet is unnecessary, since the switch write before and the
restore after are each short with the bus free between them.

## Build order

1. The radio in its own task, and a cross-core I2C lock. Done.
2. Radio settings above, and slots on GPS time with the header and record format, carrying
   positions and neighbours, with a timebase taken from other nodes without a fix. Done, with ids
   from the MAC and a development key until step 3.
3. Pairing, the member table and ids. Done: the exchange, the member table, member records in
   the mesh's packets, their storage, and the screens (`SCREEN-DESIGN-BRIEF.md`, "Group and
   pairing as built").
4. The cancel rule and neighbour-only listening. Built as listening to members and neighbours
   with periodic sweeps, the cancel rule entry by entry (see "Medium access"), and a changed
   member record sent in the node's next slot, and ran on the two boards
   (`docs/logs/lora/founding-and-listening-2026-10-02/`). Contention replaced the listening and
   the slots; the cancel rule stays.
5. CAD with slot phase refined from arrival times. Dropped with the slots (2026-10-05).
6. Messages, then private messages. Built with removal and signed records, and ran on the two
   boards (`docs/logs/lora/step6-2026-10-03/`, `signing-2026-10-03/`, and a member deaf through
   two removals catching up in 19 minutes, `catch-up-2026-10-03/`).
7. Pruning relays from the gossiped graph. Closed (owner, 2026-10-07): the cancel rule is its
   group half, and store-and-forward dropped its private half (see "Messages", "Pruning").

After step 4 the work went in this order (owner, 2026-10-03), all of it now done:

- Shuffled slots and member records on request (see "Medium access" and "Packet"), together
  and first. Both change what goes on the air, which is cheapest while there are two boards.
  Both are built and ran on the two boards (`docs/logs/lora/refresh-and-recovery-2026-10-03/`).
- Step 6, ahead of step 5. A new group key goes to each member as a private message, so
  removing a member needs the message machinery: flooding, the seen-set, acknowledgements,
  sequence numbers kept in flash, and the pairwise seal. It is built with removal as its first
  use, then messages on top. Removal and messages need screens, which need a design round; the
  mesh's side goes first, driven over the USB JTAG as pairing's was. Its design was settled
  with the owner on 2026-10-03 ("Messages", "Removing a member", "Identity and storage"). The
  mesh's side is built and ran on the two boards (`docs/logs/lora/step6-2026-10-03/`).
- Step 5, then step 7. Contention replaced the slots after step 6 (owner, 2026-10-05), which
  dropped step 5, and step 7 closed on 2026-10-07.

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

- A removal's lead in groups spread out over several hops (owner to decide, 2026-10-07). With
  the switch 5¼ minutes off for 32, up to about a third of the members switch after the group,
  cut off for up to 12 minutes, and 17 along a line that loses one packet in five; four rounds
  more cut that to a tenth and 6 minutes, and eight
  rounds to almost none, while the removed device reads everything 3 or 6 minutes longer
  (`docs/logs/lora/removal-changes-2026-10-07/`). In reach nobody is late with any lead.
- A replay of two recorded packets still moves a clock, within 5 minutes where the node's RTC
  holds the time and anywhere where it does not ("Replays" above). A replayed packet held for a
  second makes the node treat its clock as sweeping for a floor and a round; it does not move the
  clock. Jamming does more harm more easily.
- A member being removed can still see that a removal is under way before the switch: key
  messages are marked as such, and none comes to it. Firmware changed to act on that can
  remove its remover first. The lower remover's id decides which removal holds (owner,
  2026-10-04, once the simulator staged rivals): it cannot be ground, as the lower hash could
  in about 2^16 tries, but the lowest ids, the founder's first, win every race. Each device's
  user is shown both and can decline the one they do not want. Bounding a key message's switch
  round and refusing a key that names no member stop the cheapest uses (owner, 2026-10-03).
- A message lost on a chain's last hop is repaired only as fast as the last node hears its
  neighbour's next packet: no relay is expected of the last node, and a lone message leaves no
  gap and no settling to show it. In the simulator, every node holding a fix and every link of
  a chain of 12 losing a packet in five, a lone message crossed it in 53 s at the median over
  32 seeds, 301 s at p90 (`docs/logs/lora/rotation-2026-10-07/`). A sender that followed up a
  round later, up to three times, took the p90 to 242 s and changed nothing elsewhere, so it
  was not built (owner, 2026-10-07).
- Contention ran on the two boards (`docs/logs/lora/contention-2026-10-05/`). Its first build
  never sent: the modem's RX on-going bit holds throughout continuous receive, and the check
  took it for a packet under way. A sender starts a few milliseconds after the time its
  header names, so a node two hops from its clock's root hears it 5 to 9 ms late. What two
  boards side by side cannot show, nodes hidden from each other and two starting inside the
  modem's detection time, rests on the simulator.
- Continuous receive's power against the budget, two days on a cell of about 1,000 mAh (owner,
  2026-10-03), which waits on measuring what the rest of the device draws
  ([`POWER-INVESTIGATION.md`](POWER-INVESTIGATION.md)).
- Measuring GNSS time sync (see "Time sync").

## Deferred

- Tuning for range, the spreading factor, once the protocol carries everything (owner,
  2026-10-03). Range has not been measured. At SF8 a 255-byte packet takes about 707 ms against
  400 at SF7, which takes 1.77 times the channel each packet holds and the rest after it.
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
