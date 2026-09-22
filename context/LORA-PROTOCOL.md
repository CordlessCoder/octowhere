# LoRa mesh protocol

The product is a closed location-sharing network of eight equivalent nodes. GPS supplies both
position and the time reference; LoRa carries positions between nodes. This document is the agreed
design. None of it is implemented yet. What exists on the radio today is the link test described in
[`AGENTS.md`](../AGENTS.md).

## Shape

Every node broadcasts everything it knows, not just its own position. A digest carries the whole
eight-entry table, so one received packet resyncs a node's entire view rather than one row.

That choice removes the routing layer outright. If A cannot hear C but both hear B, B's digest
carries C to A. There is no relay logic, no sequence-number seen-set, no duplicate suppression and
no hop counter, because a node that rebroadcasts its table is already relaying. Nodes are expected
to go out of range in normal use, which is what makes this worth the extra payload.

## Medium access

GPS-anchored TDMA. Slot index is the node's paired id. Slots are anchored to absolute UTC, so two
partitions that cannot hear each other compute the same schedule independently and are already in
phase when they rejoin.

Round is 45 s across eight slots, so slots are 5.6 s apart. A slot is the 174 ms packet, two
~80 µs TCA9554 writes, and a ±250 ms guard: about 674 ms, or 12% of the spacing. Guard bands are
cheap here because the duty cycle forces slots to be sparse, so plain NMEA time is sufficient and
no PPS is needed. There is none on this board: the LC76G is on I2C and the board marks the GPS UART
routes `NC`.

Budget at a 45 s round: 3.1% channel occupancy, 0.39% duty per node against the 1% EU868 limit.

A node transmits in its slot when it has something to say, and stays silent otherwise:

- its own position moved more than ~25 m since the last one it sent, which is above GPS noise
- it learned an entry materially newer than what it last relayed for that node
- a node appeared for the first time
- its floor expired, currently 150 s

Cancel a pending transmission if an arriving digest already carries everything it would have said.
Without that, all eight nodes react to the same event at once.

Triggers promote a transmission to the node's next slot rather than sending immediately, so trigger
latency is at most one round. Off-slot transmission has no audience, because the power saving
depends on every node sleeping outside slots.

### Why not contention

Contention needs continuous RX, roughly 12 mA or 288 mAh/day. TDMA at a 45 s round listens 12% of
the time, 35 mAh/day. That moves the radio from dominant draw to rounding error, and it is not a
retrofit: the radio task wakes on a schedule instead of sitting in RX.

Pairing is the exception and stays contention-based, because a node being paired has no id and
therefore no slot. That exchange is two devices with a user watching, so collisions cost a retry.

## Packet

ChaCha20-Poly1305 with a 12-byte nonce and a 16-byte tag. The nonce travels in the clear and is
fully random, not `sender_id || counter`: a sender id in it would tell a direction-finding listener
which node transmitted. At ~1e7 messages over the network's life, 96 random bits give a collision
probability around 6e-16.

Header, 6 bytes:

| Field | Bits |
| --- | --- |
| version | 4 |
| sender id | 3 |
| flags | 1 |
| presence bitmap | 8 |
| base timestamp, UTC seconds | 32 |

Entry, 68 bits, one per bit set in the bitmap, in slot order:

| Field | Bits | Note |
| --- | --- | --- |
| latitude | 25 | ~1 m |
| longitude | 26 | ~1 m at the equator |
| time delta below base | 12 | 1 s units, 68 min horizon |
| fix quality | 2 | |
| hdop | 3 | log scale |

Eight entries is 102 bytes and 174 ms on air. A partial table shrinks the packet rather than padding
it: two entries is 51 bytes and 103 ms.

The id is implicit in bitmap order, so it is not carried per entry. Reserve a slot-phase field in
the header now. CAD needs it later and adding a header field after deployment is a wire break.

Sizing fields to their true ranges is the compression. Entropy coding gains nothing on top: the
residual bits are close to uniform and 100 bytes is far too short for a dictionary method. Delta
coordinates against a reference position would save another 17 bytes but need an escape path for a
node outside the delta range; not worth it before the base design flies.

## Time and freshness

Entries carry absolute UTC seconds, set once by the originator and copied verbatim by every relay.
Relays never recompute them, so error does not accumulate across hops and a clockless node can
forward entries whose freshness it cannot evaluate. Merge keeps the larger timestamp.

A fix and a UTC stamp arrive in the same RMC sentence, so there is no state with fresh position and
no clock to stamp it. A node with no fix has nothing of its own to report anyway.

Absolute timestamps make replay self-defeating rather than something to defend against. A replayed
digest carries its original timestamps, so its entries lose the merge against anything newer, and
where nothing newer exists they are accepted and displayed as what they are: old. This is why there
is no epoch, no per-sender counter and no accept-window policy. Relative ages would need all three,
because a replayed "5 seconds old" is a lie an hour later.

What absolute timestamps do introduce is an unbounded top end. An entry stamped far in the future
wins every merge permanently, and a node with a bad clock causes that by accident, not just an
attacker. So:

- Reject entries more than an hour ahead of local time. The gate is the PCF85063A oscillator-stop
  flag, already read in [`src/peripherals/rtc.rs`](../src/peripherals/rtc.rs) and exposed as
  `oscillator_stopped()`. A node with OS set skips the check and re-evaluates on its first fix.
- Drop entries past the retention horizon. Old positions are not worth relaying.

An hour of margin passes a node whose RTC has free-run for a year. The margin only has to exceed
worst-case disagreement between two honest clocks; the round period does not enter.

## Security

One group key. Every node encrypts and decrypts with it.

A group key proves membership, not identity: a compromised node can forge any other node's entry.
Per-node authenticity would need Ed25519 signatures at 64 bytes each, which is four times the whole
payload budget for eight entries. Revocation is a group rekey.

### Pairing

Display-confirmed X25519, the numeric-comparison pattern. Founder and joiner exchange public keys,
both screens show a six-digit code derived from the transcript, the user confirms they match, and
the founder sends the group key under the ECDH secret. A MITM has to guess six digits and a failed
attempt is visible on screen. The 466 px touchscreen on every node is what makes this available;
most LoRa devices have to settle for trust-on-first-use.

Chain it, so any already-paired node can enrol a new one. Seven exchanges either way, but nobody
has to carry the founder around.

A node must ignore pairing packets unless the user has explicitly entered pairing mode on the
screen. A device always willing to pair is a permanent unauthenticated attack surface, which would
defeat the point.

The ESP32-S3 has no ECC accelerator, so X25519 runs in software. It runs once, interactively.

### Identity and storage

The protocol id is 3 bits, assigned by the founder at pairing. The eFuse base MAC is the stable
hardware identity, used to recognise a re-pair of the same physical device rather than issuing a
second id.

Group key, id and peer table persist through `esp-storage` and `sequential-storage` in a flash
partition, not the SD card, which is removable. Store the blob behind a one-byte version envelope.

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

Two changes the protocol needs, both worth making on their own merits.

The radio needs its own task. It is currently a register poll inside the 250 ms sensor loop, which
leaves the receiver deaf for most of every cycle. That is why the round-trip log shows every other
sequence number.

I2C needs a single owning task. Today "every I2C user stays on core 0" is enforced by convention and
a `NoopRawMutex` that would fail silently if someone spawned a user elsewhere. An owning task makes
it structural and gives the slot scheduler somewhere to express priority. Bus contention is not a
timing risk for slots: the TCA9554 write is about 80 µs and entirely predictable, and the lock can
be taken before the decision to transmit. Holding it across the packet is unnecessary, since the
switch write before and the restore after are each short with the bus free between them.

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

## Deferred

- CAD to cut RX power. Waking 674 ms to find an empty slot dominates the receive cost; a CAD is a
  couple of symbols. It needs slot phase good to milliseconds rather than ±250 ms, which means
  refining phase from DIO0 arrival times. Revisit once TDMA works and RX power is measurable.
- Flash encryption, as above.
- Delta coordinates against a reference position.
- Temperature-compensated RTC calibration.

## UI obligation

[`AGENTS.md`](../AGENTS.md) requires that a screen not imply data the system does not have. Entry
age has to be visible on the map, not buried in a detail view. A five-minute-old position drawn as a
plain marker is a false claim about where someone is.
