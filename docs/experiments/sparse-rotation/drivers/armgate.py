#!/usr/bin/env python3
"""The equal-WORK matched-arm gate, generalised to N arms on ONE binary.

    armgate.py OUTDIR BINARY PARENTSJSON ROUNDS ARM[,ARM...] [DROP_MM] [ALLOW]

`workgate.py` compares exactly two arms - the operator on and off - because
that is the only question the rotation-tax round had. This round has four:
per-rung offset against equivariant offset, design A against design B, and the
crosses. So the arms are named on the command line and each is a set of
environment flags on the same binary:

    base      nothing
    crot      design A
    crotEq    design A + equivariant construction
    sparse    design B
    sparseEq  design B + equivariant construction

Everything else is `workgate.py` unchanged: one pinned parent, one serial
mode-34 slice, the anatomy's design-slice work cap, and the raw source depth of
the best exact-valid publication with the parent as the floor.

**Paired and interleaved**, with the arm order rotated every round, which
`workgate.py` did not do - it ran each parent once. The box is shared, and this
round's claims are per-arm depth differences on the same parent, so an unpaired
ordering would let a co-tenant's compilation land on one arm.

The equality check `ablate.py` makes is deliberately *absent*. The equivariant
construction changes the surrogate's geometry, so the arms are not expected to
walk the same trajectory and a fingerprint mismatch here is the premise, not a
finding. What is compared is quality at equal work, which is the only honest
question to ask of a changed operator geometry.
"""
import hashlib
import json
import os
import statistics
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import runlib  # noqa: E402
import workgate  # noqa: E402

# arm label -> the environment it runs under, on top of the shared schedule spec.
ARMS = {
    'base': {},
    'crot': {'POLYGON_NESTING_CONTINUOUS_ROTATION': '1'},
    'crotEq': {'POLYGON_NESTING_CONTINUOUS_ROTATION': '1',
               'POLYGON_NESTING_ROTATION_EQUIVARIANT': '1'},
    'sparse': {'POLYGON_NESTING_CONTINUOUS_ROTATION': '1',
               'POLYGON_NESTING_SPARSE_ROTATION': '1'},
    'sparseEq': {'POLYGON_NESTING_CONTINUOUS_ROTATION': '1',
                 'POLYGON_NESTING_SPARSE_ROTATION': '1',
                 'POLYGON_NESTING_ROTATION_EQUIVARIANT': '1'},
}

EXTRA_KEYS = ('sparseRotation', 'rotationEquivariantOffset',
              'rotationEquivariantBuilds', 'rotationEquivariantFallbacks',
              'sparseRotationEpisodes', 'sparseRotationPiecesArmed',
              'sparseRotationSweeps', 'se2WitnessCalls', 'se2WitnessAccepted',
              'se2WitnessMs', 'se2WitnessBoughtMm')


def run_arm(binary, seed, fixture, target, arm_env, out_path, allowance):
    """`workgate.run_arm` with an arbitrary environment instead of one flag."""
    import subprocess
    import time
    args = [a.format(seed=seed) for a in runlib.ARGS]
    tail = ['34', fixture, f'{target:.17g}', '', allowance]
    command = [binary, runlib.REQUESTS['mixed-61']] + args + tail
    env = dict(os.environ)
    env['POLYGON_NESTING_PROFILE'] = '1'
    env['POLYGON_NESTING_COMPRESSION_SCHEDULE'] = workgate.SPEC.format(
        work=int(os.environ.get('WORKGATE_UNITS',
                                workgate.DESIGN_SLICE_UNITS)))
    for name in ('POLYGON_NESTING_CONTINUOUS_ROTATION',
                 'POLYGON_NESTING_ROTATION_EQUIVARIANT',
                 'POLYGON_NESTING_SPARSE_ROTATION',
                 'POLYGON_NESTING_SE2_WITNESS'):
        env.pop(name, None)
    env.update(arm_env)
    os.makedirs(os.path.dirname(out_path), exist_ok=True)
    started = time.monotonic()
    with open(out_path, 'w') as handle:
        proc = subprocess.run(command, stdout=handle, stderr=subprocess.PIPE,
                              check=False, env=env)
    wall = time.monotonic() - started
    try:
        return json.load(open(out_path)), wall, ''
    except json.JSONDecodeError:
        return None, wall, (proc.stderr or b'').decode()[-800:]


def row_for(doc, wall, parent_depth):
    row = workgate.row_for(doc, wall, parent_depth)
    pop = workgate.population(doc) or {}
    schedule = pop.get('compressionSchedule') or {}
    row.setdefault('rotation', {})
    for key in EXTRA_KEYS:
        row['rotation'][key] = schedule.get(key)
    return row


def summarise(result, labels):
    """Per-arm depth medians, and every pairwise paired difference."""
    by_key = {}
    for cell in result['cells']:
        for label, row in cell['arms'].items():
            depth = row.get('rawSourceDepthMm')
            if depth is not None:
                by_key[(label, cell['seed'], cell['round'])] = row
    summary = {'perArm': {}, 'paired': {}}
    for label in labels:
        depths = [row['rawSourceDepthMm'] for (arm, _, _), row
                  in by_key.items() if arm == label]
        walls = [row['processWallSeconds'] for (arm, _, _), row
                 in by_key.items() if arm == label]
        work = [row.get('processWorkUnits', 0) for (arm, _, _), row
                in by_key.items() if arm == label]
        rotation = {}
        for key in list(workgate.ROTATION_KEYS[1:]) + list(EXTRA_KEYS):
            rotation[key] = sum(
                (row.get('rotation') or {}).get(key) or 0
                for (arm, _, _), row in by_key.items() if arm == label)
        summary['perArm'][label] = {
            'cells': len(depths),
            'depthMedianMm': statistics.median(depths) if depths else None,
            'wallMedianSeconds': statistics.median(walls) if walls else None,
            'wallRelativeSpread': ((max(walls) - min(walls))
                                   / statistics.median(walls)
                                   if walls and statistics.median(walls)
                                   else None),
            'workMedian': statistics.median(work) if work else None,
            'rotation': rotation,
        }
    for left in labels:
        for right in labels:
            if left >= right:
                continue
            deltas, wall_ratios = [], []
            for cell in result['cells']:
                a = cell['arms'].get(left, {})
                b = cell['arms'].get(right, {})
                if ('rawSourceDepthMm' not in a
                        or 'rawSourceDepthMm' not in b):
                    continue
                # right minus left: negative means `right` is shallower, so
                # `right` is the better arm.
                deltas.append(b['rawSourceDepthMm'] - a['rawSourceDepthMm'])
                if a.get('processWallSeconds') and b.get('processWallSeconds'):
                    wall_ratios.append(a['processWallSeconds']
                                       / b['processWallSeconds'])
            if not deltas:
                continue
            summary['paired'][f'{right}-minus-{left}'] = {
                'cells': len(deltas),
                'medianDeltaMm': statistics.median(deltas),
                'minDeltaMm': min(deltas), 'maxDeltaMm': max(deltas),
                'rightBetter': sum(1 for d in deltas if d < 0),
                'leftBetter': sum(1 for d in deltas if d > 0),
                'equal': sum(1 for d in deltas if d == 0),
                'medianWallRatioLeftOverRight':
                    statistics.median(wall_ratios) if wall_ratios else None,
            }
    return summary


def main():
    outdir, binary, parents_json = sys.argv[1], sys.argv[2], sys.argv[3]
    rounds = int(sys.argv[4])
    labels = sys.argv[5].split(',')
    drop_mm = float(sys.argv[6]) if len(sys.argv) > 6 \
        else workgate.DEFAULT_DROP_MM
    allowance = sys.argv[7] if len(sys.argv) > 7 else runlib.DEFAULT_ALLOWANCE
    for label in labels:
        if label not in ARMS:
            raise SystemExit(f'unknown arm {label!r}; have {sorted(ARMS)}')
    parents = json.load(open(parents_json))['rows']
    os.makedirs(outdir, exist_ok=True)
    result = {
        'binary': binary,
        'binarySha256': hashlib.sha256(open(binary, 'rb').read()).hexdigest(),
        'workUnits': int(os.environ.get('WORKGATE_UNITS',
                                        workgate.DESIGN_SLICE_UNITS)),
        'dropMm': drop_mm, 'allowance': allowance, 'rounds': rounds,
        'parents': parents_json,
        'arms': {label: ARMS[label] for label in labels},
        'cells': [],
    }
    for round_index in range(rounds):
        ordered = labels[round_index % len(labels):] \
            + labels[:round_index % len(labels)]
        for parent in parents:
            seed = parent['seed']
            parent_depth = parent['rawDepthMm']
            target = parent_depth - drop_mm
            cell = {'round': round_index, 'seed': seed,
                    'parentRawDepthMm': parent_depth, 'arms': {}}
            for label in ordered:
                path = f'{outdir}/r{round_index}-seed{seed}-{label}.json'
                doc, wall, err = run_arm(binary, seed, parent['fixture'],
                                         target, ARMS[label], path, allowance)
                cell['arms'][label] = ({'error': err} if doc is None
                                       else row_for(doc, wall, parent_depth))
            depths = {label: cell['arms'][label].get('rawSourceDepthMm')
                      for label in ordered}
            print(f"r{round_index} seed{seed}: parent={parent_depth:.4f} "
                  + ' '.join(f'{k}={v}' for k, v in depths.items()),
                  flush=True)
            result['cells'].append(cell)
            json.dump(result, open(f'{outdir}/armgate.json', 'w'), indent=1)
    result['summary'] = summarise(result, labels)
    json.dump(result, open(f'{outdir}/armgate.json', 'w'), indent=1)
    print(json.dumps(result['summary'], indent=1))


if __name__ == '__main__':
    main()
