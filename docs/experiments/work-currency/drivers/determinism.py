#!/usr/bin/env python3
"""§3.2: the campaign's hard gate, with the currency armed.

Two processes, same binary, same spec, same seed. A **work** budget is a
function of counters and of nothing else, so the two documents must be
identical - and that is precisely the claim the parallel currency has to keep,
because the currency's counts are counters and its weights are constants.
There is nothing in `class_self_units` that reads a clock, and the price is
integer arithmetic, so a `cur2=1` run is as reproducible as a `cur2=0` one or
it has a bug.

Reported per cell: whether the two documents are equal, and - when a plan
budget is used - whether the two processes chose the same `plan.units` first,
because a plan-mode disagreement about the *budget* is a different failure
from a disagreement about the *document* and `calibrated-plan` §7 predicts the
first under load.

    python3 determinism.py OUT_JSON BINARY REQUESTS SEEDS MODE VALUE [EXTRA]
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import runlib  # noqa: E402


def main():
    out, binary = sys.argv[1:3]
    requests = sys.argv[3].split(',')
    seeds = [int(v) for v in sys.argv[4].split(',')]
    mode, value = sys.argv[5], sys.argv[6]
    extra = sys.argv[7] if len(sys.argv) > 7 else ''
    outdir = os.path.dirname(out)

    rows = []
    for request in requests:
        for seed in seeds:
            spec = runlib.spec_for(seed, mode, value, True, extra)
            docs, walls = [], []
            for process in (0, 1):
                tag = f'det-{request}-s{seed}-p{process}'
                doc, wall, _ = runlib.run(binary, request, seed, spec,
                                          f'{outdir}/{tag}.json')
                docs.append(doc)
                walls.append(wall)
            plans = [((d.get('portfolio') or {}).get('plan') or {}).get('units')
                     for d in docs]
            digests = [runlib.doc_digest(d) for d in docs]
            diff = runlib.leaf_diff(docs[0], docs[1])
            currencies = [(d.get('portfolio') or {}).get('workCurrency')
                          for d in docs]
            rows.append({
                'request': request, 'seed': seed, 'spec': spec,
                'plans': plans, 'plansAgree': plans[0] == plans[1],
                'digests': digests, 'equal': digests[0] == digests[1],
                'leafDiff': diff,
                'depths': [((d.get('portfolio') or {}).get('incumbent')
                            or {}).get('rawDepthMm') for d in docs],
                'workUnits': [(d.get('portfolio') or {}).get('workUnits')
                              for d in docs],
                'classUnits': [(c or {}).get('classUnits')
                               for c in currencies],
                'chargedExtraUnits': [(c or {}).get('chargedExtraUnits')
                                      for c in currencies],
                'walls': walls,
            })
            print(f"{request} s{seed}: equal={rows[-1]['equal']} "
                  f"plansAgree={rows[-1]['plansAgree']} "
                  f"depths={rows[-1]['depths']} "
                  f"classUnits={rows[-1]['classUnits']}", flush=True)

    document = {
        'binary': binary, 'binarySha256': runlib.sha256_of(binary),
        'mode': mode, 'value': value, 'extra': extra, 'rows': rows,
        'allEqual': all(r['equal'] for r in rows),
        'allPlansAgree': all(r['plansAgree'] for r in rows),
        'equalCells': sum(r['equal'] for r in rows),
        'cells': len(rows),
        'boxLoad': runlib.LOAD,
    }
    with open(out, 'w') as handle:
        json.dump(document, handle, indent=1, sort_keys=True)
    print(f"allEqual={document['allEqual']} "
          f"{document['equalCells']}/{document['cells']}")
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
