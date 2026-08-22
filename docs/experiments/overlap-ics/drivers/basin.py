#!/usr/bin/env python3
"""The basin sweep: how large a perturbation of a known-legal layout can this
field still return from?

    python3 basin.py [budget]

Diagnostic, not a gate. It exists because "S1 failed" is not a finding on its
own - a measure that cannot return from *any* displacement is a broken
primitive, and one that returns from 0.05 mm but not 0.5 mm is a working
primitive with a small basin. Those are different verdicts about the family and
the evidence has to be able to tell them apart.

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
    commit = sys.argv[2] if len(sys.argv) > 2 else 'always'
    rows = []
    for millimetres, degrees in LADDER:
        doc, wall, status, err = lib.run(
            's1', 'mixed-61',
            f'{out}/basin-{commit}-{millimetres}-{degrees}.json',
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
            'solverSeconds': doc.get('wall', {}).get('solverSeconds'),
            'wallSeconds': wall,
            'work': outcome.get('work'),
        })
    document = {'experiment': 'overlap-ics', 'battery': 'basin-sweep',
                'lockedWmm': LOCKED_W_MM, 'proposalBudget': budget,
                'jumpCommitRule': commit,
                'parent': 'S0 (the Sparrow correctness pin)', 'rows': rows}
    print(json.dumps(document, indent=1))
    with open(f'{out}/basin-{commit}.json', 'w') as handle:
        json.dump(document, handle, indent=1)


if __name__ == '__main__':
    main()
