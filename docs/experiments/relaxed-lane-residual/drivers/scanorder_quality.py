#!/usr/bin/env python3
"""The class (B) scan-ordering quality gate: descendant depth at a fixed budget.

    python3 scanorder_quality.py <parentBinary> <aLabel> <aBin> <bLabel> <bBin> ...

The lever under test lives in the *relaxed lane*, so unlike the constructor
stage's gate the arms run the **descent** and a single binary produces the
parents. Four mode-20 endpoints — the salt is the target depth, because
`construction_seed` derives from the anchor, the seed domain and the target, so
varying the relaxed-seed argument produces replicas rather than samples — are
pinned once by `parentBinary`. Every arm then descends each of those four
parents at two relaxed seeds, on the identical pinned schedule, so the only
thing that differs across arms is the order the candidate scan visits its
neighbours.

The statistic is the paired `rawSourceDepthMm` delta, arm b minus arm a, on each
of the eight (salt, seed) cells; lower is better. The falsifier is
`exactValid`/`contractValid`: a cell where either is false in any arm sinks the
lever regardless of the deltas.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib  # noqa: E402

parent_binary = sys.argv[1]
arms = [(sys.argv[i], sys.argv[i + 1]) for i in range(2, len(sys.argv), 2)]
outdir = '/var/lib/t3/tmp/relaxb/quality'
os.makedirs(outdir, exist_ok=True)
ANCHOR = '/var/lib/t3/tmp/ex5-seed-native.json'
# Four salts. The gate-1 target is one of them so the pinned regression number
# is inside the sample rather than beside it.
SALTS = ['320.000', '321.500', '323.000', '324.500']
# The designed replication is two relaxed seeds; `RELAXB_SEEDS=0,1,2,3` widens
# it, which is how the eight designed cells were extended to sixteen after one
# of them moved.
SEEDS = tuple(os.environ.get('RELAXB_SEEDS', '0,1').split(','))
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


parents = {}
for salt in SALTS:
    doc, wall, err = lib.run(parent_binary, f'parent-m20-{salt}', 20, ANCHOR,
                             salt, None, outdir)
    path = f'{outdir}/parent-{salt}.json'
    depth, fingerprint = pin(doc, path, f'scan-order gate parent, target {salt}')
    parents[salt] = {'depth': depth, 'fp': fingerprint, 'parent': path,
                     'wallSeconds': wall}
    print(json.dumps({'parent': salt, 'depth': depth, 'fp': fingerprint[:16]}),
          flush=True)

descents = {}
for label, binary in arms:
    for salt in SALTS:
        row = parents[salt]
        target = f'{row["depth"] + DESCENT_HEADROOM_MM:.6f}'
        for seed in SEEDS:
            doc, wall, err = lib.run(binary, f'descend-{label}-{salt}-s{seed}',
                                     22, row['parent'], target, '0.0005',
                                     outdir, seed=seed)
            pop = population(doc)
            diagnostics = doc.get('relaxedDiagnostics') or {}
            descents[(label, salt, seed)] = {
                'raw': pop.get('rawSourceDepthMm') if pop else None,
                'independent': pop.get('independentDepthMm') if pop else None,
                'fp': (pop.get('finalPlacementFingerprint') if pop else None),
                'exactValid': pop.get('exactValid') if pop else None,
                'contractValid': pop.get('contractValid') if pop else None,
                'surrogateEvaluations': diagnostics.get('surrogateEvaluations'),
                'pieceBroadPhaseProbes': diagnostics.get('pieceBroadPhaseProbes'),
                'acceptedMoves': diagnostics.get('acceptedMoves'),
                'wallSeconds': wall,
                'error': (doc.get('_loadError') or err or '')[:300] or None,
            }
            print(json.dumps({'descent': [label, salt, seed],
                              'raw': descents[(label, salt, seed)]['raw'],
                              'exactValid': descents[(label, salt, seed)]['exactValid'],
                              'contractValid': descents[(label, salt, seed)]['contractValid']}),
                  flush=True)

a_label = arms[0][0]
cells = []
for salt in SALTS:
    for seed in SEEDS:
        base = descents[(a_label, salt, seed)]
        cell = {'salt': salt, 'relaxedSeed': seed,
                'parentDepth': parents[salt]['depth'],
                'parentFingerprint': parents[salt]['fp'][:16]}
        for label, _ in arms:
            row = descents[(label, salt, seed)]
            cell[label] = {
                'raw': row['raw'], 'fp': (row['fp'] or '')[:16],
                'exactValid': row['exactValid'],
                'contractValid': row['contractValid'],
                'surrogateEvaluations': row['surrogateEvaluations'],
                'pieceBroadPhaseProbes': row['pieceBroadPhaseProbes'],
            }
            if label != a_label:
                cell[f'{label}MinusA'] = (
                    None if row['raw'] is None or base['raw'] is None
                    else row['raw'] - base['raw'])
        cells.append(cell)

summary = {'parentBinary': parent_binary,
           'arms': [{'label': label, 'binary': binary} for label, binary in arms],
           'salts': SALTS, 'seeds': list(SEEDS),
           'descentHeadroomMm': DESCENT_HEADROOM_MM,
           'statistic': 'paired rawSourceDepthMm delta per (salt, seed), b minus a',
           'cells': cells}
for label, _ in arms[1:]:
    deltas = [c[f'{label}MinusA'] for c in cells if c.get(f'{label}MinusA') is not None]
    summary[f'{label}Deltas'] = {
        'n': len(deltas), 'min': min(deltas), 'max': max(deltas),
        'sum': sum(deltas), 'mean': sum(deltas) / len(deltas),
        'cellsBetter': sum(1 for d in deltas if d < 0),
        'cellsWorse': sum(1 for d in deltas if d > 0),
        'cellsEqual': sum(1 for d in deltas if d == 0),
    }
summary['allCellsValid'] = all(
    c[label]['exactValid'] and c[label]['contractValid']
    for c in cells for label, _ in arms)
print(json.dumps(summary, indent=1))
json.dump(summary, open(
    f'{outdir}/scanorder-quality-{len(SEEDS)}seeds.json', 'w'), indent=1)
