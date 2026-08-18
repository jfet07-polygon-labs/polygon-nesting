#!/usr/bin/env python3
"""The frontier stack of a pinned layout: which pieces set the depth.

    python3 frontier.py PIN [N]

The perturbation instruments (`k` deepest pieces moved in by `d`) and the
flatten grid are both parameterised by this stack, and the two lineages behave
differently precisely because their stacks do (the from-scratch frontier's ranks
1-8 sit within 0.0225 mm, the record line's do not), so it is worth printing
rather than guessing.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib  # noqa: E402

PIN = sys.argv[1]
N = int(sys.argv[2]) if len(sys.argv) > 2 else 12
placements = json.load(open(PIN))['placements']
extent = lib.extents(placements)
ranked = sorted(((high, pid) for pid, (_, high) in extent.items()), reverse=True)
frontier = ranked[0][0]
print(f'depth {lib.depth_mm(placements)!r}  frontier maxY {frontier!r}')
for index, (high, pid) in enumerate(ranked[:N], 1):
    print(f'{index:3d}  {pid:44s} maxY={high!r:22s} '
          f'gap={frontier - high:.6f}')
