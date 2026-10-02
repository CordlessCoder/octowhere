# Why some of the mesh's packets failed their CRC, 2026-10-01 to 02

Four one-hour runs of `bench/mesh-crc` on the two bench boards, side by side under a metre apart,
indoors. Each node sends in every other slot (node 24 the even ones, node 28 the odd), about 1,350
packets an hour each way, and logs every packet it sends and receives with its bytes. The logs
are xz-compressed. `tools/mesh-crc-compare.py` and `tools/mesh-burst-compare.py` on the branch
read them, two at a time, after `xz -dk`.

| Run | Bench | Sync word | Power | 24 hears 28 | 28 hears 24 |
| --- | --- | --- | --- | --- | --- |
| 1, bytes | `a74bcff` | `0x12` | +17 dBm | 7 CRC errors of 1,348 | 5 of 1,278 |
| 2, channel monitor | `964ebfb` | `0x6C` | +17 dBm | 11 of 1,384 | 0 of 1,382 |
| 3, sender's activity | `151432b` | `0x6C` | +17 dBm | 18 of 1,339 | 0 of 1,336 |
| 4, low power | `5eb74b0` | `0x6C` | +2 dBm | 0 of 1,339 | 0 of 1,336 |

- At +17 dBm the packets arrive at about -21 dBm. A failing packet arrives as strong as the rest
  but with its SNR down from about 6.8 to 3 dB, 1 to 20 bits flipped across several 5-symbol
  blocks, mostly late in the packet, and its frequency error a fixed step, about 650 units, from
  where the other packets from that node sit. Every packet off that cluster fails about half the
  time; none in it failed.
- Nothing else was on the channel: in runs 2 to 4 the monitor found no burst 10 dB over the floor
  (-96 and -99 dBm) that was not one of the two nodes' packets.
- Neither node's display flush, frame draw, GNSS read, sensor read or motion sample, on the
  receiving side (runs 1 to 4) or the sending side (runs 3 and 4), lines up with the failures.
- At +2 dBm the packets arrive at about -45 dBm, and no packet failed or left its cluster. The
  receivers were overloaded at under a metre; the AGC is on, at its reset value. That does not
  arise between nodes at any useful distance, but it does between two worn close together.
- Every packet that passed its CRC opened under the key, in all four runs.
- Foreign packets: 1 to 5 an hour, with either sync word. Most failed their CRC at an SNR near
  -11 dB, which looks like noise passing the header check in continuous receive rather than
  another network. Run 4 also caught a 32-byte one that passed its CRC.
