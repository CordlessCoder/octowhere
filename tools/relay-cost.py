# /// script
# requires-python = ">=3.11"
# dependencies = ["scipy", "numpy"]
# ///
"""Sets what a flood cost in the simulator beside what it could cost on the same links.

Reads the lines `tests/relay_cost.rs` writes (RELAY_COST_OUT) and prints, per shape, the packets
that carried one group message in the simulator, beside:

- best: the fewest packets any flood from the origin can use, a connected dominating set that
  holds the origin, solved exactly;
- cancel: the cancel rule with no losses, each node relaying unless a packet it heard first
  reported every neighbour it has, over random orders;
- mpr: designated relays as OLSR picks them, each sender naming a set of its neighbours that
  reaches every node two hops away, and a node relaying only when the sender it first heard
  named it;
- path: the packets a private message to the farthest node needs on a shortest path alone.

    uv run tools/relay-cost.py results/flood-cost.jsonl
"""

import json
import random
import statistics
import sys
from collections import defaultdict

import numpy as np
from scipy.optimize import Bounds, LinearConstraint, milp
from scipy.sparse import lil_matrix

ORDERS = 200


def adjacency(nodes, edges):
    adj = [set() for _ in range(nodes)]
    for a, b in edges:
        adj[a].add(b)
        adj[b].add(a)
    return adj


def best(adj, origin=0):
    """The fewest transmitters that reach every node, connected from the origin."""
    n = len(adj)
    arcs = [(u, v) for u in range(n) for v in adj[u]]
    # Variables: x_v for each node, then a flow on each arc.
    nx = n
    nvar = nx + len(arcs)
    cost = np.zeros(nvar)
    cost[:nx] = 1
    lo = []
    hi = []
    A = lil_matrix((2 * n + 2 * len(arcs), nvar))
    r = 0
    # Every node is a transmitter or next to one.
    for v in range(n):
        A[r, v] = 1
        for u in adj[v]:
            A[r, u] = 1
        lo.append(1)
        hi.append(np.inf)
        r += 1
    # Flow from the origin: each transmitter but the origin keeps one unit.
    for v in range(n):
        if v == origin:
            continue
        for i, (a, b) in enumerate(arcs):
            if b == v:
                A[r, nx + i] += 1
            if a == v:
                A[r, nx + i] -= 1
        A[r, v] -= 1
        lo.append(0)
        hi.append(0)
        r += 1
    # Flow only between transmitters.
    for i, (a, b) in enumerate(arcs):
        A[r, nx + i] = 1
        A[r, a] = -(n - 1)
        lo.append(-np.inf)
        hi.append(0)
        r += 1
        A[r, nx + i] = 1
        A[r, b] = -(n - 1)
        lo.append(-np.inf)
        hi.append(0)
        r += 1
    A = A[:r]
    lower = np.zeros(nvar)
    lower[origin] = 1
    upper = np.full(nvar, np.inf)
    upper[:nx] = 1
    integrality = np.zeros(nvar)
    integrality[:nx] = 1
    result = milp(
        cost,
        constraints=LinearConstraint(A.tocsr(), lo, hi),
        integrality=integrality,
        bounds=Bounds(lower, upper),
    )
    assert result.success, result.message
    return round(result.fun)


def cancel(adj, rng, origin=0):
    """Transmissions under the cancel rule, lossless, in one random order of relays."""
    n = len(adj)
    has = {origin}
    pending = []
    sent = 0
    queue = [origin]
    while queue:
        u = queue.pop(0)
        sent += 1
        reported = adj[u]
        for v in adj[u]:
            if v not in has:
                has.add(v)
                pending.append(v)
                # Its sender reports every neighbour it has.
                if (adj[v] - {u}) <= reported:
                    pending.remove(v)
            elif v in pending and (adj[v] - {u}) <= reported:
                pending.remove(v)
        if not queue and pending:
            rng.shuffle(pending)
            queue.append(pending.pop(0))
    assert len(has) == n
    return sent


def mpr_sets(adj):
    sets = []
    for u in range(len(adj)):
        two = set().union(*(adj[v] for v in adj[u])) - adj[u] - {u}
        chosen = set()
        left = set(two)
        # Neighbours that alone reach some two-hop node first.
        for w in two:
            only = [v for v in adj[u] if w in adj[v]]
            if len(only) == 1:
                chosen.add(only[0])
        for v in chosen:
            left -= adj[v]
        while left:
            v = max(adj[u] - chosen, key=lambda v: (len(adj[v] & left), -v))
            chosen.add(v)
            left -= adj[v]
        sets.append(chosen)
    return sets


def mpr(adj, sets, rng, origin=0):
    """Transmissions when each node relays only if the sender it first heard named it."""
    has = {origin}
    queue = [origin]
    sent = 0
    while queue:
        rng.shuffle(queue)
        u = queue.pop(0)
        sent += 1
        for v in adj[u]:
            if v not in has:
                has.add(v)
                if v in sets[u]:
                    queue.append(v)
    assert len(has) == len(adj), "designated relays left a node out"
    return sent


def farthest_path(adj, origin=0):
    dist = {origin: 0}
    frontier = [origin]
    while frontier:
        nxt = []
        for u in frontier:
            for v in adj[u]:
                if v not in dist:
                    dist[v] = dist[u] + 1
                    nxt.append(v)
        frontier = nxt
    return max(dist.values())


def main():
    runs = defaultdict(list)
    for line in open(sys.argv[1]):
        run = json.loads(line)
        runs[run["shape"]].append(run)
    print(
        f"{'shape':<16}{'nodes':>6}{'deg':>6}{'sim':>12}{'again':>7}"
        f"{'best':>6}{'cancel':>9}{'mpr':>8}{'path':>6}{'reached s':>12}"
    )
    for shape, group in runs.items():
        first = group[0]
        adj = adjacency(first["nodes"], first["edges"])
        degree = 2 * len(first["edges"]) / first["nodes"]
        rng = random.Random(1)
        cancels = [cancel(adj, rng) for _ in range(ORDERS)]
        sets = mpr_sets(adj)
        mprs = [mpr(adj, sets, rng) for _ in range(ORDERS)]
        sim = [run["carrying"] for run in group]
        again = sum(run["again"] for run in group)
        reached = [run["reached_s"] for run in group if run["reached_s"] is not None]
        print(
            f"{shape:<16}{first['nodes']:>6}{degree:>6.1f}"
            f"{min(sim):>5}-{max(sim):<6}{again:>7}"
            f"{best(adj):>6}{statistics.mean(cancels):>9.1f}{statistics.mean(mprs):>8.1f}"
            f"{farthest_path(adj):>6}"
            f"{(f'{min(reached):.1f}-{max(reached):.1f}' if reached else 'never'):>12}"
        )


if __name__ == "__main__":
    main()
