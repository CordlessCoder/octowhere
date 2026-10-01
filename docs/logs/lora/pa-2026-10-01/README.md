# Which PA pin the antenna is on, 2026-10-01

`bench/mesh-pa` at `c07e836` (`mesh-pa-bench`), on both boards on one bench, the same two as
`../mesh-2026-10-01/`. Every round each node sends with the next of four settings and names it in
the header's slot-phase byte: 0 is `PA_BOOST` at +17 dBm, 1 `PA_BOOST` at +2 dBm, 2 `RFO` at
+14 dBm, 3 `RFO` at 0 dBm. Packet strength is from `RegPktRssiValue`, as the driver reads it since
`e8b42af`.

| Setting | Node 24 hears 28 | Node 28 hears 24 |
| --- | --- | --- |
| `PA_BOOST` +17 dBm | -22 dBm, SNR 7 | -25 to -32 dBm, SNR 6 to 7 |
| `PA_BOOST` +2 dBm | -38 to -41 dBm, SNR 6 to 7 | -39 to -45 dBm, SNR 5 to 6 |
| `RFO` +14 dBm | -96 dBm, SNR 0 to 1 | -103 to -109 dBm, SNR -2 to -1 |
| `RFO` 0 dBm | -94 to -95 dBm, SNR 1 to 3 | -98 to -105 dBm, SNR 0 |

The antenna is on `PA_BOOST`: its strength follows the setting, 15 dB for 15 dB, and `RFO`
arrives about 70 dB lower whatever its setting, as leakage. The link test of 2026-09-22 sent on
`RFO`, at the reset value of `RegPaConfig`. The SNR tops out at about 7 dB on the bench, so it
does not tell strong signals apart.
