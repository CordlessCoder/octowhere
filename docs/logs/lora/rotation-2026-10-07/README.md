# The position rotation and last-hop repair, in the simulator, 2026-10-07

Whether the rest of the position table still belongs in every packet, now that the mesh
contends for the channel, and whether a message lost on a chain's last hop needs a repair of
its own without it. Measured on master after the removal changes, every node holding a fix.
Nothing here ran on a board.

Every run is `octowhere-sim` on `bench/removal-changes` (`70a0265`). `relay_cost.rs` takes
`RELAY_COST_ROTATION`, as `table::BENCH_ROTATION` reads it: `0` the firmware's rotation, `1`
none, and `4` and `6` at most two and four entries of it a packet. `RELAY_COST_FOLLOW_UP=n`
follows a packet that carried messages to a neighbour expected to pass nothing on with another
a round later, up to `n` times until that neighbour is heard holding what the sender holds;
`RELAY_COST_ANSWER=1` sends the message again on hearing that neighbour's messages digest
unlike the sender's. `channel_load.rs` counts, besides the review's measures, the position
pairs that never showed in a whole run (`never`). The link lists are left out of these files;
the review's flood files have them.

| File | Run |
| --- | --- |
| `channel-built.jsonl` | `CHANNEL_LOAD_MOTION=still,moving CHANNEL_LOAD_ROTATION=0,1,4,6`, two seeds, 20 minutes |
| `lossy32-r<rotation>-f<follow-ups>a<answer>.jsonl` | one group message on `line12-loss20`, `grid4x8-loss10` and `scatter32r25s1-loss10`, 32 seeds |
| `single/iso8-*.jsonl` | one group message on seven shapes, eight seeds, the same settings |
| `floods/flood-*.jsonl` | one group message on all seventeen shapes, four seeds, the same settings |
| `built8-rot1.jsonl`, `built8-rot6.jsonl` | the removal set of `docs/logs/lora/removal-changes-2026-10-07/` without the rotation, and capped at four |
| `built8-rot1-x2.jsonl`, `-x4` | without the rotation, the switch two and four rounds further off |

What the runs showed:

- The channel, unchanged by the removal changes: in reach, 10% busy standing still and 31%
  moving with the rotation; 2 and 8% without; 4 and 12% capped at four entries. In grids and
  scattered groups standing still, 10 to 18% of receptions are lost to overlaps with it, 2 to
  3% without it, and 2 to 4% capped.
- Every node showed every other's position within every run, with or without the rotation.
  Without it, a few distant pairs in `grid4x8` and one scattered group showed theirs later
  after the start; capped at four entries, almost none did.
- A single message along `line12-loss20`, 32 seeds: 34 s at the median with the rotation, p90
  247 s, slowest 570 s; 53 s without it, p90 301 s, slowest 353 s. In `grid4x8-loss10` it went
  the other way: 70 s with the rotation, 37 s without. In `scatter32r25s1-loss10`, 8 s and 13 s.
  The review's four seeds of `line12-loss20`, 213 to 269 s without the rotation in three of
  them, overstated the cost.
- Without the rotation, the follow-up and the answer changed little but `line12-loss20`, where
  three follow-ups took its median from 53 to 49 s and p90 from 301 to 242 s. In the slow runs
  the end node's summaries were lost on the lossy link, and each retry waited for the next
  packet between the two nodes, at the floor.
- A removal without the rotation took 265 to 516 s at the median in the spread-out groups,
  against 424 to 586 s with it, and fewer members switched after the group: 26 to 74 summed
  over eight seeds, against 33 to 86. Along `line12-loss20`, 123 s against 209 s. Capped at
  four entries, 290 to 501 s in the spread-out groups and 190 s along `line12-loss20`, where
  one run of eight never finished.
- Without the rotation and with the switch four rounds further off, at most 13 members in the
  worst group switched after the group, over eight seeds, and none was cut off for more than
  135 s.
