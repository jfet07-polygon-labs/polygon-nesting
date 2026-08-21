#!/usr/bin/env python3
"""Produces the pinned parents `wallfixtures.py` replays from.

    python3 buildparents.py OUTDIR BINARY [SECONDS]

shapes-17, triangle-20 and the eight-piece small-N request have no pinned
parents anywhere in the campaign, which is a large part of why the previous
round measured none of them. This runs one coordinator pass per fixture from the
bare request and pins the layout it lands on.

The binary used here should be the **flag-off** one: the parent is the shared
starting point of a paired A/B, so it must not be produced by either arm's
advantage. Which layout it is does not matter to the wall ratio - both arms
replay the same one - but where it came from does, and this records it.
"""
import hashlib
import json
import os
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import runlib  # noqa: E402

ROOT = runlib.ROOT
REQUESTS = {
    'shapes-17': f'{ROOT}/tests/fixtures/shapes-17/2000x2700-compact/request.json',
    'triangle-20': f'{ROOT}/tests/fixtures/triangle-20/2000x2700-compact/'
                   'request.json',
    'small-8': f'{ROOT}/tests/vectors/core/'
               'thread-equality-mixed61-8-piece-request.json',
}


def main():
    outdir, binary = sys.argv[1], sys.argv[2]
    seconds = float(sys.argv[3]) if len(sys.argv) > 3 else 20.0
    os.makedirs(outdir, exist_ok=True)
    report = {
        'binary': binary,
        'binarySha256': hashlib.sha256(open(binary, 'rb').read()).hexdigest(),
        'budgetSeconds': seconds,
        'parents': {},
    }
    for label, request in REQUESTS.items():
        spec = (f'wall={int(seconds * 1000)},'
                f'cells={runlib.SALT_SETS[0]},v3=1,m34lanes=1,m34pconfirm=1')
        run_path = f'{outdir}/{label}-run.json'
        args = [a.format(seed=0) for a in runlib.ARGS]
        tail = ['0', '', '', '', runlib.DEFAULT_ALLOWANCE, spec]
        env = dict(os.environ)
        env.pop('POLYGON_NESTING_PROFILE', None)
        env.pop('POLYGON_NESTING_COMPRESSION_SCHEDULE', None)
        env.pop('POLYGON_NESTING_CONTRACT_VALIDATOR_CENSUS', None)
        started = time.monotonic()
        with open(run_path, 'w') as handle:
            proc = subprocess.run([binary, request] + args + tail,
                                  stdout=handle, stderr=subprocess.PIPE,
                                  check=False, env=env)
        wall = time.monotonic() - started
        try:
            doc = json.load(open(run_path))
        except json.JSONDecodeError:
            report['parents'][label] = {
                'error': (proc.stderr or b'').decode()[-400:]}
            print(f'{label}: FAILED', file=sys.stderr)
            continue
        placements = doc.get('placements') or []
        parent = {
            'schemaVersion': 1,
            'description': f'{label} pinned parent for the fast-contract-validator '
                           'per-confirmation wall',
            'requestSha256': doc.get('requestSha256'),
            'expectedPlacementFingerprint': (
                (doc.get('portfolio') or {}).get('incumbent') or {}
            ).get('fingerprint'),
            'reportedDepthMm': doc.get('usedLongAxisDepthMm'),
            'independentDepthMm': doc.get('independentUsedLongAxisDepthMm'),
            'provenance': {'source': run_path, 'binary': binary,
                           'spec': spec, 'request': request},
            # `pairClearanceMm`, NOT `requestTotalPaddingMm`. The replay checks
            # the pin's settings against the *effective* ones, and the pinned CLI
            # tail overrides the request: shapes-17 asks for 10 mm of padding and
            # the tail's positional argument 19 sets 5, so pinning the request's
            # number makes every replay die with
            # "parent fixture settings mismatch: totalPaddingMm fixture=10
            # effective=5". The run document reports both, and the effective one
            # is the layout's own.
            'settings': {
                'sheetShortAxisMm': doc.get('sheetShortAxisMm'),
                'sheetLongAxisMm': doc.get('sheetLongAxisMm'),
                'totalPaddingMm': doc.get('pairClearanceMm'),
                'sheetEdgeClearanceMm': doc.get('sheetEdgeClearanceMm'),
                'clearanceSafetyMarginMm': doc.get('clearanceSafetyMarginMm'),
                'flatteningSagToleranceMm': doc.get('flatteningSagToleranceMm'),
            },
            'placements': [
                {'pieceId': p['pieceId'], 'rotationDeg': p['rotationDeg'],
                 'mirrored': p['mirrored'],
                 'translateShortAxis': p['translateShortAxis'],
                 'translateLongAxis': p['translateLongAxis']}
                for p in placements
            ],
        }
        out_path = f'{outdir}/{label}.json'
        json.dump(parent, open(out_path, 'w'), indent=1)
        report['parents'][label] = {
            'path': out_path,
            'pieces': len(parent['placements']),
            'reportedDepthMm': parent['reportedDepthMm'],
            'independentDepthMm': parent['independentDepthMm'],
            'sha256': hashlib.sha256(open(out_path, 'rb').read()).hexdigest(),
            'processWallSeconds': wall,
        }
        print(f'{label}: {len(parent["placements"])} pieces at '
              f'{parent["independentDepthMm"]} mm', file=sys.stderr)
    json.dump(report, open(f'{outdir}/parents.json', 'w'), indent=1)
    print(json.dumps(report, indent=1))


if __name__ == '__main__':
    main()
