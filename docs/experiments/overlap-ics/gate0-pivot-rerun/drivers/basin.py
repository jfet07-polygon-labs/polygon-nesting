#!/usr/bin/env python3
"""The basin sweep, re-run: how large a perturbation of a known-legal layout
can this field return from, now that the jump exists?

    python3 basin.py [budget] [jumpcommit]

Diagnostic, not a gate, and unchanged from the previous round's `basin.py`
except that `jumpcommit` now defaults to *absent* - which means the binary's
own derived default rather than a driver override. The A/B arm is still one
command away (`python3 basin.py 200000 guided`).

The parent is the S0 pin, so every row starts from a layout both exact
authorities accept at W = 150.16547.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib  # noqa: E402

LOCKED_W_MM = 150.16547
LADDER = [
    (0.005, 0.02),
    (0.02, 0.08),
    (0.05, 0.2),
    (0.1, 0.4),
    (0.25, 1.0),
    (0.5, 2.0),
    (2.0, 10.0),
]


def main():
    out = os.environ.get('ICS_OUT', lib.OUT)
    budget = int(sys.argv[1]) if len(sys.argv) > 1 else 200_000
    commit = sys.argv[2] if len(sys.argv) > 2 else None
    label = commit or 'default'
    rows = []
    for millimetres, degrees in LADDER:
        doc, wall, status, err = lib.run(
            's1', 'mixed-61',
            f'{out}/basin-{label}-{millimetres}-{degrees}.json',
            poses=lib.SPARROW_POSES, target=LOCKED_W_MM, budget=budget,
            seed=0, perturbmm=millimetres, perturbdeg=degrees,
            jumpcommit=commit, checkpointevery=1)
        if status != 0:
            rows.append({'perturbationMm': millimetres,
                         'perturbationDeg': degrees,
                         'exit': status, 'stderr': err})
            continue
        outcome = doc.get('outcome', {})
        published = lib.published(doc)
        rows.append({
            'perturbationMm': millimetres,
            'perturbationDeg': degrees,
            'entryRawPhi': doc.get('entry', {}).get('rawPhi'),
            'entryMaxViolationMm': doc.get('entry', {}).get('maxViolationMm'),
            'finalRawPhi': outcome.get('proxy', {}).get('rawPhi'),
            'finalMaxViolationMm': outcome.get('proxy', {}).get('maxViolationMm'),
            'census': outcome.get('census'),
            'exactCheckpoints': len(lib.checkpoints(doc)),
            'publications': len(published),
            'publishedRawDepthMm': (outcome.get('incumbent', {})
                                    .get('rawSourceDepthMm')),
            'invalidPublications': lib.invalid_publications(doc),
            'maxRepairUm': lib.max_repair_um(doc),
            'maxGivebackMm': lib.max_giveback_mm(doc),
            'republished': bool(published),
            'jumps': outcome.get('jumps'),
            'jumpAttempted': outcome.get('jumpAttempted'),
            'jumpCommitted': outcome.get('jumpCommitted'),
            'jumpsImprovingGuided': outcome.get('jumpsImprovingGuided'),
            'jumpEvents': lib.jump_events(doc),
            'solverSeconds': doc.get('wall', {}).get('solverSeconds'),
            'wallSeconds': wall,
            'work': outcome.get('work'),
        })
    document = {'experiment': 'overlap-ics', 'battery': 'basin-sweep-rerun',
                'lockedWmm': LOCKED_W_MM, 'proposalBudget': budget,
                'jumpCommitRule': commit or 'derived default (unconditional)',
                'parent': 'S0 (the Sparrow correctness pin)', 'rows': rows}
    print(json.dumps(document, indent=1))
    with open(f'{out}/basin-{label}.json', 'w') as handle:
        json.dump(document, handle, indent=1)


if __name__ == '__main__':
    main()
