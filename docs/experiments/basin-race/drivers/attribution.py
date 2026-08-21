#!/usr/bin/env python3
"""Sol review 8 §2 P0, reproduced on this binary: the two counters disagree.

    attribution.py OUTDIR BINARY PARENTSJSON [DROP_MM] [ALLOWANCE]

Three arms per pinned parent, at the same work cap, on the same binary:

    control   no rotation operator at all - `POLYGON_NESTING_CONTINUOUS_ROTATION`
              unset, so **zero rungs are ever proposed**
    designA   the operator armed on every piece of every descent
    designB   the sparse operator: rungs only for the pieces a stalled
              schedule step names

The claim under test is that `rotationAcceptedMoves` is not the operator's
attribution. On the control arm it must be large and the operator's own chain
must be exactly zero - which is the shape of the sparse-rotation round's own
control cell, 11,523 accepted "rotation" moves against zero rungs. On an armed
arm the two must differ by however many catalogue starts happened to win at a
different angle, which is what makes the ratio worth reporting.

Nothing here is about depth. The depth column is printed only so a reader can
see that the arms really are three different searches.
"""
import hashlib
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import runlib  # noqa: E402
import witnessab  # noqa: E402

ARMS = {
    'control': {},
    'designA': {'POLYGON_NESTING_CONTINUOUS_ROTATION': '1'},
    'designB': {'POLYGON_NESTING_CONTINUOUS_ROTATION': '1',
                'POLYGON_NESTING_SPARSE_ROTATION': '1',
                'POLYGON_NESTING_ROTATION_EQUIVARIANT': '1'},
}

CHAIN = ('sparseRotationEpisodes', 'sparseRotationRungsProposed',
         'sparseRotationRungWinners', 'sparseRotationCommittedMoves',
         'sparseRotationCommittedEpisodes')
LEGACY = ('rotationRungsProposed', 'rotationAcceptedMoves', 'acceptedMoves')


def run_arm(binary, seed, fixture, target, env_extra, out_path, allowance):
    """`witnessab.run_arm` with the base environment replaced, not extended."""
    saved = dict(witnessab.BASE_ENV)
    try:
        witnessab.BASE_ENV.clear()
        return witnessab.run_arm(binary, seed, fixture, target, env_extra,
                                 out_path, allowance)
    finally:
        witnessab.BASE_ENV.clear()
        witnessab.BASE_ENV.update(saved)


def main():
    outdir, binary, parents_json = sys.argv[1:4]
    drop_mm = float(sys.argv[4]) if len(sys.argv) > 4 \
        else witnessab.DEFAULT_DROP_MM
    allowance = sys.argv[5] if len(sys.argv) > 5 else runlib.DEFAULT_ALLOWANCE
    parents = json.load(open(parents_json))['rows']
    os.makedirs(outdir, exist_ok=True)
    result = {
        'binary': binary,
        'binarySha256': hashlib.sha256(open(binary, 'rb').read()).hexdigest(),
        'workUnits': int(os.environ.get('WITNESSAB_UNITS',
                                        witnessab.DESIGN_SLICE_UNITS)),
        'dropMm': drop_mm, 'allowance': allowance, 'arms': ARMS, 'cells': [],
    }
    for parent in parents:
        seed = parent['seed']
        target = parent['rawDepthMm'] - drop_mm
        cell = {'seed': seed, 'parentRawDepthMm': parent['rawDepthMm'],
                'arms': {}}
        for label, env_extra in ARMS.items():
            path = f'{outdir}/seed{seed}-{label}.json'
            doc, wall, err = run_arm(binary, seed, parent['fixture'], target,
                                     env_extra, path, allowance)
            if doc is None:
                cell['arms'][label] = {'error': err}
                continue
            pop = witnessab.population(doc) or {}
            slice_report = pop.get('compressionSchedule') or {}
            cell['arms'][label] = {
                'rawSourceDepthMm': pop.get('rawSourceDepthMm'),
                'wallSeconds': wall,
                **{k: slice_report.get(k) for k in CHAIN + LEGACY},
            }
        result['cells'].append(cell)
        row = cell['arms']
        print(f'seed {seed}: ' + '  '.join(
            f'{label}[accepted={row[label].get("rotationAcceptedMoves")} '
            f'committed={row[label].get("sparseRotationCommittedMoves")}]'
            for label in ARMS), flush=True)

    summary = {}
    for label in ARMS:
        agg = {k: 0 for k in CHAIN + LEGACY}
        for cell in result['cells']:
            arm = cell['arms'].get(label) or {}
            for key in agg:
                agg[key] += arm.get(key) or 0
        committed = agg['sparseRotationCommittedMoves']
        agg['acceptedOverCommitted'] = (
            agg['rotationAcceptedMoves'] / committed if committed else None)
        agg['committedEpisodeFraction'] = (
            agg['sparseRotationCommittedEpisodes']
            / agg['sparseRotationEpisodes']
            if agg['sparseRotationEpisodes'] else None)
        summary[label] = agg
    result['summary'] = summary
    json.dump(result, open(f'{outdir}/attribution.json', 'w'), indent=1)
    print(json.dumps(summary, indent=1))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
