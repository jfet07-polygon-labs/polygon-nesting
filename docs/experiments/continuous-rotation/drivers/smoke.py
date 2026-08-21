#!/usr/bin/env python3
"""One run per arm, with the operator's own counters printed.

    smoke.py OUTNAME REQUEST SEED WALLMS [EXTRA]

The first instrument of the round and the one that decides whether the rest is
worth running: it says whether the operator fired at all under the coordinator
(`rotationRungsProposed > 0`), whether anything it proposed was accepted, and
what the surrogate builds cost - per m34 slice, from the run's own report
rather than from a profile.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import runlib  # noqa: E402

KEYS = ('continuousRotation', 'rotationRungsProposed', 'rotationRungsImproved',
        'mirrorTogglesProposed', 'mirrorTogglesImproved',
        'rotationAcceptedMoves', 'acceptedMoves', 'rotationLossBoughtMm',
        'translationLossBoughtMm', 'rotationSurrogateBuilds',
        'rotationSurrogateHits', 'rotationSurrogateEvictions',
        'rotationSurrogateBuildMs')


def main():
    name, request, seed, wall_ms = sys.argv[1], sys.argv[2], int(sys.argv[3]), \
        sys.argv[4]
    extra = sys.argv[5] if len(sys.argv) > 5 else ''
    spec = runlib.spec_for(seed, 'wall', wall_ms, True, extra)
    out = f'{runlib.OUT}/{name}/{name}.json'
    doc, seconds, err = runlib.run(runlib.BIN, request, seed, spec, out)
    portfolio = doc.get('portfolio') or {}
    incumbent = portfolio.get('incumbent') or {}
    slices = [call for call in portfolio.get('operatorCalls', [])
              if call.get('operator') == 'mode34']
    report = {
        'binary': runlib.BIN, 'spec': spec, 'request': request, 'seed': seed,
        'processSeconds': seconds,
        'engineDepthMm': doc.get('independentUsedLongAxisDepthMm'),
        'rawDepthMm': incumbent.get('rawDepthMm'),
        'dualGateValid': incumbent.get('dualGateValid'),
        'incumbentSource': incumbent.get('source'),
        'm34Slices': len(slices),
        'm34Published': sum(1 for call in slices if call.get('published')),
        'slices': [{key: (call.get('scheduleSlice') or {}).get(key)
                    for key in KEYS} for call in slices],
        'loadError': doc.get('_loadError'),
        'stderr': err[-400:] if err else '',
    }
    totals = {key: sum((row.get(key) or 0) for row in report['slices'])
              for key in KEYS if key != 'continuousRotation'}
    report['totals'] = totals
    os.makedirs(os.path.dirname(out), exist_ok=True)
    json.dump(report, open(f'{runlib.OUT}/{name}/summary.json', 'w'), indent=1)
    print(json.dumps(report, indent=1))


if __name__ == '__main__':
    main()
