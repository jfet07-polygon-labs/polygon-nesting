#!/usr/bin/env python3
"""Coordinator quality at an identical WORK budget, across relaxed seeds.

    python3 coordquality.py <workUnits> <seeds> <label> <bin> [<label> <bin> ...]

`PortfolioBudget::Work` is deterministic and load-independent, so every arm is
handed the same scheduled search and the incumbent it publishes is a *quality*
statistic rather than a timing one. A lever that only reorders a scan cannot buy
extra search here — the budget is in work units, not seconds — so this measures
exactly the thing the class (B) question asks: is the different trajectory
better, worse, or the same at equal work?

Reported per (arm, seed): the incumbent's raw depth, its dual-gate validity, and
the publication count. Seeds are replicas of one another; the paired statistic
is per seed.
"""
import json
import os
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib  # noqa: E402

units = sys.argv[1]
seeds = sys.argv[2].split(',')
arms = [(sys.argv[i], sys.argv[i + 1]) for i in range(3, len(sys.argv), 2)]
spec = f'work={units},cells=13:15:17:19'
outdir = '/var/lib/t3/tmp/relaxb/coordquality'
os.makedirs(outdir, exist_ok=True)

# The pinned positional tail, with the relaxed-seed slot addressed by index so
# the rest of the contract is untouched.
RELAXED_SEED_SLOT = 25


def once(binary, seed, label):
    argv = list(lib.ARGS)
    argv = [a.format(clamp='0', seed=seed) for a in argv]
    assert argv[RELAXED_SEED_SLOT] == seed, argv[RELAXED_SEED_SLOT]
    command = [binary, lib.REQ] + argv + ['0', '', '', '', '0.002', spec]
    env = dict(os.environ)
    env.pop('POLYGON_NESTING_PROFILE', None)
    path = f'{outdir}/{label}-s{seed}.json'
    started = time.monotonic()
    with open(path, 'w') as handle:
        subprocess.run(command, stdout=handle, stderr=subprocess.DEVNULL,
                       check=False, env=env)
    wall = time.monotonic() - started
    doc = json.load(open(path))
    portfolio = doc.get('portfolio') or {}
    incumbent = portfolio.get('incumbent') or {}
    return {
        'rawDepthMm': incumbent.get('rawDepthMm'),
        'dualGateValid': incumbent.get('dualGateValid'),
        'fingerprint': (incumbent.get('placementFingerprint')
                        or incumbent.get('finalPlacementFingerprint') or '')[:16],
        'publications': portfolio.get('publications'),
        'coordinatorSeconds': portfolio.get('elapsedSeconds'),
        'processWallSeconds': wall,
    }


rows = {}
for label, binary in arms:
    for seed in seeds:
        rows[(label, seed)] = once(binary, seed, label)
        print(json.dumps({'arm': label, 'seed': seed, **rows[(label, seed)]}),
              flush=True)

base = arms[0][0]
cells = []
for seed in seeds:
    cell = {'seed': seed}
    for label, _ in arms:
        cell[label] = rows[(label, seed)]
        if label != base:
            left = rows[(base, seed)]['rawDepthMm']
            right = rows[(label, seed)]['rawDepthMm']
            cell[f'{label}MinusA'] = (None if left is None or right is None
                                      else right - left)
    cells.append(cell)

summary = {'spec': spec, 'seeds': seeds,
           'arms': [{'label': label, 'binary': binary} for label, binary in arms],
           'statistic': 'incumbent rawDepthMm at an identical work budget',
           'cells': cells}
for label, _ in arms[1:]:
    deltas = [c[f'{label}MinusA'] for c in cells
              if c.get(f'{label}MinusA') is not None]
    summary[f'{label}Deltas'] = {
        'n': len(deltas), 'min': min(deltas), 'max': max(deltas),
        'mean': sum(deltas) / len(deltas),
        'cellsBetter': sum(1 for d in deltas if d < 0),
        'cellsWorse': sum(1 for d in deltas if d > 0),
        'cellsEqual': sum(1 for d in deltas if d == 0)}
print(json.dumps(summary, indent=1))
json.dump(summary, open(f'{outdir}/coordquality-{units}.json', 'w'), indent=1)
