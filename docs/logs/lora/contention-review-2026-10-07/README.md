# The protocol on contention, in the simulator, 2026-10-07

What the mesh costs on contention with every node holding a fix, as it does outdoors: the
channel's use, a flood, and a removal, today and with the changes the review proposed. The
owner decided on them on 2026-10-07. Nothing here ran on a board.

Every run is `octowhere-sim` at `384f3fd` on `bench/contention-review`, which adds switches to
the firmware's own code whose defaults are the firmware's. The tests are
`crates/octowhere-sim/tests/channel_load.rs` and `relay_cost.rs` there, both ignored, and their
headers have the commands. `tools/channel-load.py` and `tools/relay-*.py` print the tables.

| File | Run | What it holds |
| --- | --- | --- |
| `channel-load.jsonl` | `CHANNEL_LOAD_ROTATION=0,1,2` | 20 minutes of each shape without a fix, with one that stays put, and moving: packets, airtime, busy share, receptions lost to overlaps, and how old the positions a node holds get |
| `channel-load-cap.jsonl` | `CHANNEL_LOAD_MOTION=still,moving CHANNEL_LOAD_ROTATION=0,1,4,6` | the same, with the rest of the table capped at two and four entries a packet |
| `flood-nofix-today.jsonl` | `RELAY_COST_VARIANTS=32:3` | one group message, no fixes, the firmware's settings |
| `flood-fix-today.jsonl` | `RELAY_COST_FIX=still RELAY_COST_VARIANTS=32:3` | the same with fixes |
| `flood-fix-again1.jsonl` | `RELAY_COST_FIX=still RELAY_COST_FIRST=1 RELAY_COST_VARIANTS=32:1` | sent again once instead of three times; with one message, first sends first changes nothing |
| `flood-fix-rest.jsonl` | `RELAY_COST_FIX=still RELAY_COST_FIRST=1 RELAY_COST_VARIANTS=32:3:r0,32:1:r0,32:3:r2,32:1:r2` | no rest, or two airtimes, after every packet |
| `flood-nofix-bundle.jsonl`, `flood-fix-bundle.jsonl` | `RELAY_COST_FIRST=1 RELAY_COST_VARIANTS=32:1:r0,32:3:r0`, the second with `RELAY_COST_FIX=still RELAY_COST_ROTATION=1` | no rest, and with fixes no rotation either |
| `iso-*.jsonl` | `RELAY_COST_FIX=still` on `line12-loss20`, `grid4x8` and three scattered groups | one change each: `rot1` no rotation, `cap4` and `cap6` two and four entries of it, `first` first sends first, `r0` no rest, `again1` sent again once |
| `final8-today.jsonl` | `RELAY_COST_SEEDS=1..8 RELAY_COST_FIX=still RELAY_COST_KIND=removal RELAY_COST_VARIANTS=32:3` | a removal in ten shapes, eight seeds, the firmware's settings |
| `final8-burst.jsonl` | the same with `RELAY_COST_FIRST=1 RELAY_COST_CATCH_UP=5000:13 RELAY_COST_BURST=1 RELAY_COST_VARIANTS=32:3:k,32:1:k` | the four changes, and the five |
| `final8-r9.jsonl` | the same without `RELAY_COST_BURST=1`, `RELAY_COST_VARIANTS=32:1:k` | the five changes but the remover's rest kept |
| `final8-bundle.jsonl` | `RELAY_COST_FIRST=1 RELAY_COST_CATCH_UP=5000:13 RELAY_COST_VARIANTS=32:3:k,32:3:r0:k,32:1:r0:k` | no rest after any packet |

The shapes are step 7's (`docs/logs/lora/relay-pruning-2026-10-06/`). `still` gives every
node a fix on a grid 100 m apart that never moves; `moving` moves each about 44 m every 30 s,
each node at its own time. Two packets that overlap at a receiver are both lost unless one is
6 dB stronger. The late-member counts need each run's node logs, which are not kept here.

What the runs showed:

- A packet carries the rest of the position table in rotation, so with fixes every packet is
  full, 248 bytes. In reach that keeps the channel 10% busy standing still and 31% moving,
  against 2 and 8% with the rotation left out, and the positions each node holds are no older.
  In grids and scattered groups 10 to 18% of receptions are lost to overlaps, against 2 to 3%.
  But there, with nodes standing still, some nodes never got some others' positions in 20
  minutes without the rotation, in `grid4x8` and one scattered group; with it, every node got
  every position.
- The rotation also keeps every node sending every round, which repairs a message lost on a
  chain's last hop. Without it, a message along `line12-loss20` took 213 to 269 s in three
  seeds of four, against 15 to 35 s; capped at two and four entries, 97 and 132 s at the median.
- Positions slow a single message: in `grid2x8` from 2.7 to 97 s at the median.
- A removal in the spread-out shapes took 16 to 21 minutes at the median, and 3 of 80 runs left
  a member without its key after 30 minutes. With the five changes, no key acknowledgements,
  first sends before sends again, catch-ups spread over 5 s, no rest after the remover's own key
  messages and one send again, 7 to 9 minutes, and every run finished. Along `line12-loss20` it
  got slower, 169 to 209 s at the median; without the one send again, 134 s.
- Dropping the rest after every packet sped removals up but slowed a single message in the
  grids up to seven times at the median, and left 6 of 272 floods unfinished after 10 minutes.
