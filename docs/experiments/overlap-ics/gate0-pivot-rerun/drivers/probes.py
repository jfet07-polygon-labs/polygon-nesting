#!/usr/bin/env python3
"""Read-only probes: they answer §0.2's two escape clauses and keep the
frozen-theta evidence current. No probe is a cell and no probe carries a
verdict."""
import json
import os
import sys

sys.path.insert(0, '/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_16cf0332-938-1'
                   '/docs/experiments/overlap-ics/gate0-pivot-rerun/drivers')
import lib  # noqa: E402

OUT = '/var/lib/t3/tmp/overlapics-pivot/probes'
os.makedirs(OUT, exist_ok=True)
S1_W = 150.16547


def row(name, doc, wall, status):
    o = doc.get('outcome', {})
    p = o.get('proxy', {})
    ck = o.get('exactCheckpoints', [])
    pub = [r for r in ck if r.get('publishedRawDepthMm') is not None]
    return {
        'probe': name,
        'exit': status,
        'entryRawPhi': doc.get('entry', {}).get('rawPhi'),
        'entryMaxViolationMm': doc.get('entry', {}).get('maxViolationMm'),
        'lockedTargetMm': doc.get('entry', {}).get('lockedTargetMm'),
        'constructorDepthMm': doc.get('constructor', {}).get('rawSourceDepthMm'),
        'finalRawPhi': p.get('rawPhi'),
        'finalGuidedPhi': p.get('guidedPhi'),
        'finalMaxViolationMm': p.get('maxViolationMm'),
        'finalRawDepthMm': p.get('rawSourceDepthMm'),
        'exactCheckpointAttempts': o.get('work', {}).get('exactCheckpoints'),
        'publications': len(pub),
        'publishedRawDepthMm': (o.get('incumbent') or {}).get('rawSourceDepthMm'),
        'strictChild': not (o.get('incumbent') or {}).get('fromConstructor'),
        'acceptedMoves': o.get('work', {}).get('acceptedMoves'),
        'guidedStalls': o.get('guidedStalls'),
        'jumpAttempted': o.get('jumpAttempted'),
        'jumpCommitted': o.get('jumpCommitted'),
        'jumpEvents': o.get('jumpEvents', []),
        'census': o.get('census'),
        'solverSeconds': doc.get('wall', {}).get('solverSeconds'),
        'wallSeconds': wall,
    }


rows = []

# --- Escape clause 2: is the local sweep a sweep, or is the JUMP the thing
#     that explodes Phi? Same cell, same seeds, same quota, jump allowance 0.
for seed in (0, 1, 2):
    doc, wall, status, _ = lib.run(
        'c175', 'mixed-61', f'{OUT}/c175-seed{seed}-nojump.json', seed=seed,
        budget=240_000, checkpointevery=1, jumps=0)
    rows.append(row(f'C175 seed {seed}, jump allowance 0', doc, wall, status))

# --- The frozen-theta probes, kept current. Grok review 10 Finding 3 asked for
#     the S1 one by name.
for label, extra in (('derived commit rule', {}), ('--jumpcommit=guided', {'jumpcommit': 'guided'})):
    doc, wall, status, _ = lib.run(
        's1', 'mixed-61', f'{OUT}/s1-rotation-frozen-{len(rows)}.json',
        poses=lib.SPARROW_POSES, target=S1_W, budget=200_000, seed=0,
        perturbmm=0.5, perturbdeg=2.0, checkpointevery=1, rotation='off', **extra)
    rows.append(row(f'S1, rotation OFF, {label}', doc, wall, status))

document = {
    'experiment': 'overlap-ics',
    'battery': 'gate-0-pivot-rerun probes',
    'binary': lib.BIN,
    'note': 'read-only diagnostics; no probe is a cell and none carries a verdict',
    'probes': rows,
}
print(json.dumps(document, indent=1))
with open(f'{OUT}/probes.json', 'w') as handle:
    json.dump(document, handle, indent=1)
