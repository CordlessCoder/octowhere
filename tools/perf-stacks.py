# /// script
# requires-python = ">=3.11"
# ///
"""Summarises `perf script` output from a frame-pointer profile: the functions most samples sit
in, the functions most samples pass through, and for a leaf named by `--leaf`, the nearest frame
of the given crates that leads to it.

    perf script -i perf.data > stacks.txt
    uv run tools/perf-stacks.py stacks.txt [--leaf compress256] [--crates octowhere_]
"""

import argparse
import re
from collections import Counter

FRAME = re.compile(r"^\s+[0-9a-f]+ (.+?)(?:\+0x[0-9a-f]+)? \((.*)\)$")


def simplify(name):
    # Drop generic arguments below the outermost, which make most of a Rust symbol's length but
    # keep the type a method is on.
    out, depth = [], 0
    for ch in name:
        if ch == "<":
            depth += 1
            if depth == 1:
                out.append(ch)
            continue
        if ch == ">" and depth:
            depth -= 1
            if depth == 0:
                out.append(ch)
            continue
        if depth <= 1:
            out.append(ch)
    return re.sub(r"::h[0-9a-f]{16}$", "", "".join(out))


def samples(path):
    stack = []
    for line in open(path, errors="replace"):
        if not line.strip():
            if stack:
                yield stack
            stack = []
            continue
        m = FRAME.match(line)
        if m:
            stack.append(simplify(m[1]))
    if stack:
        yield stack


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("script")
    parser.add_argument("--leaf", action="append", default=[])
    parser.add_argument("--callers-of", action="append", default=[])
    parser.add_argument("--crates", default="octowhere_")
    parser.add_argument("--top", type=int, default=25)
    args = parser.parse_args()
    own, inclusive, total = Counter(), Counter(), 0
    leaves = {leaf: Counter() for leaf in args.leaf}
    callers_of = {name: Counter() for name in args.callers_of}
    for stack in samples(args.script):
        total += 1
        own[stack[0]] += 1
        for name in set(stack):
            inclusive[name] += 1
        for name, callers in callers_of.items():
            at = next((i for i, f in enumerate(stack) if name in f), None)
            if at is not None:
                chain = [f for f in stack[at + 1 :] if args.crates in f][:2]
                callers[" <- ".join(chain) or "(none)"] += 1
        for leaf, callers in leaves.items():
            if leaf in stack[0]:
                caller = next((f for f in stack[1:] if args.crates in f), "(none)")
                callers[caller] += 1
    print(f"{total} samples\n\nself")
    for name, n in own.most_common(args.top):
        print(f"{100 * n / total:6.1f}%  {name[:140]}")
    print(f"\ninclusive, frames of {args.crates}* only")
    for name, n in [(k, v) for k, v in inclusive.most_common() if args.crates in k][: args.top]:
        print(f"{100 * n / total:6.1f}%  {name[:140]}")
    for name, callers in callers_of.items():
        n = sum(callers.values())
        print(f"\n{name}: {100 * n / total:.1f}% of samples, through")
        for chain, k in callers.most_common(12):
            print(f"{100 * k / total:6.1f}%  {chain[:200]}")
    for leaf, callers in leaves.items():
        n = sum(callers.values())
        print(f"\n{leaf}: {100 * n / total:.1f}% of samples, nearest {args.crates}* caller")
        for name, k in callers.most_common(12):
            print(f"{100 * k / total:6.1f}%  {name[:140]}")


if __name__ == "__main__":
    main()
