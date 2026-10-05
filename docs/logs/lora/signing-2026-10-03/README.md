# Signed records, 2026-10-03

Runs of the mesh's signed records (`context/LORA-PROTOCOL.md`, "Signatures") on the two bench
boards, indoors, with no GNSS fix, on firmware built with `pair-inject`, `touch-inject` and
`rtc-inject`. Commands came from `tools/pair-inject.py`. The logs are named by the hardware address
each board printed, and are xz-compressed. Times are each board's own, from its boot.

## A stack overflow at pairing

The first run, on the build of `8250656`, has no log: the second run below wrote over it. `1a38`
panicked 9.44 s after boot, as it took the joining device's first key exchange: `async fn` resumed
after completion, in the touch task's future, a static that core 0's stack had grown into.
Signatures doubled a group to about 5 KB, and every frame that held or moved one grew with it:
the radio task's poll took 27,056 bytes and `Mesh::pair` 21,504. `d0bc123` keeps a group's records
and a pairing's welcome on the heap, and those frames came to 11,792 and 6,448.

## Signed records

`signed-1a38.log` and `signed-1c1c.log`, on the build of `d0bc123`.

- **A new identity.** Each board read the X25519 secret it had stored as its Ed25519 seed, which
  gave it a new identity. Their member records, in the old layout, did not read back, so both
  came up in no group.
- **Pairing.** `1a38` founded a group and `1c1c` joined. The joining device built its record from
  the welcome and signed it, in the frame that took 35.4 ms; the adding device checked the
  signature, in one that took 32.5 ms, and stored the member. `1c1c` never heard the done, so its
  join reads as unconfirmed.
- **Renames.** `1c1c` renamed itself, and `1a38` took the signed record at 307.3 s. `1a38`'s
  rename reached `1c1c` only at 747.7 s: `1c1c` failed the CRC of three packets in that time, six
  in the run. The boards sat close together, heard at -9 to -29 dBm, and a packet carrying a
  signed record is about 160 bytes.
- **A phantom**, enrolled on `1a38` with a key of its own, reached `1c1c` at 823.6 s: its record
  carried its own signature.
- **Leaving.** `1c1c` left and sent its signed gone record, which `1a38` took at 837.4 s. Paired
  again, it took id 1 as a new member (`returning: false`): its old record was a gone record,
  not a member's.

## A start-up panic with a stored signed group

`start-up-panic-1a38.log` and `start-up-panic-1c1c.log`, on the build of `6750e8d`, meant for two
missed switches and their catch-up, which never started. Both boards booted with the group the
run above stored and panicked 5.44 s after boot: the global allocator could not serve fontdue's
glyph raster (`Lines::new`), on the identity's first frame.

The start-up's identity holds its title's coverage in two 45,828-byte buffers, and the raster
grew to 34,968 bytes alongside them. esp-alloc serves the internal heap's two regions, 72 KiB then
120 KiB, first fit, and grows a block by allocating the new one before freeing the old. With a
signed group stored, about 5 KB more was in use at boot (27,812 bytes, against 22,676 with an
unsigned one): the title's first buffer no longer fitted the first region, and neither region had
a block left for the raster. The run above booted in no group, so it never met this.
`dfcc637` makes the raster at its largest first thing at boot, so it never grows.
`bench/ui-allocations` counts the stage's heap requests on the host.
