#!/usr/bin/env python3
"""The A/B that isolates the pivot fix from the jump lottery.

Same cell, same three seeds, same 240,000-proposal quota, `--jumps=0` on both
arms, everything else identical. The only difference between the two arms is
the binary: `base` is commit 1f5cd5b (the torque pivot broken), `pivot` is this
round's tree. With the jump removed there is nothing stochastic left, so what
the two columns differ by is the move set and nothing else.
"""
import json
import os
import sys

ROOT = ('/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_16cf0332-938-1')
sys.path.insert(0, f'{ROOT}/docs/experiments/overlap-ics/gate0-pivot-rerun/drivers')
os.environ['ICS_ROOT'] = ROOT

BINS = {
    'base': '/var/lib/t3/tmp/pivot/base-tree/target/release/examples/overlap_ics_benchmark',
    'pivot': f'{ROOT}/target/release/examples/overlap_ics_benchmark',
}
OUT = '/var/lib/t3/tmp/overlapics-pivot/ab'
os.makedirs(OUT, exist_ok=True)

import lib  # noqa: E402

rows = []
for arm, binary in BINS.items():
    lib.BIN = binary
    for seed in (0, 1, 2):
        doc, wall, status, err = lib.run(
            'c175', 'mixed-61', f'{OUT}/{arm}-seed{seed}.json', seed=seed,
            budget=240_000, checkpointevery=1, jumps=0)
        o = doc.get('outcome', {})
        p = o.get('proxy', {})
        rc = o.get('rejectionCensus', {})
        tot = o.get('work', {}).get('pieceProposals', 0)
        acc = rc.get('acceptedProposals', 0)
        rej = rc.get('rejectedProposals', 0)
        rows.append({
            'arm': arm,
            'seed': seed,
            'exit': status,
            'entryRawPhi': doc.get('entry', {}).get('rawPhi'),
            'entryMaxViolationMm': doc.get('entry', {}).get('maxViolationMm'),
            'shockedPoseDigest': doc.get('shock', {}).get('shockedPoseDigest'),
            'finalRawPhi': p.get('rawPhi'),
            'finalMaxViolationMm': p.get('maxViolationMm'),
            'finalRawDepthMm': p.get('rawSourceDepthMm'),
            'proposals': tot,
            'acceptedProposals': acc,
            'rejectedProposals': rej,
            'zeroEnergyProposals': tot - acc - rej,
            'acceptanceRateAmongGradientForming': acc / max(acc + rej, 1),
            'guidedStalls': o.get('guidedStalls'),
            'exactCheckpointAttempts': o.get('work', {}).get('exactCheckpoints'),
            'publications': len([r for r in o.get('exactCheckpoints', [])
                                 if r.get('publishedRawDepthMm') is not None]),
            'census': o.get('census'),
            'solverSeconds': doc.get('wall', {}).get('solverSeconds'),
        })

document = {
    'experiment': 'overlap-ics',
    'battery': 'gate-0-pivot-rerun A/B: the descent alone, both pivots',
    'binaries': BINS,
    'baseCommit': '1f5cd5b0a2ba68df84d441c145afe6e367199cd7',
    'note': ('`--jumps=0` on both arms, so no topology move fires and the only '
             'difference between the columns is the pivot a proposal turns '
             'about. Read-only diagnostic; not a cell and not a verdict.'),
    'arms': rows,
}
print(json.dumps(document, indent=1))
with open(f'{OUT}/pivot-ab.json', 'w') as handle:
    json.dump(document, handle, indent=1)
