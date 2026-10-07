# Relay pruning in the simulator, 2026-10-06

What a flood and a removal cost on contention, against what a flood could cost on the same
links, and what four relay changes did to both. Step 7 of the protocol's build order asked for
this; the owner closed it on 2026-10-07 on these results. Nothing here ran on a board.

No node held a GPS fix in these runs, so no packet carried positions, which outdoors every
packet does. The 2026-10-07 review of the protocol on contention measured with fixes: a
removal in the spread-out groups took 16 to 21 minutes at the median, and a single message in
`grid2x8` 97 s against 2.7.

Every run is `octowhere-sim` at `8bded71` on `bench/relay-pruning`, each node the firmware's
own node on virtual time, four seeds a shape. The test is `crates/octowhere-sim/tests/relay_cost.rs`
there, and its header has the command; the table scripts are that branch's `tools/relay-*.py`.

| File | What it holds |
| --- | --- |
| `floods.jsonl` | One group message from node 0, every variant, shape and seed: packets that carried it, sends again, seconds until every node held it, and the links |
| `removal-32-*.jsonl` | Node 0 removes the last node, one file a variant: packets that carried messages, and seconds until every other member held its key message |
| `tables.txt` | The three scripts' tables from those, and the exact best a flood could do |

A variant is `steps:resends[:n|d|dn]`. `32:3` is the firmware's: a packet holding records
backs off up to 32 steps, and a message goes again three times for a neighbour not heard
passing it on. `32:1` sends again once. `n` has a send again name the neighbours still waited
for, and a named one that holds the message answers. `d` has each sender name the fewest
neighbours whose reported neighbours reach every node two hops away, and only those relay.
`dn` is both. `n` and `d` passed what they name outside the radio, at no cost on the air, so
they are upper bounds. The late-member counts need each run's node logs, 387 MB, which are not
kept here; `tables.txt` has them.

The shapes: `full32`, all in reach; `line12`, each node in reach of the next; `clusters10`,
two clusters of ten joined by one node; `grid2x8`, `grid4x8` and `grid4x8d`, grids with and
without diagonals; and `scatter32r<r>s<n>`, 32 nodes placed at random in a square, in reach
within `r` percent of its side, placed again until the group is connected. `-loss<p>` loses
that share of packets on every link. Two packets that overlap at a receiver are both lost
unless one is 6 dB stronger, and every link here is equally strong.

What the runs showed:

- The cancel rule is the group half of step 7, and it already runs. In `full32`, `line12` and
  `clusters10` a flood costs the fewest packets possible. In grids and scattered groups it
  costs 34 to 80 packets where 5 to 14 could do.
- More than half of those are sends again, and 12 of about 1,200 brought the message to a node
  that lacked it. Two neighbours of a sender, hidden from each other, relay at once and collide
  at the sender, which sends again; both hold the message, and a node never relays one it holds.
- The private half of step 7, skipping a private message off the shortest path, was written
  before store-and-forward. Every node now holds every message, so a node pruning skipped
  takes it later from a summary, which costs more than the relay saved.
- A removal floods about 60 messages, a key message to each member and each one's
  acknowledgement. In `full32` every member held its key in 96 s. In `grid4x8` and the
  scattered groups it took 592 to 976 s at the median, and 26 to 55 members over four seeds
  held theirs only after the first member switched. They catch up through the old key.
- `d` with `n` cut a flood's packets by 36 to 76% in the grids and scattered groups, with
  slower worst cases. For removals it was fastest in three of six spread-out shapes and slower
  than today in two.
- `32:1` was the only change that made every spread-out removal sooner, 14 to 41%, with 17 to
  36% fewer packets, but a lost packet waited longer: a single message's slowest seed on
  `line12-loss20` went from 75 to 445 s.
