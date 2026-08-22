#!/usr/bin/env python3
"""Folds the arm's extra top rung into the main ladder document.

    mergeladders.py MAIN.json EXTRA.json OUT.json

The arm is the cheaper of the two, so its ladder ends at a *shorter* operator
wall than the control's does. Read as-is, the equal-wall comparison at the top
budget would be a clamp - the control's wall falls off the end of the arm's
curve and the arm is credited with its last measured depth rather than with what
it would have reached. `collect.sh` therefore runs one extra rung on the arm
alone, and this folds it in so `gateverdict.py` sees one ladder per arm.

Nothing is recomputed and nothing is averaged: the extra rung's cells are copied
into the matching seed's `arms` map under their own `union:48000000` key, and
`works` gains the budget.
"""
import json
import sys


def main():
    main_path, extra_path, out_path = sys.argv[1:4]
    document = json.load(open(main_path))
    extra = json.load(open(extra_path))
    by_seed = {cell['seed']: cell for cell in document['cells']}
    added = 0
    for cell in extra['cells']:
        target = by_seed.get(cell['seed'])
        if target is None:
            continue
        for label, row in cell['arms'].items():
            if label in target['arms']:
                raise SystemExit(f'{label} already present on seed {cell["seed"]}')
            target['arms'][label] = row
            added += 1
    for work in extra['works']:
        if work not in document['works']:
            document['works'].append(work)
    document['works'].sort()
    document['mergedFrom'] = {'main': main_path, 'extra': extra_path,
                              'cellsAdded': added}
    json.dump(document, open(out_path, 'w'), indent=1)
    print(json.dumps({'cellsAdded': added, 'works': document['works'],
                      'parents': len(document['cells'])}, indent=1))


if __name__ == '__main__':
    main()
