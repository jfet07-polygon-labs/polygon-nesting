#!/usr/bin/env python3
"""**The red/green vector for the two driver repairs, before and after.**

    python3 driver-fix-vector.py <drivers-dir> <cells-dir>

Neither repair touches the engine. Both are evidence-presentation defects: a
filter that cannot fire, and a licence that is granted by an empty selection.

**D1 - `wall.py`'s checkpoint filter is in the wrong clock frame.**
Re-implements both the old predicate and the repaired one over real cell
documents and prints, per cell, what each excludes. The old one is shown to be
inert by construction: its left side is bounded above by
`budget - constructorSeconds`.

**D2 - `cutclose.py`'s `CANARY_PASS` is vacuously true when the canary did not
run.** Drives the old and the new expression over a stage list that does not
contain the canary.
"""
import glob
import json
import os
import sys

BUDGET_S = 10.0


def old_filter(publications, limit):
    return [row for row in publications
            if row.get('wallSeconds') is None or row['wallSeconds'] <= limit]


def new_filter(publications, limit, lower_offset):
    keep = []
    for row in publications:
        loop_s = row.get('wallSeconds')
        low = None if loop_s is None or lower_offset is None else lower_offset + loop_s
        if low is not None and low > limit:
            continue
        keep.append(row)
    return keep


def main():
    if len(sys.argv) < 3:
        raise SystemExit(__doc__)
    _, cells_dir = sys.argv[1], sys.argv[2]

    rows = []
    for path in sorted(glob.glob(os.path.join(cells_dir, 'wall-*.json'))):
        document = json.load(open(path))
        wall = document.get('wall', {})
        publications = document.get('outcome', {}).get('publications', [])
        constructor_s = wall.get('constructorSeconds')
        loop = [row['wallSeconds'] for row in publications
                if row.get('wallSeconds') is not None]
        before = old_filter(publications, BUDGET_S)
        after = new_filter(publications, BUDGET_S, constructor_s)
        rows.append({
            'cell': os.path.basename(path),
            'publications': len(publications),
            'constructorSeconds': constructor_s,
            'maxLoopSeconds': max(loop, default=None),
            'maxRequestSecondsLower': (None if constructor_s is None or not loop
                                       else constructor_s + max(loop)),
            'oldFilterKept': len(before),
            'newFilterKept': len(after),
            'oldFilterCanEverFire':
                (None if constructor_s is None
                 else BUDGET_S - constructor_s >= max(loop, default=0.0)) is False,
            'oldFilterHeadroomS': (None if not loop else BUDGET_S - max(loop)),
        })

    # D2, driven directly.
    stages_without_canary = [{'stage': 'bites', 'pass': True},
                             {'stage': 'merge', 'pass': True}]
    canary_rows = [row for row in stages_without_canary
                   if row['stage'] == 'canary']
    d2 = {
        'stagesRun': [row['stage'] for row in stages_without_canary],
        'oldExpression': all(row['pass'] for row in stages_without_canary
                             if row['stage'] == 'canary'),
        'newExpression': bool(canary_rows)
                         and all(row['pass'] for row in canary_rows),
    }
    d2['red'] = d2['oldExpression'] is True and d2['newExpression'] is False

    document = {
        'experiment': 'overlap-ics',
        'battery': 'evidence-audit-driver-fix-vector',
        'D1_checkpointFrame': {
            'cells': rows,
            'oldFilterExcludedAnything': any(row['oldFilterKept'] != row['publications']
                                             for row in rows),
            'newFilterExcludedAnything': any(row['newFilterKept'] != row['publications']
                                             for row in rows),
            'minOldFilterHeadroomS': min((row['oldFilterHeadroomS'] for row in rows
                                          if row['oldFilterHeadroomS'] is not None),
                                         default=None),
            'maxRequestSecondsLowerAcrossCells': max(
                (row['maxRequestSecondsLower'] for row in rows
                 if row['maxRequestSecondsLower'] is not None), default=None),
            'budgetS': BUDGET_S,
        },
        'D2_canaryLicence': d2,
    }
    print(json.dumps(document, indent=1))
    return 0


if __name__ == '__main__':
    sys.exit(main())
