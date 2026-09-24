# Touch recordings, 2026-09-24

`cover-hold-lift-2026-09-24.log` is every CST9217 report read on the compass screen, from the
`touch-report-log` feature on `bench/touch-cover`. The owner covered the panel and held for about
3 s, covered briefly, covered, lifted and covered again within about a second, held for about
10 s, then tapped once, with a pause of about 3 s between each.

Each `[TOUCHLOG]` line gives the time, whether the interrupt line (`line`) or a 50 ms poll
during a cover (`poll`) triggered the read, the line's level afterwards, the raw 15 bytes, and
the parsed result. A poll while the line is high returns the previous report again.

What it shows:

- A held hand keeps reporting the cover (bit 7 of byte 4) on the interrupt line, with gaps of up
  to 180 ms between reports.
- Unreadable reports, with `0xab` or `0x00` in byte 0, arrive both during a held cover and after
  a finger lifts. No valid report with zero points appeared.
- A lift is sometimes, not always, reported as a cover with event 0 in byte 0 (`0x10`). The
  10 s hold ended without one.
- Covering again after a lift took 346 ms from the last cover report to the next.

`finger-hold-lift-2026-09-24.log` is the same logging with a finger: two drags held still for
about 3 s, a press held without moving, and a tap. The bench polled about every 35 to 50 ms
while a contact was held.

What it shows:

- A held finger keeps being reported, still or not. The longest gap between valid reports
  during a contact was 101 ms.
- A read before the controller writes its next report returns our acknowledgement, `0xab`, in
  byte 0. Eleven arrived mid-contact, 6 to 9 ms after a valid report. The firmware used to read
  these as no contact, and three in a row ended the contact: a held drag completed early.
- Every lift was followed first by a report with `0x00` in byte 0, 10 to 62 ms after the last
  point, and `0xab` reads after it.
