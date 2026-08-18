#!/usr/bin/env python3
"""Paired interleaved A/B of one gate stream between two binaries.

    python3 ab.py <rounds> <aLabel> <aBinary> <bLabel> <bBinary> [gate]

Arms alternate order every round; the statistic is the per-round paired ratio
b/a. Both the engine's own clock and the process wall are reported, because on
a box shared with other benchmarking agents they answer slightly different
questions and disagreeing is informative.
"""
import json
import os
import statistics
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib  # noqa: E402

rounds = int(sys.argv[1])
a_label, a_binary, b_label, b_binary = sys.argv[2:6]
tag = sys.argv[7 - 1] if len(sys.argv) > 6 else 'g1'
outdir = '/tmp/rl/ab'
gate = next(g for g in lib.GATES if g[0] == tag)

rows = []
outcomes = {a_label: set(), b_label: set()}
for index in range(rounds):
    order = [(a_label, a_binary), (b_label, b_binary)]
    if index % 2:
        order.reverse()
    timings = {}
    for label, binary in order:
        doc, wall, _ = lib.run_gate(binary, gate, outdir, label=f'{label}-r{index}-')
        timings[label] = (lib.engine_seconds(doc), wall)
        check = lib.gate_check(gate, doc)
        outcomes[label].add(json.dumps(
            {k: v for k, v in check.items() if k != 'wallSeconds'}, sort_keys=True))
    rows.append({
        'round': index,
        'first': order[0][0],
        f'{a_label}Engine': timings[a_label][0],
        f'{b_label}Engine': timings[b_label][0],
        f'{a_label}Wall': timings[a_label][1],
        f'{b_label}Wall': timings[b_label][1],
        'engineRatio': timings[b_label][0] / timings[a_label][0],
        'wallRatio': timings[b_label][1] / timings[a_label][1],
    })
    print(json.dumps(rows[-1]), flush=True)

engine = [row['engineRatio'] for row in rows]
wall = [row['wallRatio'] for row in rows]
result = {
    'gate': tag,
    'rounds': rounds,
    'armsAlternateOrderEveryRound': True,
    'statistic': 'per-round paired ratio, b over a',
    'a': {'label': a_label, 'binary': a_binary},
    'b': {'label': b_label, 'binary': b_binary},
    f'{a_label}EngineMedianSeconds': statistics.median(
        row[f'{a_label}Engine'] for row in rows),
    f'{b_label}EngineMedianSeconds': statistics.median(
        row[f'{b_label}Engine'] for row in rows),
    'engineRatioMedian': statistics.median(engine),
    'engineRatioRange': [min(engine), max(engine)],
    'wallRatioMedian': statistics.median(wall),
    'wallRatioRange': [min(wall), max(wall)],
    'roundsBelowParity': sum(1 for value in engine if value < 1.0),
    'outcomesIdenticalWithinArm': {
        label: len(values) == 1 for label, values in outcomes.items()},
    'outcomesIdenticalAcrossArms': (
        outcomes[a_label] == outcomes[b_label]),
    'rows': rows,
}
print(json.dumps({k: v for k, v in result.items() if k != 'rows'}, indent=1))
os.makedirs(outdir, exist_ok=True)
json.dump(result, open(f'{outdir}/ab-{a_label}-{b_label}-{tag}.json', 'w'),
          indent=1)
