# Two-board mesh run, 2026-10-01

Firmware `e575fa5` (the mesh's first build, with per-round logs) on both boards, indoors on one
bench, neither with a fix. Node 24 is the board with MAC `44:1b:f6:86:1a:38`; node 28 is
`44:1b:f6:86:1c:1c`, whose `DIO0` does not reach GPIO44, so it polls the radio's flags 1 ms apart.
Node 24 was started about 140 s before node 28, so node 28's first sweep would span node 24's
first floor round. Each log's times are from its own boot.

- `node-24.log`: heard nobody in its first sweep and started its own clock as root 24 at 137 s.
- `node-28.log`: heard node 24's floor packet 97 s into its first sweep and took its clock, one
  hop from root 24.

| | Node 24 | Node 28 |
| --- | --- | --- |
| Packets sent | 9 | 8 |
| Packets heard from the other | 8 of 8 | 7 of 9, 2 CRC errors |
| Lateness of the other's packets on its own clock | 2.2 to 2.9 ms | refined from them: -0.9 to +0.3 ms |
| Reported RSSI | -98 to -100 dBm | -96 to -100 dBm, twice -119 |

- Node 24's lateness is the whole round trip: node 28's clock is late by the latencies of node
  24's sending and its own receiving, and its packets then arrive late again by its sending and
  node 24's receiving. Half of it, about 1.3 ms, is one hop's.
- Node 28's refinements are its clock's error between two of node 24's packets, 135 s apart:
  about 7 ppm of crystal difference at most, under the 1 ms polling's jitter.
- Both nodes swept again about 12 minutes in, and kept their slots through it.
- Without a fix neither had a position, so every packet was 30 bytes: the header and the
  neighbours record. Each listed the other.
- The reported RSSI is not the packets'. At zero SNR and above the driver read the channel's
  RSSI after the packet had ended, which is the noise; `e8b42af` fixed it. The packets themselves
  arrive at about -22 to -32 dBm (`../pa-2026-10-01/`).
- The two CRC errors were 405 s apart, at the same place in the round. Their cause was not looked
  into.
