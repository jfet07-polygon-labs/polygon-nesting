#!/usr/bin/env python3
"""PERTURB -> {mode 31, mode 26, mode 22} combination driver.

Perturbation is pv36-perturb.py's binding-stack nudge, verbatim in mechanism:
the k pieces with the largest TRUE transformed max-Y (the engine's own
`high_frontier_blockers` ordering) are translated d mm toward the sheet origin
along the depth axis. That drops the layout's measured depth to the (k+1)-th
piece's frontier and manufactures a small infeasible overlap stack at true
depth - exactly the compressed frontier mode 31's infeasibility certificate
fires on.

One deviation from pv36 is forced by the harness: pv36 declared
`independentDepthMm = parent_depth / (1 - contraction ratio)` on every emitted
fixture. The current binary re-derives that field from the fixture's own
placements (`check_parent_fixture_depths`) and hard-errors when it is more than
0.002 mm from a real convention, so every fixture here declares its own
truthfully measured depth. That convention only ever mattered for the warm-start
slot anyway; these states are handed to the repair modes as the PARENT, whose
depth the engine re-measures.
"""

import json
import os
import sys

sys.path.insert(0, '/var/lib/t3/tmp/combo')
import base  # noqa: E402

KS = (2, 3, 4, 6)
DS = (1.0, 2.0, 3.5)


def nudge(placements, k, d):
    extent = base.extents(placements)
    ranked = sorted(placements, key=lambda p: (-extent[p['pieceId']][1], p['pieceId']))
    ids = {p['pieceId'] for p in ranked[:k]}
    return [dict(p, translateLongAxis=p['translateLongAxis'] - d)
            if p['pieceId'] in ids else dict(p) for p in placements]


def build(parent_path, outdir, ks=KS, ds=DS):
    """Writes every perturbed fixture; returns [(tag, path, measured_depth)]."""
    os.makedirs(outdir, exist_ok=True)
    placements = json.load(open(parent_path))['placements']
    cases = []
    for k in ks:
        for d in ds:
            tag = f'k{k}-d{d}'
            perturbed = nudge(placements, k, d)
            path = f'{outdir}/{tag}.json'
            depth = base.depth_mm(perturbed)
            base.write_fixture(path, f'binding-stack nudge {tag} from {parent_path}',
                               perturbed, depth)
            cases.append((tag, path, depth))
    return cases
