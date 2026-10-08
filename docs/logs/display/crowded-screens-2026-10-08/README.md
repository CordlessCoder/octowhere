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
adds master's `cbd6451`: the member face is no longer built under the open drawer. The fourth
and fifth runs added to `369d846` master's `3e21df5`, which stops the face under the open drawer
counting as showing, so that the motion task samples slowly there and turning no longer keeps
the screen lit. The sixth run added to that build master's `f2c2cd5`, which steps the breathing
scatter only when its breath changes, and the seventh master's `e81ec08`, which keeps the member
face from one step to the next while what it is built from stays the same. The eighth added
`39e72d1`, which finds the scatter's damage between two breaths from the levels at which each of
its marks changes, instead of comparing every mark at both. The ninth added `aa09532`, which
lays the clock face out again only when more than its breath changes, and the tenth `e38e9c3`,
which keeps the boxes a list's scatter keeps clear of once worked out, and shares them with the
list built after it while the two hold the same items. The eleventh added `929400a`, which finds
the scatter's damage where its clear boxes move from the points they cover, and `a7d8081`, which
keeps every item's bounds in a breathing list. A step is the stage's step, from the frame loop's
wake; a draw is the drawing into the framebuffer, for the steps that changed pixels; a flush is
the transfer that followed. Each summary gives a phase's steps twice: all of them, and then
apart for the steps that changed the panel and those that changed nothing, with what the phase's
steps took together for each second it lasted. Times are in milliseconds, the heap in bytes.

| File | What it is |
| --- | --- |
| `over-members.txt`, `over-members-phases.txt` | The drawer opened over the member face, where it is opened from on the face |
| `over-clock.txt`, `over-clock-phases.txt` | The same run with the drawer opened over the clock face |
| `over-members-fixed.txt`, `over-members-fixed-phases.txt` | The first run again on `369d846` |
| `over-members-covered.txt`, `over-members-covered-phases.txt` | The first run again with the face under the open drawer no longer counting as showing |
| `over-clock-covered.txt`, `over-clock-covered-phases.txt` | The same build with the drawer opened over the clock face |
| `over-members-breath.txt`, `over-members-breath-phases.txt` | The fourth run again with the breathing scatter stepped only when its breath changes |
| `over-members-kept.txt`, `over-members-kept-phases.txt` | The sixth run again with the member face kept while nothing it shows moved |
| `over-members-levels.txt`, `over-members-levels-phases.txt` | The seventh run again with the scatter's damage found from where its marks change |
| `over-members-parts.txt`, `over-members-parts-phases.txt` | The eighth run again with the clock face laid out again only when more than its breath changes |
| `over-members-shared.txt`, `over-members-shared-phases.txt` | The ninth run again with a list's clear boxes kept and shared |
| `over-members-scrolled.txt`, `over-members-scrolled-phases.txt` | The tenth run again with the scatter's damage found where its clear boxes move, and every item's bounds kept |

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
- In the drawer, a step either changed the panel or changed nothing, and the two cost very
  different amounts. Scrolling the events over the clock face, the steps that changed the panel
  took 22.9 ms at the median and the others 8.9 ms. A median over all of a phase's steps moves
  with how many of each a run made, so the points below compare the two kinds apart.
- Before `cbd6451`, a step in the drawer cost about 12 to 13 ms more over the member face than
  over the clock face, whether it changed the panel or not. Scrolling the events, the steps
  that changed it took 36.3 ms against 22.9, and the others 20.5 against 8.9. The runs differ
  only in the face underneath, and the stage built the member face's list every step even under
  the drawer. The heap's peak was 138,744 bytes over the member face, against 116,120 over the
  clock face.
- With the face no longer built under the open drawer, the steps over the member face that
  changed the panel took 24.5 ms scrolling the events, against 36.3, and the others 8.5 ms,
  against 20.5. Scrolling the conversations, they took 27.2 and 9.3 ms, against 38.4 and 22.9.
  The heap's peak while scrolling fell to 116,120 bytes. The face is still built while the
  drawer moves, since it shows beneath it then.
- That run's medians over all steps, 9.9 ms scrolling the events and 12.4 the conversations,
  were lower than the clock face's 17.8 and 21.0 because it made more of the cheap steps. The
  face under the open drawer still counted as showing, so the motion task sampled every 20 ms,
  and each sample woke the frame loop for a step that changed nothing. Scrolling the events, the
  frame loop made 36.8 frames a second over the member face, 25.9 of them changing nothing,
  against 28.7 and 17.4 over the clock face. Split by kind, the two faces were within 1.6 ms.
- With the face under the open drawer no longer counting as showing, the same build ran over
  both faces. Scrolling the events, the frame loop made 29.0 frames a second over the member
  face and 28.9 over the clock face. The steps that changed the panel took 22.3 and 23.0 ms, and
  the others 8.4 and 8.7. Scrolling the conversations, they took 25.7 and 26.1 ms, and 9.7 and
  10.8. Over all steps, the medians were 17.3 and 17.6 ms for the events, and 20.5 and 21.0 for
  the conversations. The clock face under the drawer adds at most about a millisecond a step.
  With the drawer still over the member face, stepping took 396 ms of each second, against 506
  before.
- Most steps in the drawer change nothing. In the phase that opened the drawer and left it
  still, 26 of about 34 frames a second changed nothing, each taking about 8 ms. The drawer's
  backdrop breathes, and while anything breathes the stage counts as animating, so the frame
  loop steps at its frame pace. Each of those steps builds the drawer's list again.
- With the breathing scatter stepped only when its breath changes, the frame loop made 20.5
  frames a second at rest on the clock face, against 49.3, and 21.4 with the drawer still,
  against 33.7. Stepping took 137 ms of each second on the clock face, against 182, and 311 with
  the drawer still, against 396. Scrolling the events, it took 392 ms against 460, and reading a
  conversation 251 against 392. What is left is the breath's own steps. At rest on the clock
  face, a step that changed nothing took 8.2 ms at the median, and in the still drawer 10.5 ms.
- On the member face the frame loop stepped as often as before, about 40 times a second at
  10.5 ms a step with 31 members: 468 ms of each second at rest. The face samples motion every
  20 ms, and the frame loop steps for each sample.
- With the member face kept while its group, fix, heading and selection stay the same and none
  of its ages moves on, a step that changed nothing took 1.2 ms at the median on the face,
  against 10.5. The frame loop stepped as often, but stepping took 122 ms of each second at rest,
  against 468, 140 with a member selected, against 455, and 141 after it was let go, against
  479. Turning, the face is built each degree as before. Keeping what the face was built from
  raised the heap's peak there by 3,160 bytes, to 119,280.
- With the scatter's damage found from the levels at which its marks change, a step at rest on
  the clock face that changed nothing took 3.3 ms at the median, against 8.3, and in the still
  drawer 7.6 ms, against 9.8. Stepping took 70 ms of each second at rest on the clock face,
  against 136, and 168 with the drawer still, against 287. Reading a conversation it took 143
  against 294, and scrolling the events 284 against 391. Scrolling, a step that changed the panel
  still took 28.8 ms at p95, against 29.2. While the list moves, the texts the scatter keeps clear
  of move with it, and the damage then compares every mark at both breaths. Each scatter's levels
  raised the heap's peak by 1,024 bytes, to 94,520 on the clock face and 142,928 in the drawer.
- With the clock face laid out again only when more than its breath changed, a step at rest on
  the clock face that changed nothing took 2.1 ms at the median, against 3.3, and stepping took
  51 ms of each second, against 70. The drawer's phases stayed within 14 ms a second of the
  eighth run. The log lost the lines of 19 frames, in two bursts while the lists scrolled.
- On the host, 83% of a step in the still drawer went into laying out every text of the list
  drawn and of the list built, to find the boxes the scatter keeps clear of. With those boxes
  kept, and shared by a list built again with the same items, a step in the still drawer that
  changed nothing took 4.1 ms at the median, against 7.6. Stepping took 101 ms of each second
  with the drawer still, against 173, 210 scrolling the events, against 280, 214 scrolling the
  conversations, against 291, and 106 reading a conversation, against 141. The heap's peak rose
  by 544 bytes in the still drawer, to 143,472, and by at most 32 bytes on the faces. Closing the
  drawer, it was 145,584 bytes, against 133,664 in the two runs before and 141,904 in the
  seventh.
- While a list scrolls, the texts its scatter keeps clear of move with it, and the damage
  compared every mark at both breaths. With the damage found from the points the moved boxes
  cover, and every item's bounds kept by a breathing list, a scrolling step that changed the
  panel took 12.4 ms at p95 scrolling the events, against 26.3, and 12.2 scrolling the
  conversations, against 28.4. Stepping took 162 ms of each second scrolling the events, against
  210, and 161 scrolling the conversations, against 214, and the frame loop made 24.2 frames a
  second scrolling the events, against 22.4. On the host the two changes took a scrolling step
  from 115 µs to 27. Keeping the points that can show and the items' bounds raised the heap's
  peak by 1,040 bytes on the clock face, and by up to 4,656 while a list scrolled, to 124,568.
- The drawer's draws stayed small in every run, 6 to 9 ms at the median and under 37 ms at p95
  while a list scrolled.
- No panic in any run. Besides the GNSS module's warnings, the PMIC's key and battery reads
  failed a few times in each run, each within a quarter of a second of a GNSS reset, which goes
  through the I/O expander on the same bus.
