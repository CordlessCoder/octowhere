# Two-board mesh run with arrival latency taken out, 2026-10-01

The same two boards and staggered start as `../mesh-2026-10-01/`, after node 28's `DIO0` joint
was reworked, so both boards time arrivals from `DIO0`.

- `node-24.log`, `node-28.log`: the mesh with `ARRIVAL_LATENCY_US` at 1,050 µs, half the
  round-trip lateness measured without it. Node 28 took node 24's clock 121 s into its first
  sweep. Node 24 then heard node 28's packets -0.15 to 0.00 ms late, where without the correction
  it heard them 2.01 to 2.11 ms late. Node 28's refinements were -0.14 to -0.34 ms, 90 to 135 s
  apart: the two crystals drift about 1.5 to 2.5 ppm apart. Neither board polled.
- `node-24-restart.log`: node 24 restarted while node 28 kept its clock, on the build before the
  latency correction. Node 24 heard its own clock, rooted at its own id, in node 28's packet 102 s
  into its sweep and took it back as root at 0 hops. Node 28's next refinement from it was
  +1.9 ms: the restarted root's clock was a hop late, which the correction now takes out.
