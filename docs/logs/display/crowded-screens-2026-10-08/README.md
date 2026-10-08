# The crowded member face and a many-row drawer on a board, 2026-10-08

What the member face costs board 1A:38 with 31 other members placed round it, crowded into
sectors, and what the Events and Messages drawer costs with more than 16 rows: the board timing
the 2026-10-05 design hand-off asked for (`context/design/DECISIONS.md` 34). Two boards can place
one member at most, so `bench/crowded-screens` (`crowded-screens-bench`) stands in the hand-off's
crowded fixture, Kestrel and thirty more in four runs, for the group the mesh publishes, and a
private message from each of them, unread. The heading turns at 30° a second when the bench says,
in place of the compass's. Rerun:

```text
git worktree add /tmp/bench-crowded bench/crowded-screens
cd /tmp/bench-crowded/firmware
cargo build --release --offline --features crowded-screens-bench
# flash, capture the log, and set the RTC with tools/rtc-inject.py if it has no time; then:
uv run tools/crowded-screens-bench.py <elf> <probe> <log> [--drawer-over clock]
uv run tools/crowded-screens-summary.py <log>
```

The script drives the board over the USB JTAG through the phases below and gives it a fix. The
board ran `c17cbac`, master at `60ffb36` with the bench, and for the third run `369d846`, which
adds master's `cbd6451`: the member face is no longer built under the open drawer. A step is the stage's step, from the frame
loop's wake; a draw is the drawing into the framebuffer, for the steps that changed pixels; a
flush is the transfer that followed. Times are in milliseconds, the heap in bytes.

| File | What it is |
| --- | --- |
| `over-members.txt`, `over-members-phases.txt` | The drawer opened over the member face, where it is opened from on the face |
| `over-clock.txt`, `over-clock-phases.txt` | The same run with the drawer opened over the clock face |
| `over-members-fixed.txt`, `over-members-fixed-phases.txt` | The first run again on `369d846` |

The `-phases.txt` files hold what the board reported at the start of each phase: the screen,
the member selected, the drawer's root, and how many events, unread entries and conversations it
held. Every run held 31 conversations and 31 to 35 events. The events beyond the 31
conversations were the GNSS module's: it stopped answering about a minute into each run, and
its task reset it every minute or two.

What the runs showed:

- Turning, the crowded face redraws whole each degree: 111 ms at the median and 124 to 126 ms
  at p95 in every run, against 82 to 99 ms on 2026-10-04 with a single other member. Each
  step took 24 ms at the median, and the flush 14.5 ms, so the face turns at about seven
  frames a second. A member selected inside a sector costs the same.
- With the drawer open, a step costs 10 to 13 ms more at the median over the member face than
  over the clock face: scrolling the events, 29.3 ms against 17.8 ms, and the conversations,
  33.8 against 21.0. The runs differ only in the face underneath, and the stage builds the
  member face's list every step even under the drawer. The heap's peak was 138,744 bytes over
  the member face, against 116,120 over the clock face.
- With the face no longer built under the open drawer, the same run over the member face
  stepped in 9.9 ms at the median scrolling the events, against 29.3 ms, and 12.4 ms scrolling
  the conversations, against 33.8. The heap's peak while scrolling fell to 116,120 bytes. The
  face is still built while the drawer moves, since it shows beneath it then. On 2026-10-04,
  with a few events, scrolling them stepped in 6.3 ms.
- Over the clock face, scrolling stepped slower than over the member face as fixed: 17.8 ms for
  the events and 21.0 ms for the conversations. At rest in the drawer the two were within 2 ms
  of each other. Something more is done under the drawer while it scrolls over the clock face;
  it was not looked into.
- The drawer's draws stayed small in every run, 6 to 9 ms at the median and under 37 ms at p95
  while a list scrolled.
- No panic in any run. Besides the GNSS module's warnings, the PMIC's key and battery reads
  failed a few times in each run, each within a quarter of a second of a GNSS reset, which goes
  through the I/O expander on the same bus.
