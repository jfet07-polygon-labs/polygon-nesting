#!/usr/bin/env python3
"""The equal-WORK matched-arm gate: the operator against the same lane without
it, from the same pinned parent at the same seed under the same work cap.

    workgate.py OUTDIR BINARY PARENTSJSON [DROP_MM] [ALLOWANCE]

Adapted from `parallel-compression-schedule/drivers/workgate.py`, with the two
arms changed and nothing else: both run on the *same* binary and differ only in
`POLYGON_NESTING_CONTINUOUS_ROTATION`, so the build is out of the comparison and
what is left is the operator.

Work rather than wall is the denomination on purpose. The operator charges its
surrogate builds to the lane, but a build is not a candidate query, so at a
fixed *work* cap the armed arm gets the same number of proxy questions as the
unarmed one and has to pay for its rungs in quality rather than in seconds.
That is the fairest test the operator can be given: it removes the throughput
loss that dominates the wall battery and asks whether the rungs are worth
anything at all when the price is waived.

The statistic per cell is the raw source depth of the best exact-valid
publication, with the parent as the floor for both arms - the contract mode 34
already publishes under - so an arm that finds nothing scores its parent rather
than being dropped.
"""
import hashlib
import json
import os
import statistics
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import runlib  # noqa: E402

# The anatomy's design slice, the arm the compression-schedule round found most
# efficient per unit of work.
DESIGN_SLICE_UNITS = 3_341_379
DEFAULT_DROP_MM = 0.3
SPEC = 'past=1,rollback=0,work={work},lanes=1,pconfirm=0'


def run_arm(binary, seed, fixture, target, armed, out_path, allowance):
    args = [a.format(seed=seed) for a in runlib.ARGS]
    tail = ['34', fixture, f'{target:.17g}', '', allowance]
    command = [binary, runlib.REQUESTS['mixed-61']] + args + tail
    env = dict(os.environ)
    env['POLYGON_NESTING_PROFILE'] = '1'
    env['POLYGON_NESTING_COMPRESSION_SCHEDULE'] = SPEC.format(
        work=int(os.environ.get('WORKGATE_UNITS', DESIGN_SLICE_UNITS)))
    if armed:
        env['POLYGON_NESTING_CONTINUOUS_ROTATION'] = '1'
    else:
        env.pop('POLYGON_NESTING_CONTINUOUS_ROTATION', None)
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


def population(doc):
    return ((doc.get('relaxedDiagnostics') or {})
            .get('coupledDynamicSeparator') or {}).get(
                'persistentVacancyPopulation')


ROTATION_KEYS = ('continuousRotation', 'rotationRungsProposed',
                 'rotationRungsImproved', 'mirrorTogglesProposed',
                 'mirrorTogglesImproved', 'rotationAcceptedMoves',
                 'acceptedMoves', 'rotationLossBoughtMm',
                 'translationLossBoughtMm', 'rotationSurrogateBuilds',
                 'rotationSurrogateHits', 'rotationSurrogateEvictions',
                 'rotationSurrogateBuildMs', 'rotationSurrogateCells',
                 'rotationBuildsRefused')


def row_for(doc, wall, parent_depth):
    row = {'processWallSeconds': wall}
    profile = (doc.get('searchProfile') or {}).get('counters') or {}
    row['processCandidateQueries'] = profile.get('candidateQueries', 0)
    row['processExactPairTests'] = profile.get('exactPairTests', 0)
    row['processWorkUnits'] = (row['processCandidateQueries']
                               + 5 * row['processExactPairTests'])
    pop = population(doc)
    if pop is None:
        row['error'] = 'no population'
        return row
    row['exactValid'] = pop.get('exactValid')
    row['contractValid'] = pop.get('contractValid')
    raw = pop.get('rawSourceDepthMm')
    row['rawSourceDepthMm'] = raw if raw is not None else parent_depth
    row['deltaMm'] = parent_depth - row['rawSourceDepthMm']
    row['fingerprint'] = pop.get('finalPlacementFingerprint')
    schedule = pop.get('compressionSchedule')
    if schedule:
        row['schedule'] = {k: v for k, v in schedule.items() if k != 'steps'}
        row['rotation'] = {k: schedule.get(k) for k in ROTATION_KEYS}
    return row


def summarise(result):
    deltas = []
    for cell in result['cells']:
        base, arm = cell['arms'].get('base'), cell['arms'].get('crot')
        if not base or not arm:
            continue
        if 'rawSourceDepthMm' not in base or 'rawSourceDepthMm' not in arm:
            continue
        deltas.append(arm['rawSourceDepthMm'] - base['rawSourceDepthMm'])
    summary = {'cells': len(deltas)}
    if deltas:
        summary.update({
            'medianDeltaMm': statistics.median(deltas),
            'minDeltaMm': min(deltas), 'maxDeltaMm': max(deltas),
            'crotBetter': sum(1 for d in deltas if d < 0),
            'baseBetter': sum(1 for d in deltas if d > 0),
            'equal': sum(1 for d in deltas if d == 0),
        })
    for label in ('base', 'crot'):
        walls = [cell['arms'][label]['processWallSeconds']
                 for cell in result['cells']
                 if label in cell['arms']
                 and 'processWallSeconds' in cell['arms'][label]]
        work = [cell['arms'][label].get('processWorkUnits', 0)
                for cell in result['cells'] if label in cell['arms']]
        summary[f'{label}WallSecondsMedian'] = (statistics.median(walls)
                                                if walls else None)
        summary[f'{label}WorkUnitsMedian'] = (statistics.median(work)
                                              if work else None)
    rotation = {}
    for key in ROTATION_KEYS[1:]:
        rotation[key] = sum(
            (cell['arms'].get('crot', {}).get('rotation') or {}).get(key) or 0
            for cell in result['cells'])
    summary['rotation'] = rotation
    return summary


def main():
    outdir, binary, parents_json = sys.argv[1], sys.argv[2], sys.argv[3]
    drop_mm = float(sys.argv[4]) if len(sys.argv) > 4 else DEFAULT_DROP_MM
    allowance = sys.argv[5] if len(sys.argv) > 5 else runlib.DEFAULT_ALLOWANCE
    parents = json.load(open(parents_json))['rows']
    os.makedirs(outdir, exist_ok=True)
    result = {
        'binary': binary,
        'binarySha256': hashlib.sha256(open(binary, 'rb').read()).hexdigest(),
        'workUnits': int(os.environ.get('WORKGATE_UNITS',
                                        DESIGN_SLICE_UNITS)),
        'dropMm': drop_mm, 'allowance': allowance,
        'parents': parents_json,
        'arms': {'base': 'POLYGON_NESTING_CONTINUOUS_ROTATION unset',
                 'crot': 'POLYGON_NESTING_CONTINUOUS_ROTATION=1'},
        'cells': [],
    }
    for parent in parents:
        seed = parent['seed']
        parent_depth = parent['rawDepthMm']
        target = parent_depth - drop_mm
        cell = {'seed': seed, 'parentRawDepthMm': parent_depth, 'arms': {}}
        for label, armed in (('base', False), ('crot', True)):
            path = f'{outdir}/seed{seed}-{label}.json'
            doc, wall, err = run_arm(binary, seed, parent['fixture'], target,
                                     armed, path, allowance)
            cell['arms'][label] = ({'error': err} if doc is None
                                   else row_for(doc, wall, parent_depth))
        base = cell['arms']['base'].get('rawSourceDepthMm')
        arm = cell['arms']['crot'].get('rawSourceDepthMm')
        if base is not None and arm is not None:
            cell['deltaMm'] = arm - base
        print(f"seed{seed}: parent={parent_depth:.4f} base={base} "
              f"crot={arm} delta={cell.get('deltaMm')}", flush=True)
        result['cells'].append(cell)
        json.dump(result, open(f'{outdir}/workgate.json', 'w'), indent=1)
    result['summary'] = summarise(result)
    json.dump(result, open(f'{outdir}/workgate.json', 'w'), indent=1)
    print(json.dumps(result['summary'], indent=1))


if __name__ == '__main__':
    main()
