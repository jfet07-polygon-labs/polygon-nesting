#!/usr/bin/env python3
"""**The raw per-bite rows, lifted out of the cell documents.**

    python3 bites.py <cells-dir> <out.json> [label]

Sol review 18's second non-gating risk, verbatim: *"The raw 27 cell documents
containing per-bite schedules were not committed. `wall.py` reduces them to
aggregate fields and drops `outcome.bites`. Consequently the README's
922/53/strike statements cannot be reconstructed from committed `wall.json`.
Commit the raw bite rows - or a lossless per-bite extract - in the rerun."*

Two things now carry that evidence and they are different halves:

  * `wall.py` keeps `bites` verbatim on every cell row it writes, so the
    **rerun's** own per-bite schedule is inside `wall.json`;
  * this script lifts the same array out of a directory of cell documents
    without running anything, which is how the **round 1** rows - the red
    trajectory vector, `5319 / 0 strikes / 0 disruptions` on seed 1 at 30 s -
    became committed evidence after the fact.

It copies the rows; it does not reduce, round, rename or filter them. The
per-cell `sourceSha256` is the SHA-256 of the document each array came out of,
so a reader who still has the raw file can prove the copy is one.
"""
import hashlib
import json
import os
import sys

BUDGETS = ['3', '10', '30']
SEEDS = list(range(9))


def extract(cells_dir, label):
    document = {
        'experiment': 'overlap-ics',
        'battery': label,
        'what': ('the per-bite rows of all 27 wall cells, copied verbatim out of '
                 'the raw cell documents, per Sol review 18 general-fidelity '
                 'risk 2'),
        'source': f'{cells_dir}/wall-<budget>s-seed<n>.json',
        'cells': {},
    }
    missing = []
    for budget in BUDGETS:
        for seed in SEEDS:
            name = f'wall-{budget}s-seed{seed}.json'
            path = os.path.join(cells_dir, name)
            if not os.path.exists(path):
                missing.append(name)
                continue
            with open(path, 'rb') as handle:
                payload = handle.read()
            cell = json.loads(payload)
            outcome = cell.get('outcome', {})
            document['cells'][f'{budget}s-seed{seed}'] = {
                'seed': seed,
                'budgetSeconds': float(budget),
                'executableSha256': cell.get('executableSha256'),
                'sourceFile': name,
                'sourceSha256': hashlib.sha256(payload).hexdigest(),
                'exploreBites': outcome.get('exploreBites'),
                'compressBites': outcome.get('compressBites'),
                # Verbatim. Every field the engine emitted per bite.
                'bites': outcome.get('bites'),
            }
    document['cellsFound'] = len(document['cells'])
    document['cellsMissing'] = missing
    return document


def main():
    if len(sys.argv) < 3:
        raise SystemExit(__doc__)
    cells_dir, out_path = sys.argv[1], sys.argv[2]
    label = sys.argv[3] if len(sys.argv) > 3 else 'raw-bite-rows'
    document = extract(cells_dir, label)
    os.makedirs(os.path.dirname(os.path.abspath(out_path)), exist_ok=True)
    with open(out_path, 'w') as handle:
        json.dump(document, handle, indent=1)
    print(json.dumps({'cellsFound': document['cellsFound'],
                      'cellsMissing': document['cellsMissing'],
                      'out': out_path}, indent=1))
    return 0 if document['cellsFound'] == 27 else 1


if __name__ == '__main__':
    sys.exit(main())
