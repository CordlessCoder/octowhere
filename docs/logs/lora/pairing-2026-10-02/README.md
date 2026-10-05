# Pairing two boards, 2026-10-02

The first pairings on the hardware, between the two bench boards side by side, indoors, with no
GNSS fix. Each ran firmware built with `pair-inject` and `DEFMT_LOG=info,octowhere::mesh=debug`,
and took its commands from `tools/pair-inject.py`. Nodes are named by the last two bytes of their
hardware addresses. `1a38` joined each time and `1c1c` added. The logs are xz-compressed.

| Run | Firmware | What happened |
| --- | --- | --- |
| 1, FIFO limit | `9ffc064` with frame logging | Discovery, keys and code went through; the 173-byte part never left `1c1c` |
| 2, paired | `5e34e0d` | A group founded, the joining device stored and confirmed, the mesh running under the group's key |
| 3, restart and rename | `5e34e0d` | `1a38` restarted in the group; its new name reached `1c1c` through a member record |
| 4, member writes, returning | `822177f` | A rename stored as one member's key; leave and rejoin as a returning device; a reported mismatch; a last pairing that leaves both in the group |

- **The FIFO limit.** The radio driver without its `half_duplex` feature splits the radio's FIFO
  between transmit and receive and refuses a packet over 128 bytes. Loading the part failed every
  second for 30 s, with nothing logged, and `1c1c` ended with the outcome unknown while `1a38`
  waited. The fix (`5e34e0d`) gives a packet the whole 256 bytes, and a failed load now warns.
  The mesh's own packets would have met the same limit at four positions beside a member
  record, or ten without one.
- **Discovery and the code.** `1c1c` heard `1a38`'s announcement within 2 s every time. The
  offer, nonce and reveal crossed on their first attempts. The frame whose X25519 and key
  derivation run took 21.9 to 22.1 ms to take, on either side. Every pairing showed the same code
  on both boards, 069463 among them, leading zero kept.
- **Transfer.** Two members fit one part. In run 2 its first sending was lost to one of the
  joining device's repeated accepts, and the resend a second later arrived; in run 4's two
  pairings the first arrived. The adding device was done 0.2 to 0.3 s after it started the
  transfer, and 1.3 s with the part sent twice.
- **The mesh after.** Both nodes sweep for 135 s after pairing, so the first packets went out
  about three minutes later. `1a38` took `1c1c`'s clock (root 0), and the two then heard each
  other's packets within 230 µs of where they expected them. Packets carrying the neighbours and a
  member record were 86 bytes; the member record alternates between ids 0 and 1. Received at
  -13 to -16 dBm at +17 dBm, the overload range in `../crc-2026-10-02/`.
- **Storage.** Each store holds the display for its length.

  | What was stored | Time |
  | --- | --- |
  | The whole group at the end of a pairing | 16 to 123 ms |
  | The whole group for one renamed member, before `822177f` | 96 and 179 ms |
  | One member's record, after it | 21 ms |
  | The device's name | 35 and 57 ms |
  | Leaving the group | 31 and 235 ms |

  The longer times include page erases; `context/BACKLOG.md` has the per-erase hold as the lever
  for settings saves.
- **Returning.** After `1a38` left, it joined again and got id 1 back, with `1c1c` reporting it
  returning. That run's users confirmed in the other order, the adding device first.
- **Mismatch.** `1a38` reported that the codes differed: it ended with the mismatch, and `1c1c`
  with the other device's report of it.

The captures' tty numbers swapped between the boards whenever a capture reset them, so each log
is named by the hardware address the firmware printed, not the port it was opened on.
