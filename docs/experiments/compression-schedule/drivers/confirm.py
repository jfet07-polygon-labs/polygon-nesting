#!/usr/bin/env python3
"""Independent confirmation of an arm's published layout.

    python3 confirm.py RUN.json OUTDIR BINARY [ALLOWANCE]

Takes the 61 placements a run returned, writes them out as a pinned parent
fixture, and replays them through **mode 27** - the micro-legalization probe,
the one mode meant to be pointed at states that may not validate, so it measures
the residue instead of gating on it - in a separate process from the
**default-feature gate binary**, which contains neither the compression schedule
nor mode 34.

A layout that is exact-valid and contract-valid under the real request comes
back with the fixture's own fingerprint unchanged and zero repair applied. That
is a different build, a different code path and a different process agreeing
that the layout is legal at the depth it claims. This protocol is the
opportunity ledger's, unchanged, because a claim this size should be checked the
way that round's claims were checked.
"""
import hashlib
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import runlib  # noqa: E402


def population(doc):
    return ((doc.get('relaxedDiagnostics') or {})
            .get('coupledDynamicSeparator') or {}).get(
                'persistentVacancyPopulation') or {}


def main():
    source_path = sys.argv[1]
    outdir = sys.argv[2]
    binary = sys.argv[3]
    allowance = sys.argv[4] if len(sys.argv) > 4 else runlib.DEFAULT_ALLOWANCE
    source = json.load(open(source_path))
    pop = population(source)
    os.makedirs(outdir, exist_ok=True)
    request_path = runlib.REQUESTS['mixed-61']
    request_sha = hashlib.sha256(open(request_path, 'rb').read()).hexdigest()
    fixture = {
        'schemaVersion': 1,
        'description': 'compression-schedule publication, for confirmation',
        'requestSha256': request_sha,
        'expectedPlacementFingerprint': pop.get('finalPlacementFingerprint'),
        'reportedDepthMm': source['usedLongAxisDepthMm'],
        'independentDepthMm': source['independentUsedLongAxisDepthMm'],
        'provenance': {'source': source_path,
                       'mode': pop.get('mode'),
                       'rawSourceDepthMm': pop.get('rawSourceDepthMm')},
        'settings': {
            'sheetShortAxisMm': source['sheetShortAxisMm'],
            'sheetLongAxisMm': source['sheetLongAxisMm'],
            'totalPaddingMm': source['pairClearanceMm'],
            'sheetEdgeClearanceMm': source['sheetEdgeClearanceMm'],
            'clearanceSafetyMarginMm': source['clearanceSafetyMarginMm'],
            'flatteningSagToleranceMm': source['flatteningSagToleranceMm'],
            'searchOffsetAllowanceMm': float(allowance),
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
               + ['27', fixture_path, '', '', allowance])
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
    replay = population(doc)
    print(json.dumps({
        'source': source_path,
        'fixture': fixture_path,
        'fixtureFingerprint': fixture['expectedPlacementFingerprint'],
        'claimedRawSourceDepthMm': pop.get('rawSourceDepthMm'),
        'binary': binary,
        'binarySha256': hashlib.sha256(open(binary, 'rb').read()).hexdigest(),
        'mode27': {
            'attempted': replay.get('attempted'),
            'parentFingerprint': replay.get('parentFingerprint'),
            'parentIndependentDepthMm':
                replay.get('parentIndependentDepthMm'),
            'exactValid': replay.get('exactValid'),
            'contractValid': replay.get('contractValid'),
            'independentDepthMm': replay.get('independentDepthMm'),
            'rawSourceDepthMm': replay.get('rawSourceDepthMm'),
            'finalPlacementFingerprint':
                replay.get('finalPlacementFingerprint'),
            'microLegalization': replay.get('microLegalization'),
            'failureReason': replay.get('failureReason'),
        },
        'fingerprintUnchanged':
            replay.get('finalPlacementFingerprint')
            == fixture['expectedPlacementFingerprint'],
        'depthAgrees':
            replay.get('rawSourceDepthMm') == pop.get('rawSourceDepthMm'),
    }, indent=1))


if __name__ == '__main__':
    main()
