#!/usr/bin/env python3
"""The constructor-quality gate: descendant depth under a fixed work budget.

    python3 qualitygate.py <aLabel> <aBinary> <bLabel> <bBinary> <defaultBinary>

Four salted mode-20 arms per binary — the salt is the *target depth*, because
`construction_seed` derives from the anchor, the seed domain and the target, so
varying the relaxed-seed argument produces replicas rather than samples (the
caveat the previous round recorded). Every endpoint is then pinned and given the
identical short mode-22 descent by the **default** binary, at two relaxed seeds,
so that only the parent differs between the arms.

The reported statistic is the paired descended-depth delta, arm b minus arm a,
on each of the eight (salt, seed) pairs.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib  # noqa: E402

a_label, a_binary, b_label, b_binary, default_binary = sys.argv[1:6]
outdir = '/var/lib/t3/tmp/cinner/quality'
os.makedirs(outdir, exist_ok=True)
ANCHOR = '/var/lib/t3/tmp/ex5-seed-native.json'
# Four salts. The gate-1 target is one of them so the pinned regression number
# is inside the sample rather than beside it.
SALTS = ['320.000', '321.500', '323.000', '324.500']
DESCENT_HEADROOM_MM = 0.8


def population(doc):
    return (doc.get('relaxedDiagnostics', {})
               .get('coupledDynamicSeparator', {})
               .get('persistentVacancyPopulation'))


def pin(doc, path, description):
    pop = population(doc)
    assert pop and pop['exactValid'], 'refusing to pin a state that did not validate'
    placements = [{
        'pieceId': p['pieceId'],
        'rotationDeg': p['rotationDeg'],
        'mirrored': p['mirrored'],
        'translateShortAxis': p['translateShortAxis'],
        'translateLongAxis': p['translateLongAxis'],
    } for p in pop['finalPlacements']]
    depth = pop['independentDepthMm']
    request_sha = doc.get('requestSha256') or doc.get('request', {}).get('sha256')
    json.dump({
        'schemaVersion': 1,
        'description': description,
        'requestSha256': request_sha,
        'expectedPlacementFingerprint': pop['finalPlacementFingerprint'],
        'reportedDepthMm': depth,
        'independentDepthMm': depth,
        'provenance': {'producedBy': description},
        'placements': placements,
    }, open(path, 'w'), indent=1)
    return depth, pop['finalPlacementFingerprint']


endpoints = {}
for label, binary in ((a_label, a_binary), (b_label, b_binary)):
    for salt in SALTS:
        doc, wall, err = lib.run(binary, f'{label}-m20-{salt}', 20, ANCHOR,
                                 salt, None, outdir)
        pop = population(doc)
        if pop is None:
            endpoints[(label, salt)] = {'error': (doc.get('_loadError') or err)[:400]}
            continue
        path = f'{outdir}/parent-{label}-{salt}.json'
        depth, fingerprint = pin(doc, path,
                                 f'constructor endpoint {label} target {salt}')
        endpoints[(label, salt)] = {
            'depth': depth, 'fp': fingerprint, 'parent': path,
            'wallSeconds': wall}
        print(json.dumps({'endpoint': [label, salt], 'depth': depth,
                          'fp': fingerprint[:16]}), flush=True)

descents = {}
for (label, salt), row in endpoints.items():
    if 'error' in row:
        continue
    for seed in ('0', '1'):
        target = f'{row["depth"] + DESCENT_HEADROOM_MM:.6f}'
        doc, _, err = lib.run(default_binary, f'descend-{label}-{salt}-s{seed}',
                              22, row['parent'], target, '0.0005', outdir,
                              seed=seed)
        pop = population(doc)
        descents[(label, salt, seed)] = {
            'raw': pop.get('rawSourceDepthMm') if pop else None,
            'fp': (pop.get('finalPlacementFingerprint') if pop else None),
            'exactValid': pop.get('exactValid') if pop else None,
        }
        print(json.dumps({'descent': [label, salt, seed],
                          'raw': descents[(label, salt, seed)]['raw']}),
              flush=True)

pairs = []
for salt in SALTS:
    for seed in ('0', '1'):
        left = descents.get((a_label, salt, seed), {})
        right = descents.get((b_label, salt, seed), {})
        delta = (None if left.get('raw') is None or right.get('raw') is None
                 else right['raw'] - left['raw'])
        pairs.append({
            'salt': salt, 'relaxedSeed': seed,
            f'{a_label}Endpoint': endpoints[(a_label, salt)].get('depth'),
            f'{b_label}Endpoint': endpoints[(b_label, salt)].get('depth'),
            'endpointFingerprintsEqual':
                endpoints[(a_label, salt)].get('fp')
                == endpoints[(b_label, salt)].get('fp'),
            f'{a_label}Descended': left.get('raw'),
            f'{b_label}Descended': right.get('raw'),
            'descendedDelta': delta,
            'descendantFingerprintsEqual': left.get('fp') == right.get('fp'),
        })

result = {
    'salts': SALTS,
    'descentHeadroomMm': DESCENT_HEADROOM_MM,
    'descentBinary': default_binary,
    'a': {'label': a_label, 'binary': a_binary},
    'b': {'label': b_label, 'binary': b_binary},
    'pairs': pairs,
    'maxAbsDescendedDelta': max(
        (abs(row['descendedDelta']) for row in pairs
         if row['descendedDelta'] is not None), default=None),
    'allEndpointFingerprintsEqual': all(row['endpointFingerprintsEqual']
                                        for row in pairs),
    'allDescendantFingerprintsEqual': all(row['descendantFingerprintsEqual']
                                          for row in pairs),
}
print(json.dumps(result, indent=1))
json.dump(result, open(f'{outdir}/quality-gate.json', 'w'), indent=1)
