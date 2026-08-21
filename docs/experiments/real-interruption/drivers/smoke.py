#!/usr/bin/env python3
"""Does each new key do the thing its name says, once, on one seed?

    python3 smoke.py OUTDIR BINARY REQUEST TARGET_MS SEED

Five arms, one seed, one round. This is not a measurement - it is the check
that the wiring reaches the operator at all, run before any battery so that a
battery cannot spend an hour measuring a key that never arrived. Every column
it prints is read off the slice reports in the document:

    base        nothing armed
    wallstop    `m34wallstop=1` - the slice stops at the first checkpoint past
                the wall deadline, holding its exact-valid incumbent
    past        `m34past=1` - the slice continues past the nine-rung bound
                under the coordinator's own budget, in batches
    yield       `m34yield=2` - the slice suspends toward the coordinator every
                two batches and the queue resumes it after another action
    pastwall    both levers, which is the arm the battery measures
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import runlib  # noqa: E402

ARMS = [
    ('base', ''),
    ('wallstop', 'm34wallstop=1'),
    ('past', 'm34past=1'),
    ('yield', 'm34yield=2'),
    ('pastwall', 'm34past=1,m34wallstop=1'),
]


def slices_of(doc):
    calls = (doc.get('portfolio') or {}).get('operatorCalls') or []
    out = []
    for call in calls:
        report = call.get('scheduleSlice')
        if report:
            out.append(report)
    return out


def main():
    outdir, binary, request, target, seed = sys.argv[1:6]
    seed = int(seed)
    os.makedirs(outdir, exist_ok=True)
    rows = []
    for arm, extra in ARMS:
        spec = runlib.spec_for(seed, 'plan', target, True, extra)
        doc, wall, err = runlib.run(binary, request, seed, spec,
                                    f'{outdir}/{arm}.json')
        portfolio = doc.get('portfolio') or {}
        if not portfolio:
            print(f'{arm}: FAILED {err[-400:]}', flush=True)
            rows.append({'arm': arm, 'spec': spec, 'error': err[-400:]})
            continue
        reports = slices_of(doc)
        row = {
            'arm': arm, 'spec': spec, 'processWallSeconds': wall,
            'rawDepthMm': portfolio['incumbent']['rawDepthMm'],
            'dualGateValid': portfolio['incumbent']['dualGateValid'],
            'slices': len(reports),
            'batches': [r.get('batches', 1) for r in reports],
            'resumptions': [r.get('resumptions', 0) for r in reports],
            'interrupted': sum(1 for r in reports if r.get('interrupted')),
            'exits': [r.get('exitCause') for r in reports],
            'stepsTaken': [r.get('stepsTaken') for r in reports],
            'sliceWork': [r.get('workUnits') for r in reports],
            'firstSliceDropMm': (
                round(reports[0]['startDepthMm'] - reports[0]['finalDepthMm'], 4)
                if reports else None),
        }
        rows.append(row)
        print(f"{arm:9s} wall={wall:6.3f} depth={row['rawDepthMm']:.3f} "
              f"slices={row['slices']} batches={row['batches']} "
              f"resume={row['resumptions']} intr={row['interrupted']} "
              f"exits={row['exits']} drop0={row['firstSliceDropMm']}",
              flush=True)
    summary = {
        'binary': binary, 'binarySha256': runlib.binary_sha256(binary),
        'request': request, 'target': target, 'seed': seed,
        'rows': rows, 'boxLoad': runlib.LOAD,
    }
    with open(f'{outdir}/summary.json', 'w') as handle:
        json.dump(summary, handle, indent=2)


if __name__ == '__main__':
    main()
