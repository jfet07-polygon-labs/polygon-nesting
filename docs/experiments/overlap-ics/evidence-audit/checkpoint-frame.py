#!/usr/bin/env python3
"""**Which clock is `PublishedBite.wallSeconds` on, and which one does the gate
compare it against?**

    python3 checkpoint-frame.py <cells-dir> [out.json]

§0.1 of the pre-committed reading says a publication "completed after 10.000 s
cannot change that verdict", and `wall.py` implements that as

    within = [row for row in publications
              if row.get('wallSeconds') is None or row['wallSeconds'] <= limit]

with `limit = 10.000`. The engine's `wallSeconds` is
`Pacer::elapsed_s()`, and the `Pacer` is constructed inside
`Engine::run_cutclose` - **after** the constructor has already spent its share
of the request's budget. So `wallSeconds` is measured from the moment the loop
entered, and the budget it is compared against is measured from the decoded
request.

This script reads the raw cell documents (which carry `wall.constructorSeconds`
and per-publication `wallSeconds`, both of which the committed `wall.json`
reduction drops) and prints, per publication:

  * `loopSeconds`     - what `wall.py` filters on;
  * `requestSeconds`  - `constructorSeconds + loopSeconds`, which is the frame
                        the 10.000 s budget is written in;
  * whether the two verdicts differ.

`FRAME_MISMATCH` is true when any publication is inside the filter's frame and
outside the budget's, i.e. when the filter admits a publication §0.1 excludes.
`FILTER_HEADROOM_S` is how far `max(loopSeconds)` sits below the threshold the
filter uses; a large headroom means the filter cannot fire whatever happens,
which is the same finding stated as a margin.
"""
import glob
import json
import os
import sys

BUDGET_S = 10.000
BAR_MM = 168.484


def analyse(path):
    document = json.load(open(path))
    wall = document.get('wall', {})
    outcome = document.get('outcome', {})
    constructor = document.get('constructor', {})
    constructor_s = wall.get('constructorSeconds')
    publications = outcome.get('publications', [])
    rows = []
    for row in publications:
        loop_s = row.get('wallSeconds')
        request_s = None if loop_s is None or constructor_s is None else constructor_s + loop_s
        rows.append({
            'bite': row['ordinal']['bite'],
            'phase': row['phase'],
            'publishedRawDepthMm': row['publishedRawDepthMm'],
            'loopSeconds': loop_s,
            'requestSeconds': request_s,
            'insideFilterFrame': loop_s is None or loop_s <= BUDGET_S,
            'insideBudgetFrame': request_s is None or request_s <= BUDGET_S,
            'strictChild': row['placementFingerprint']
                           != constructor.get('placementFingerprint'),
        })
    admitted = [row for row in rows
                if row['insideFilterFrame'] and not row['insideBudgetFrame']]
    qualifying_filter = [row for row in rows
                         if row['insideFilterFrame'] and row['strictChild']
                         and row['publishedRawDepthMm'] <= BAR_MM]
    qualifying_budget = [row for row in rows
                         if row['insideBudgetFrame'] and row['strictChild']
                         and row['publishedRawDepthMm'] <= BAR_MM]
    loop_max = max((row['loopSeconds'] for row in rows
                    if row['loopSeconds'] is not None), default=0.0)
    return {
        'cell': os.path.basename(path),
        'seed': document.get('seed'),
        'constructorSeconds': constructor_s,
        'searchSeconds': wall.get('searchSeconds'),
        'totalSeconds': wall.get('totalSeconds'),
        'publications': len(rows),
        'maxLoopSeconds': loop_max,
        'maxRequestSeconds': max((row['requestSeconds'] for row in rows
                                  if row['requestSeconds'] is not None),
                                 default=0.0),
        'filterThresholdS': BUDGET_S,
        'filterHeadroomS': BUDGET_S - loop_max,
        'publicationsAdmittedByFilterButOutsideBudget': admitted,
        'qualifiesUnderFilterFrame': bool(qualifying_filter),
        'qualifiesUnderBudgetFrame': bool(qualifying_budget),
        'verdictDiffers': bool(qualifying_filter) != bool(qualifying_budget),
        'rows': rows,
    }


def main():
    if len(sys.argv) < 2:
        raise SystemExit(__doc__)
    cells_dir = sys.argv[1]
    out_path = sys.argv[2] if len(sys.argv) > 2 else None
    cells = [analyse(path)
             for path in sorted(glob.glob(os.path.join(cells_dir, 'wall-*.json')))]
    document = {
        'experiment': 'overlap-ics',
        'battery': 'evidence-audit-checkpoint-frame',
        'cellsDir': cells_dir,
        'cells': cells,
        'FRAME_MISMATCH': any(
            row['publicationsAdmittedByFilterButOutsideBudget'] for row in cells),
        'ANY_VERDICT_DIFFERS': any(row['verdictDiffers'] for row in cells),
        'MIN_FILTER_HEADROOM_S': min((row['filterHeadroomS'] for row in cells),
                                     default=None),
    }
    print(json.dumps({k: v for k, v in document.items() if k != 'cells'}, indent=1))
    for cell in cells:
        print(json.dumps({k: v for k, v in cell.items() if k != 'rows'}, indent=1))
    if out_path:
        os.makedirs(os.path.dirname(os.path.abspath(out_path)), exist_ok=True)
        with open(out_path, 'w') as handle:
            json.dump(document, handle, indent=1)
    return 0


if __name__ == '__main__':
    sys.exit(main())
