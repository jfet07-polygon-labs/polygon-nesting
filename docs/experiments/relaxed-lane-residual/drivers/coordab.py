#!/usr/bin/env python3
"""Paired interleaved A/B of a WORK-budgeted coordinator run.

    python3 coordab.py <rounds> <workUnits> <aLabel> <aBin> <bLabel> <bBin>

A work budget is reproducible, so both arms do the same scheduled work and the
wall clock is the comparison. Arms alternate order every round; the statistic is
the per-round paired ratio b/a. The incumbent depth is recorded per arm so a
timing win cannot hide a different search.
"""
import json
import os
import statistics
import subprocess
import sys
import time

ROOT = '/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_8545aefe-80d-2'
REQ = f'{ROOT}/tests/fixtures/mixed-61/mixed61-request-exact-clearance.json'
ARGS = ('1 4 0 0 0 0 1 0 0 1 1 0 16 4 8 0 0 5 5 24 8 40 10 10 5 5 '
        '0.005 0.001 1 6 0 0 0 structured 0 10 1 0 0 0 0').split()

rounds = int(sys.argv[1])
units = sys.argv[2]
a_label, a_binary, b_label, b_binary = sys.argv[3:7]
spec = f'work={units},cells=13:15:17:19'
outdir = '/tmp/rl/coordab'
os.makedirs(outdir, exist_ok=True)


def once(label, binary, index):
    command = [binary, REQ] + ARGS + ['0', '', '', '', '0.002', spec]
    env = dict(os.environ)
    env.pop('POLYGON_NESTING_PROFILE', None)
    path = f'{outdir}/{label}-r{index}.json'
    started = time.monotonic()
    with open(path, 'w') as handle:
        subprocess.run(command, stdout=handle, stderr=subprocess.DEVNULL,
                       check=False, env=env)
    wall = time.monotonic() - started
    doc = json.load(open(path))
    portfolio = doc.get('portfolio') or {}
    return (portfolio.get('elapsedSeconds'), wall,
            json.dumps({'depth': (portfolio.get('incumbent') or {}).get('rawDepthMm'),
                        'valid': (portfolio.get('incumbent') or {}).get('dualGateValid'),
                        'publications': portfolio.get('publications')},
                       sort_keys=True))


rows = []
outcomes = {a_label: set(), b_label: set()}
for index in range(rounds):
    order = [(a_label, a_binary), (b_label, b_binary)]
    if index % 2:
        order.reverse()
    timings = {}
    for label, binary in order:
        engine, wall, outcome = once(label, binary, index)
        timings[label] = (engine, wall)
        outcomes[label].add(outcome)
    rows.append({
        'round': index, 'first': order[0][0],
        f'{a_label}Coordinator': timings[a_label][0],
        f'{b_label}Coordinator': timings[b_label][0],
        'coordinatorRatio': timings[b_label][0] / timings[a_label][0],
        'wallRatio': timings[b_label][1] / timings[a_label][1],
    })
    print(json.dumps(rows[-1]), flush=True)

coord = [r['coordinatorRatio'] for r in rows]
wall = [r['wallRatio'] for r in rows]
result = {
    'spec': spec, 'rounds': rounds,
    'armsAlternateOrderEveryRound': True,
    'statistic': 'per-round paired ratio, b over a',
    'a': {'label': a_label, 'binary': a_binary},
    'b': {'label': b_label, 'binary': b_binary},
    f'{a_label}CoordinatorMedianSeconds': statistics.median(
        r[f'{a_label}Coordinator'] for r in rows),
    f'{b_label}CoordinatorMedianSeconds': statistics.median(
        r[f'{b_label}Coordinator'] for r in rows),
    'coordinatorRatioMedian': statistics.median(coord),
    'coordinatorRatioRange': [min(coord), max(coord)],
    'wallRatioMedian': statistics.median(wall),
    'wallRatioRange': [min(wall), max(wall)],
    'roundsBelowParity': sum(1 for v in coord if v < 1.0),
    'outcomesIdenticalWithinArm': {
        label: len(values) == 1 for label, values in outcomes.items()},
    'outcomesIdenticalAcrossArms': outcomes[a_label] == outcomes[b_label],
    'outcomes': {label: sorted(values) for label, values in outcomes.items()},
    'rows': rows,
}
print(json.dumps({k: v for k, v in result.items() if k != 'rows'}, indent=1))
json.dump(result, open(f'{outdir}/coordab-{a_label}-{b_label}-{units}.json', 'w'),
          indent=1)
