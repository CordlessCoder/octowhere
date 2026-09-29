# LoRa mesh protocol

The product is a closed group of up to 32 equivalent nodes that share positions and carry messages
between members, including private ones. GPS supplies both position and the time reference; LoRa
carries the traffic. This document is the agreed design. None of it is implemented yet. What exists
on the radio today is the link test described in [`AGENTS.md`](../AGENTS.md).

The first version of this design (commit `e57fe0a`) was eight nodes and positions only. The owner
extended it on 2026-09-29 to 32 nodes, messages and private messages, moved it to band O, and kept
fixed slots over contention.

## Radio

| Setting | Value |
| --- | --- |
| Band | O, 869.40–869.65 MHz: 500 mW e.r.p., ≤ 10% duty cycle or polite spectrum access |
| Channel | 869.4625 MHz, 125 kHz |
| Modulation | SF7, CR 4/5, explicit header, CRC on, 8-symbol preamble, sync word `0x12` |
| Output | +17 dBm on `PA_BOOST`, about 90 mA while transmitting |
| Access | duty cycle, not polite spectrum access |

Band names are those of ETSI EN 300 220-2 V3.3.1 (2025-03), Table 4. ERC Recommendation 70-03 (edition
of February 2025), Annex 1, calls the same band h1.7. In EN 300 220-2, band P is 869.7–870 MHz at 5 mW;
do not confuse the two.

- **Channel.** At 125 kHz band O holds two channels, about 869.4625 and 869.5875 MHz. The upper one
  overlaps 869.525 MHz, which is LoRaWAN's default second receive window (gateways transmit there at
  high power) and Meshtastic's EU default at 250 kHz. The sync word filters their packets out, but
  their airtime still collides with ours.
- **Power.** The SX1272 allows +17 dBm continuously and limits +20 dBm to 1% duty (datasheet section
  5.4.3). The antenna is TE 2195835-3 (Digi-Key `17-2195835-3-ND`, datasheet
  `docs/datasheets/ENG_DS_2195835_A1.pdf`), a flexible PCB antenna for 863–928 MHz on a 150 mm
  cable: 1.4 dBi peak gain, -4.5 dB average, 35% average efficiency, VSWR under 2.3:1. Digi-Key
  lists 3 dBi; TE's figure governs. Near 869 MHz its plots show VSWR about 1.35 and efficiency
  about 33%. 1.4 dBi is -0.75 dBd, so +17 dBm conducted is about 16 dBm e.r.p. at the peak, far
  under band O's 27 dBm. Band M's 14 dBm e.r.p. allows up to 14 dBm conducted.
- **PA pin.** The carrier board cannot answer it: the module exposes one `ANT` pin and chooses the
  PA inside. The module's datasheet specifies every transmit current "on PA Boost", at +7 to
  +20 dBm, and claims +20 dBm, which only `PA_BOOST` gives, so the antenna is most likely on
  `PA_BOOST`. The firmware never calls `configure_tx`, so today it transmits at `RegPaConfig`'s
  reset value `0x0F`: +14 dBm on `RFO`. That fits the link test's RSSI of about -100 dBm between
  two boards on one bench, far weaker than that range should give. A transmit on `PA_BOOST` at low
  power, compared with the reset setting, settles it.
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

A relay passes an entry on in its own next slot, 1.4 s to one round after hearing it, depending on
the ids along the path. A chain in id order crosses in one round; against id order, one round per
hop.

Messages cannot ride the digest's merge, since each one must arrive, and arrive once. They are
flooded through the same slots with their own ids and seen-set; see "Messages".

## Medium access

GPS-anchored TDMA. The slot index is the node's paired id, 0–31. Slots are anchored to absolute UTC,
so two partitions that cannot hear each other compute the same schedule and are already in phase
when they rejoin.

- **Round.** 45 s, 32 slots, 1.40625 s apart. A UTC day holds exactly 1920 rounds. Slot `k` starts
  at the round's start plus `k × 1.40625 s`.
- **Slot.** The packet, at most 400 ms (255 bytes), two ~80 µs TCA9554 writes, and a ±250 ms guard:
  at most 900 ms, leaving about 0.5 s before the next slot. Plain NMEA time is enough for that
  guard, so no PPS is needed; "Time sync" below says where the ±250 ms comes from.
- **Budget.** One packet per node per round: 0.89% duty at the largest packet, 0.41% at an
  eight-entry digest. If all 32 nodes sent an eight-entry digest every round, the channel would be
  busy 13% of the time.

A node transmits in its slot when it has something to say, and stays silent otherwise:

- its own position moved more than ~25 m since the last one it sent, which is above GPS noise
- it learned an entry materially newer than what it last relayed for that node
- a node appeared for the first time
- it holds a message, acknowledgement or member record not yet sent
- the round is one of its floor rounds (below)

Triggers promote a transmission to the node's next slot rather than sending immediately, so trigger
latency is at most one round. Off-slot transmission has no audience, because the power saving
depends on every node sleeping outside slots.

The floor is every third round, at fixed rounds: node `k` transmits in every round `r` (UTC seconds
divided by 45) with `r mod 3 == k mod 3`, whether or not it has anything new. So every node in range
is heard at least every 135 s, and every node knows in which rounds each id is sure to transmit.

A pending transmission is cancelled when an arriving packet already carries everything it would have
said and that packet's sender reports every neighbour this node has (the neighbours record, below).
Without the cancel, all nodes react to the same event at once. Without the neighbour condition, a
cancel starves a node that only this one reaches. A floor transmission is never cancelled.

### Time sync

The ±250 ms guard is an assumption carried from the first version of this design, not a
measurement. What limits it is how late the firmware learns that a UTC second began:

- **No PPS.** The LC76G is on I2C and the board marks the GPS UART routes `NC`, so the pulse that
  marks each second's start never reaches the ESP32. Only NMEA says which second it is.
- **NMEA latency.** The module writes RMC after computing the fix. How long after the second that
  is on the LC76G is unmeasured, and the adaptive low-power mode may vary it.
- **Polling.** `sensor_task` sleeps 250 ms each loop before reading GNSS, so a sentence can wait up
  to about that long. This is probably the largest term, and it is the firmware's own.
- **RTC resolution.** The PCF85063A counts whole seconds. Phase within a second between fixes has
  to come from a CPU timer anchored to a sentence's arrival.

Every term makes time late, never early, and every node runs the same hardware and firmware, so two
nodes should disagree by less than either one's worst case.

Ways to tighten it, none of which needs a PPS:

- Read the module often around the expected second instead of every 250 ms. That removes most of
  the polling term.
- Timestamp each RMC's arrival and keep the earliest over many fixes. Arrival jitter only adds
  delay, so the minimum sits closest to the true edge.
- Once a neighbour is heard, align to the group from the `DIO0` arrival times of its packets. That
  is what slots need, and what CAD depends on.

Two measurements settle how tight it gets, and with it the floor (see "CAD is required at this
size"): RMC's arrival after the second on one board, with its spread, from a fast GNSS read; and
the agreement between two boards synced this way, logged together. Both belong on a
`bench/gnss-time` branch.

### Listening

A node listens to the slot of every neighbour, every round. A neighbour is a node heard within the
last 7 rounds (315 s): long enough to span two floor transmissions, so one lost packet does not drop
it.

To find nodes that came into range, a node also listens to each other id's slot in that id's floor
rounds. A node coming into range is heard within 135 s. It costs about a third of the non-neighbour
slots each round: with 8 neighbours, 8 + 8 = 16 slots.

Listening costs the guard and the packet per slot, at 9.7 mA (125 kHz, LNA boost off, its reset
state):

| Slots listened | Receive time | Per day |
| --- | --- | --- |
| 8 | 12% | about 28 mAh |
| 16 | 24% | about 57 mAh |
| 32 | 49% | about 114 mAh |

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
nonce (12, clear) | ciphertext: header (7) + records | tag (16)
```

ChaCha20-Poly1305 under the group key. The nonce is fully random rather than `sender_id || counter`,
because a sender id in the clear would tell a direction-finding listener which node transmitted. At
~1e7 messages over the network's life, 96 random bits give a collision probability around 6e-16.

Header, 7 bytes, encrypted:

| Field | Bits |
| --- | --- |
| version | 4 |
| sender id | 5 |
| flags, reserved | 7 |
| base timestamp, UTC seconds | 32 |
| slot phase, reserved for CAD | 8 |

After the header comes a list of records, each a type byte, a length byte and a body. A node skips
any record type it does not know, so a record type can be added without a wire break. Changing the
header still is one.

| Type | Body |
| --- | --- |
| positions | packed entries, padded to a byte |
| neighbours | 32-bit set of the ids the sender heard in its recent rounds |
| member | id, X25519 public key, enrolment time, display name of up to 16 printable ASCII characters |
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

| Entries | Packet | Airtime |
| --- | --- | --- |
| 1 | 47 B | 92 ms |
| 2 | 56 B | 108 ms |
| 8 | 110 B | 185 ms |
| 16 | 183 B | 292 ms |
| 23 | 255 B | 400 ms |

A packet is filled in this order until it is full or nothing is left: the sender's own entry and its
neighbours, acknowledgements, messages oldest first, entries learned since this node last sent them
(newest first), the rest of the table in rotation, and one member record in rotation.

Sizing fields to their true ranges is the compression. Entropy coding gains nothing on top: the
residual bits are close to uniform and the packets are far too short for a dictionary method. Delta
coordinates against a reference position would save more but need an escape path for a node outside
the delta range; not worth it before the base design flies.

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
  and the neighbours record, that is about 130 bytes of message body a round, or about 10 kB an hour.
- **Pruning, later.** Gossiping every node's neighbour set gives every node the group's graph. A node
  can then skip relaying a direct message when it is not on a shortest path to the destination, and
  a group message when the sender already reaches all its neighbours. That is where routing starts
  to pay; it builds on the neighbours record rather than adding a layer.

### Private messages

A private message's body is sealed with ChaCha20-Poly1305 under a key only its two members hold.
Other members relay it without reading it. Origin, destination and timing remain visible to members,
since the header and record are under the group key.

- **Key.** HKDF-SHA256 over the X25519 shared secret of the two members' keys, bound to both public
  keys. Every member learns the others' public keys from pairing and from member records.
- **Nonce.** Built from the origin id and the sequence number. The two members share the key, so the
  origin id keeps their nonces apart, and the sequence number must never repeat for an origin. It is
  persisted in flash in reserved blocks, and a boot skips to the next block, so a crash wastes numbers
  rather than reusing them.
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
new member for the pairwise key.

### Identity and storage

The protocol id is 5 bits, 0–31. The enroller gives the joiner the lowest id free in its own table.
Two members enrolling in separate places at once can hand out the same id; member records reveal it
when the partitions meet. The member whose public key has the lower SHA-256 keeps the id, and the
other takes the lowest id free in its table and announces it with a member record. The user sees
nothing. An id is freed only by removing its member.

The eFuse base MAC is the stable hardware identity, used to recognise a re-pair of the same physical
device rather than issuing a second id.

Group key, id, member table and the sequence-number block persist through `esp-storage` and
`sequential-storage` in a flash partition, not the SD card, which is removable. Store the blob behind
a one-byte version envelope.

### Flash encryption is deferred

Not in the prototype. It costs an irreversible eFuse burn on boards that still need debugging,
Release mode disables the plaintext UART download that `cargo run --release` relies on, and it
probably breaks `sequential-storage`: XTS encrypts each 16-byte block with its offset as tweak, so
a written block cannot be partially rewritten, while `sequential-storage` does multi-pass writes
within a page.

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
   positions and neighbours.
3. Pairing, the member table and ids.
4. The cancel rule and neighbour-only listening.
5. CAD with slot phase refined from arrival times.
6. Messages, then private messages.
7. Pruning relays from the gossiped graph.

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

- Confirming the PA pin on the bench (see "Radio").
- The limit after which a rekey drops the old key.
- A shorter floor once CAD is measured (see "CAD is required at this size").
- Measuring GNSS time sync (see "Time sync").

## Deferred

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
