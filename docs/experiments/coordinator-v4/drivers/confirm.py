#!/usr/bin/env python3
"""Independent confirmation of every published layout, from a different binary.

    confirm.py WORKQUALITY.json OUTDIR BINARY [OUT.json]

Takes the placements each coordinator run returned, writes them as a pinned
parent fixture, and replays them through **mode 27** - the micro-legalization
probe, the one mode meant to be pointed at states that may not validate and
that therefore *measures* the residue rather than gating on it - in a separate
process, from a binary built from the **pristine base commit**, which contains
neither the compression-schedule class nor the mode-34 operator it calls,
and does not know the three spec keys this round adds.

A layout that is exact-valid and contract-valid under the real request comes
back with the fixture's own fingerprint unchanged and zero repair applied. That
is a different build, a different code path and a different process from the
one that published it.

A deliberate copy of `coordinator-v3/drivers/confirm.py`, itself a copy of the
opportunity ledger's, so every stage confirms on identical terms.
"""
import hashlib
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import runlib  # noqa: E402


def confirm_one(source_path, outdir, binary, request_path, request_sha):
    source = json.load(open(source_path))
    os.makedirs(outdir, exist_ok=True)
    portfolio = source['portfolio']
    fixture = {
        'schemaVersion': 1,
        'description': 'coordinator-v4 publication, for confirmation',
        'requestSha256': request_sha,
        'expectedPlacementFingerprint': portfolio['incumbent']['fingerprint'],
        'reportedDepthMm': source['usedLongAxisDepthMm'],
        'independentDepthMm': source['independentUsedLongAxisDepthMm'],
        'provenance': {'source': source_path},
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
        return {'source': source_path,
                'error': (proc.stderr or b'').decode()[-1200:]}
    population = (doc.get('relaxedDiagnostics', {})
                     .get('coupledDynamicSeparator', {})
                     .get('persistentVacancyPopulation', {}))
    micro = population.get('microLegalization') or {}
    return {
        'source': source_path,
        'fixtureFingerprint': fixture['expectedPlacementFingerprint'],
        'publishedRawDepthMm': portfolio['incumbent']['rawDepthMm'],
        'exactValid': population.get('exactValid'),
        'contractValid': population.get('contractValid'),
        'rawSourceDepthMm': population.get('rawSourceDepthMm'),
        'independentDepthMm': population.get('independentDepthMm'),
        'finalPlacementFingerprint':
            population.get('finalPlacementFingerprint'),
        'fingerprintUnchanged':
            population.get('finalPlacementFingerprint')
            == fixture['expectedPlacementFingerprint'],
        'violatingPairsBefore': micro.get('violatingPairsBefore'),
        'collisionPairsBefore': micro.get('collisionPairsBefore'),
        'movedPieces': micro.get('movedPieces'),
        'roundsRun': micro.get('roundsRun'),
        'failureReason': population.get('failureReason'),
    }


def main():
    battery = json.load(open(sys.argv[1]))
    outroot = sys.argv[2]
    binary = sys.argv[3]
    request_path = runlib.REQUESTS[battery['request']]
    request_sha = hashlib.sha256(open(request_path, 'rb').read()).hexdigest()
    rows = []
    for row in battery['rows']:
        tag = row['tag']
        source_path = (f"{runlib.OUT}/{battery['name']}/runs/{tag}.json")
        result = confirm_one(source_path, f'{outroot}/{tag}', binary,
                             request_path, request_sha)
        result.update({'tag': tag, 'arm': row['arm'], 'seed': row['seed'],
                       'work': row['work']})
        rows.append(result)
        print(f"{tag}: exact={result.get('exactValid')} "
              f"contract={result.get('contractValid')} "
              f"raw={result.get('rawSourceDepthMm')} "
              f"unchanged={result.get('fingerprintUnchanged')} "
              f"violating={result.get('violatingPairsBefore')} "
              f"moved={result.get('movedPieces')}", flush=True)
    out = {'binary': binary, 'request': battery['request'], 'rows': rows,
           'ALL_CONFIRMED': all(
               r.get('exactValid') and r.get('contractValid')
               and r.get('fingerprintUnchanged') for r in rows)}
    print(json.dumps({'ALL_CONFIRMED': out['ALL_CONFIRMED']}, indent=1))
    if len(sys.argv) > 4:
        json.dump(out, open(sys.argv[4], 'w'), indent=1)


if __name__ == '__main__':
    main()
