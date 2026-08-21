#!/usr/bin/env python3
"""One run per spec, printing the plan block and the wall it actually took.

    python3 smoke.py OUTDIR BINARY REQUEST SEED SPEC [SPEC ...]

Specs are given whole (`plan=10000,cells=...` is built for you from the seed;
pass only the budget and the extras). This is the shape check, not a
measurement: one run of each, in the order given.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import runlib  # noqa: E402


def main():
    outdir, binary, request, seed = sys.argv[1:5]
    seed = int(seed)
    specs = sys.argv[5:]
    os.makedirs(outdir, exist_ok=True)
    out = []
    for index, spec in enumerate(specs):
        key, _, value = spec.partition('=')
        budget, _, extra = value.partition(',')
        full = runlib.spec_for(seed, key, budget, True, extra)
        doc, wall, err = runlib.run(binary, request, seed, full,
                                    f'{outdir}/smoke-{index}.json')
        portfolio = doc.get('portfolio') or {}
        row = {'spec': full, 'processWallSeconds': wall,
               'budget': portfolio.get('budget'),
               'plan': portfolio.get('plan'),
               'planCalibration': portfolio.get('planCalibration'),
               'rawDepthMm': (portfolio.get('incumbent') or {}).get('rawDepthMm'),
               'workUnits': portfolio.get('workUnits'),
               'coordinatorSeconds': portfolio.get('elapsedSeconds')}
        if not portfolio:
            row['error'] = err[-400:]
        out.append(row)
        print(json.dumps(row, indent=1), flush=True)
    json.dump(out, open(f'{outdir}/smoke.json', 'w'), indent=1)
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
