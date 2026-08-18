#!/usr/bin/env python3
"""Merges the arms of two gate documents that share the same parents.

    python3 merge.py OUT.json A.json B.json [C.json ...]

The gate driver runs the arms it is given; an arm added later is a separate
invocation over the same parents, and this joins them per cell rather than
re-running the ones that already exist. Cells are matched on the seed and the
merge refuses if the parent depth differs, because two documents about
different parents are not one experiment.
"""
import json
import sys


def main():
    out_path = sys.argv[1]
    documents = [json.load(open(path)) for path in sys.argv[2:]]
    merged = dict(documents[0])
    merged['cells'] = [dict(cell, arms=dict(cell['arms']))
                       for cell in documents[0]['cells']]
    merged['mergedFrom'] = sys.argv[2:]
    by_seed = {cell['seed']: cell for cell in merged['cells']}
    for document in documents[1:]:
        for cell in document['cells']:
            target = by_seed.get(cell['seed'])
            if target is None:
                merged['cells'].append(dict(cell, arms=dict(cell['arms'])))
                by_seed[cell['seed']] = merged['cells'][-1]
                continue
            if target['parentRawDepthMm'] != cell['parentRawDepthMm']:
                raise SystemExit(
                    f'seed {cell["seed"]}: parents differ, refusing to merge')
            for arm, row in cell['arms'].items():
                target['arms'].setdefault(arm, row)
    merged['cells'].sort(key=lambda cell: cell['seed'])
    json.dump(merged, open(out_path, 'w'), indent=1)
    print(json.dumps({'out': out_path,
                      'cells': len(merged['cells']),
                      'arms': sorted({arm for cell in merged['cells']
                                      for arm in cell['arms']})}, indent=1))


if __name__ == '__main__':
    main()
