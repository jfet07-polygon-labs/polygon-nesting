#!/usr/bin/env python3
"""Independent confirmation of a probe arm's published layout.

    python3 confirm.py RUN.json OUTDIR [BINARY]

Takes the 61 placements a coordinator run returned, writes them as a pinned
parent fixture, and replays them through **mode 27** - the micro-legalization
probe, the one mode that is *meant* to be pointed at states that may not
validate and that therefore measures the residue rather than gating on it -
in a separate process, from the **default-feature gate binary**, which does not
contain the ledger or the probe phase at all.

A layout that is exact-valid and contract-valid under the real request comes
back with the fixture's own fingerprint unchanged and zero repair applied. That
is a different code path, a different build and a different process from the
one that published it.
"""
import hashlib
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import runlib  # noqa: E402


def main():
    source = json.load(open(sys.argv[1]))
    outdir = sys.argv[2]
    binary = sys.argv[3] if len(sys.argv) > 3 \
        else '/var/lib/t3/tmp/ledger-gate-binary-2'
    os.makedirs(outdir, exist_ok=True)
    request_path = runlib.REQUESTS['mixed-61']
    request_sha = hashlib.sha256(open(request_path, 'rb').read()).hexdigest()
    portfolio = source['portfolio']
    fixture = {
        'schemaVersion': 1,
        'description': 'opportunity-ledger probe publication, for confirmation',
        'requestSha256': request_sha,
        'expectedPlacementFingerprint': portfolio['incumbent']['fingerprint'],
        'reportedDepthMm': source['usedLongAxisDepthMm'],
        'independentDepthMm': source['independentUsedLongAxisDepthMm'],
        'provenance': {'source': sys.argv[1]},
        'settings': {
            'sheetShortAxisMm': source['sheetShortAxisMm'],
            'sheetLongAxisMm': source['sheetLongAxisMm'],
            'totalPaddingMm': source['pairClearanceMm'],
            'sheetEdgeClearanceMm': source['sheetEdgeClearanceMm'],
            'clearanceSafetyMarginMm': source['clearanceSafetyMarginMm'],
            'flatteningSagToleranceMm': source['flatteningSagToleranceMm'],
            'searchOffsetAllowanceMm': float(runlib.DEFAULT_ALLOWANCE),
        },
        'placements': [{
            'pieceId': p['pieceId'],
            'rotationDeg': p['rotationDeg'],
            'mirrored': p['mirrored'],
            'translateShortAxis': p['translateShortAxis'],
            'translateLongAxis': p['translateLongAxis'],
        } for p in source['placements']],
    }
    fixture_path = f'{outdir}/fixture.json'
    json.dump(fixture, open(fixture_path, 'w'), indent=1)

    args = [a.format(seed=0) for a in runlib.ARGS]
    command = ([binary, request_path] + args
               + ['27', fixture_path, '', '', runlib.DEFAULT_ALLOWANCE])
    out_path = f'{outdir}/mode27.json'
    with open(out_path, 'w') as handle:
        proc = subprocess.run(command, stdout=handle, stderr=subprocess.PIPE,
                              check=False)
    try:
        doc = json.load(open(out_path))
    except json.JSONDecodeError:
        print(json.dumps({'error': (proc.stderr or b'').decode()[-1200:]},
                         indent=1))
        return
    population = (doc.get('relaxedDiagnostics', {})
                     .get('coupledDynamicSeparator', {})
                     .get('persistentVacancyPopulation', {}))
    print(json.dumps({
        'fixture': fixture_path,
        'fixtureFingerprint': fixture['expectedPlacementFingerprint'],
        'fixtureIndependentDepthMm': fixture['independentDepthMm'],
        'binary': binary,
        'mode27': {
            'attempted': population.get('attempted'),
            'parentFingerprint': population.get('parentFingerprint'),
            'parentIndependentDepthMm':
                population.get('parentIndependentDepthMm'),
            'exactValid': population.get('exactValid'),
            'contractValid': population.get('contractValid'),
            'independentDepthMm': population.get('independentDepthMm'),
            'rawSourceDepthMm': population.get('rawSourceDepthMm'),
            'finalPlacementFingerprint':
                population.get('finalPlacementFingerprint'),
            'microLegalization': population.get('microLegalization'),
            'failureReason': population.get('failureReason'),
        },
        'fingerprintUnchanged':
            population.get('finalPlacementFingerprint')
            == fixture['expectedPlacementFingerprint'],
    }, indent=1))


if __name__ == '__main__':
    main()
