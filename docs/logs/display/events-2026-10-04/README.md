# The Events drawer on a board, 2026-10-04

The 2026-10-04 hand-off's Events drawer, toasts and unread arc, on board 1A:38 beside 1C:1C in
their group of two. The build is the firmware at `66dc772` with `pair-inject`, `touch-inject`
and `rtc-inject`. The screens were driven over the USB JTAG with `tools/touch-inject.py`, and
read back with its `shot`. The logs are the two boards' serial captures.

| Shot | What it shows |
| --- | --- |
| `drawer-empty.png` | An upward swipe on the clock face opened Events, with nothing in it yet |
| `drawer-responding.png` | A GNSS incident, resolved: GNSS RESPONDING, unread |
| `detail-responding.png` | Its detail, with the receiver's live state: last response 00S ago, no satellite fix yet, no fix since boot |
| `clock-unread.png` | The clock face after a refresh ended, with the unread arc under it |
| `drawer-refresh-ended.png` | Events with the refresh's result, 1 device heard and no new members, and a later GNSS incident |

What the run showed:

- The drawer opened, scrolled its list, opened a detail, dismissed a resolved event and closed
  on the board as on the host. Neither board logged an error.
- Every GNSS incident in the run came from the debugger, not the module. A `shot` halts the core
  for about 11 s while it reads the framebuffer. The GNSS task then finds 10 s without NMEA,
  counts the module stuck and resets it (`1a38.log`, 30 s, 92 s and 318 s, each just after a
  shot). The module answered again at once, so each incident went RECOVERING, then RESPONDING.
  The PMIC's key read failed at the same moments. Earlier runs on these boards, without
  shots, reset the module never.
- A refresh started through `pair-inject` at 166 s ended at 301 s with the other board heard.
  Its toast had timed out before the next shot, but the unread arc showed on the clock face,
  and the result was in Events.
- The shots show pixels past the glass's edge that the panel never shows: corners the screens
  never clear, and a stray stroke 242 px from the centre.
