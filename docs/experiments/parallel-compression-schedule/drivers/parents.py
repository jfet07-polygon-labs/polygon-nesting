#!/usr/bin/env python3
"""Produces the 174-179 mm parent band the quality gate has to be run at.

    python3 parents.py SEEDS BINARY OUTDIR [WORK]

The mode-26 rung anatomy measured mode 26's *cost* on the 159.079 and 164.038
record-lineage parents and its *yield* not at all: zero of eight ladders
published. The opportunity ledger's A/B/C then measured the same operator at the
coordinator's own 174-179 mm parents and got two publications in three. So a
matched-arm gate run at 159/164 would compare two zeros; it has to be run where
the control publishes, and this is how those parents are produced.

Each seed runs the pinned coordinator from the bare request at a fixed
work-unit budget - which is deterministic and load-independent, so one run per
seed is the whole measurement - and its published incumbent is written out as a
pinned-parent fixture, together with the archive the run saturated at. Both arms
of the gate then descend from *that file*, so "the same parent at the same seed"
is a property of the input rather than of a re-run.

The binary here is the pinned default-feature gate binary: the parents must not
be produced by the build under test.
"""
import hashlib
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import runlib  # noqa: E402


def fixture_for(source_path, doc, allowance):
    request_path = runlib.REQUESTS['mixed-61']
    request_sha = hashlib.sha256(open(request_path, 'rb').read()).hexdigest()
    portfolio = doc['portfolio']
    return {
        'schemaVersion': 1,
        'description': 'compression-schedule gate parent: a coordinator '
                       'publication in the 174-179 mm band',
        'requestSha256': request_sha,
        'expectedPlacementFingerprint': portfolio['incumbent']['fingerprint'],
        'reportedDepthMm': doc['usedLongAxisDepthMm'],
        'independentDepthMm': doc['independentUsedLongAxisDepthMm'],
        'provenance': {'source': source_path},
        'settings': {
            'sheetShortAxisMm': doc['sheetShortAxisMm'],
            'sheetLongAxisMm': doc['sheetLongAxisMm'],
            'totalPaddingMm': doc['pairClearanceMm'],
            'sheetEdgeClearanceMm': doc['sheetEdgeClearanceMm'],
            'clearanceSafetyMarginMm': doc['clearanceSafetyMarginMm'],
            'flatteningSagToleranceMm': doc['flatteningSagToleranceMm'],
            'searchOffsetAllowanceMm': float(allowance),
        },
        'placements': [{
            'pieceId': p['pieceId'],
            'rotationDeg': p['rotationDeg'],
            'mirrored': p['mirrored'],
            'translateShortAxis': p['translateShortAxis'],
            'translateLongAxis': p['translateLongAxis'],
        } for p in doc['placements']],
    }


def main():
    seeds = [int(s) for s in sys.argv[1].split(',')]
    binary = sys.argv[2]
    outdir = sys.argv[3]
    work = int(sys.argv[4]) if len(sys.argv) > 4 else runlib.WORK_30S
    os.makedirs(outdir, exist_ok=True)
    result = {'binary': binary,
              'binarySha256': hashlib.sha256(open(binary, 'rb').read()).hexdigest(),
              'work': work, 'allowance': runlib.DEFAULT_ALLOWANCE, 'rows': []}
    for seed in seeds:
        spec = runlib.spec_for(seed, work)
        run_path = f'{outdir}/coordinator-seed{seed}.json'
        doc, wall, err = runlib.run(binary, 'mixed-61', seed, spec, run_path)
        if '_loadError' in doc:
            result['rows'].append({'seed': seed, 'error': err[-600:]})
            continue
        portfolio = doc['portfolio']
        fixture = fixture_for(run_path, doc, runlib.DEFAULT_ALLOWANCE)
        fixture_path = f'{outdir}/parent-seed{seed}.json'
        json.dump(fixture, open(fixture_path, 'w'), indent=1)
        # The archive the run saturated at, pinned alongside the incumbent so a
        # later arm can be pointed at a non-incumbent basin without re-running
        # the coordinator.
        archive = [{
            'rank': index,
            'operator': basin.get('operator'),
            'rawDepthMm': basin.get('rawDepthMm'),
            'exactValid': basin.get('exactValid'),
            'fingerprint': basin.get('fingerprint'),
        } for index, basin in enumerate(portfolio['archive'].get('basins', []))]
        json.dump(archive, open(f'{outdir}/archive-seed{seed}.json', 'w'), indent=1)
        result['rows'].append({
            'seed': seed,
            'spec': spec,
            'fixture': fixture_path,
            'rawDepthMm': portfolio['incumbent']['rawDepthMm'],
            'independentDepthMm': doc['independentUsedLongAxisDepthMm'],
            'dualGateValid': portfolio['incumbent']['dualGateValid'],
            'fingerprint': portfolio['incumbent']['fingerprint'],
            'workUnits': portfolio['workUnits'],
            'processWallSeconds': wall,
            'archiveMembers': len(archive),
            'inBand': portfolio['incumbent']['rawDepthMm'] is not None
            and 174.0 <= portfolio['incumbent']['rawDepthMm'] <= 179.5,
        })
    print(json.dumps(result, indent=1))
    json.dump(result, open(f'{outdir}/parents.json', 'w'), indent=1)


if __name__ == '__main__':
    main()
