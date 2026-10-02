# Pairing through the screens, 2026-10-02

The step 3 screens on the two bench boards, side by side, indoors, with no GNSS fix and no RTC
time. Both ran firmware built with `touch-inject` and `pair-inject` at the default log level.
Every touch, cover and power key press came from `tools/touch-inject.py`, and `screens.png` is
the boards' own framebuffers read back with its `shot`, the corners outside the glass masked.
The logs are named by the hardware address each board printed, and are xz-compressed.

At the start `1c1c` (id 0) and `1a38` (id 1) were already a group of two.

- **Leave first, then join.** On `1a38`, GROUP, PAIR and JOIN opened the leave warning, and the
  LEAVE + JOIN drag sent the leave. Storing it took 38 ms, and the screens sent the join 4 ms
  after it was stored.
- **Adding.** On `1c1c`, PAIR, ADD and START SEARCH found `1a38` in about a second. Its row shows
  the hardware address alone: an announcement carries no name. Tapping it gave the same code on
  both boards, 386 372, each screen naming the other's address.
- **Confirming.** A drag on each confirmed the code. `1a38` stored the group, then confirmed;
  `1c1c` stored the member on that confirmation and showed RESTORED with id 1, since `1a38` had
  been a member.
- **Reading a screen back interrupts the board.** A `shot` takes about 11 s over the USB JTAG.
  While one read `1a38`, `1c1c` heard no announcement from it for over 10 s and dropped it from
  its list until the next one.
- **Renaming.** `1a38`'s keyboard took 13 deletes and `Ridge` in lower case. SAVE stored the
  name in 26 ms and the member record in 89 ms, and the panel's NAME showed `Ridge`.
- **Members.** On `1c1c`, `1a38` was heard directly 2 min 53 s before, which shows blue as a
  neighbour. With no fix, both positions show NEVER RECEIVED, and with no UTC the join time shows
  UNKNOWN. The rename had not yet reached `1c1c`: member records travel one per packet, in turn.
- **Cover.** A cover while `1c1c` searched cancelled the pairing and returned to the clock face.
- **Resting.** A finished pairing keeps the normal screen timeout, so both boards went to rest
  between steps until their timeout was set to NEVER through the TIMEOUT screen.
