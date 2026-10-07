# A removal with the 2026-10-07 changes, in the simulator

The removal changes the owner chose on 2026-10-07, as built on master (`ae5cad7` to `1ee970c`):
no acknowledgements of key messages, a message never sent ahead of one sent again, a catch-up
drawn within 5 s, no rest after the remover's own key messages, and a message sent again once.
Every node holds a fix that stays put. Nothing here ran on a board.

Every run is `octowhere-sim` on `bench/removal-changes` (`3f99dd5`), master with the step 7 and
review benches carried over and their switches defaulting to the firmware. The test is
`crates/octowhere-sim/tests/relay_cost.rs`, whose header has the command, and
`tools/relay-removal.py` and `tools/removal-cutoff.py` there print the tables. The shapes are
step 7's (`docs/logs/lora/relay-pruning-2026-10-06/`), the same ten as the review's removals
(`docs/logs/lora/contention-review-2026-10-07/`, `final8-*.jsonl`).

| File | Run |
| --- | --- |
| `built8.jsonl` | `RELAY_COST_SEEDS=1..8 RELAY_COST_FIX=still RELAY_COST_KIND=removal RELAY_COST_VARIANTS=32:1,32:3`: the firmware, and with three sends again |
| `built8-x2.jsonl`, `-x4`, `-x8` | the firmware with `RELAY_COST_EXTRA_ROUNDS` 2, 4 and 8: a switch that many rounds further off |
| `flood-built.jsonl` | `RELAY_COST_FIX=still RELAY_COST_VARIANTS=32:1`: one group message on every shape, four seeds |

As built, a removal matches the review's five changes (`final8-burst.jsonl`) in eight groups
of ten and differs only slightly in the other two. A member that learns of a removal within
three rounds of the switch, or after it, switches three rounds after it learns; until then the
group is on a key it lacks, and the member is cut off from it. Seconds until every other member
held its key message, median and slowest of eight seeds; members that switched after the group,
summed over the seeds; and the longest a member was cut off:

| Group | Before the changes | As built |
| --- | --- | --- |
| All 32 in reach | 122 / 150 s, none late | 16 / 17 s, none late |
| Line of 12 | 44 / 58 s, none late | 48 / 82 s, none late |
| Line of 12, one packet in five lost | 169 / 960 s, 21 late, up to 855 s | 209 / 1,125 s, 12 late, up to 1,035 s |
| Grid, 4 × 8 | 1,022 s / never, 145 late, up to 1,485 s | 445 / 524 s, 68 late, up to 360 s |
| Scattered, layouts 1 to 4 | 936 to 1,262 s at the median, 3 runs never; 129 to 158 late, up to 1,620 s | 424 to 586 s at the median; 33 to 86 late, up to 720 s |

Before the changes, members who never learned of the removal within the run are not counted
late, so those counts are low. The switch is 5¼ minutes off for a group of 32. Adding rounds to
it (`built8-x*.jsonl`, `tools/removal-cutoff.py`):

| Rounds added | Switch for 32 | Late, worst group of ten | Longest cut off |
| --- | --- | --- | --- |
| 0 | 315 s | 86 over 8 seeds | 1,035 s along the lossy line; 720 s in a scattered group |
| 2 | 405 s | 54 | 495 s |
| 4 | 495 s | 25 | 360 s |
| 8 | 675 s | 2, along the lossy line | 90 s |

Where every member is in reach, along a lossless line and in two clusters, no member was late
with any lead. A longer lead also brought the key messages sooner in the spread-out groups,
since fewer went by catch-up: in `grid4x8`, 445 s to 360 s at the median with two rounds added.

A single message, against the review's `flood-fix-today.jsonl`: within 2 s at the median in
14 of 17 groups, and slower in three: `grid2x8` 97 to 110 s, `grid4x8-loss10` 35 to 73 s and
`scatter32r25s2` 20 to 35 s. The group sent fewer packets until every node held it in 14
groups, as many in two, and more in `grid4x8-loss10`, 110 to 138 at the median.
