#!/usr/bin/env python3
"""Freeze f4*/f1* and build the round's keyed work plans.

The factors are derived from Gate 0 before calibration starts. Explore is
priced by the repaired engine writer on the 400-iteration shelf probe;
compress is priced by the existing wall-calibration entry point. The three
rate files differ only in their pre-declared safety factor. No quality cell is
run here and no later observation can rewrite these files.
"""

import hashlib
import json
import os
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, '..', '..', '..', '..'))
sys.path.insert(0, f'{ROOT}/docs/experiments/overlap-ics/drivers')
import lib  # noqa: E402

GATE0 = os.environ.get('ICS_DET30_GATE0', f'{HERE}/evidence/gate0/gate0.json')
OUT = os.environ.get('ICS_DET30_BUDGET_OUT', f'{HERE}/evidence/budget')
LABEL = 'single-fixture work plan, no transfer claim'
W080 = 9.5271
BASE_FACTOR = 0.80
WORKERS = 8
SHELF_BITES = 21
PROBE_ITERS = 400
CALIBRATION_WALL_SECONDS = 30.0
BUDGETS = (3.0, 10.0, 30.0, 60.0)


def sha256_of(path):
    with open(path, 'rb') as handle:
        return hashlib.sha256(handle.read()).hexdigest()


def write_json(path, value):
    with open(path, 'w') as handle:
        json.dump(value, handle, indent=1)


def run(cell_name, path, **options):
    document, process_wall, status, stderr = lib.run(
        cell_name, 'mixed-61', path, **options)
    if status != 0:
        raise RuntimeError(
            f'{cell_name} exited {status}: {stderr[-1000:]}')
    return document, process_wall


def factors(gate0):
    mixed = gate0['summaries']['mixed-61']['arms']
    c4 = mixed['A']['p95Seconds']
    c1 = mixed['B']['p95Seconds']
    ranges = [
        arm['rangeSeconds']
        for fixture in gate0['summaries'].values()
        for arm in fixture['arms'].values()
    ]
    largest_range = max(ranges)
    kappa = max(0.050, 2.0 * largest_range)

    def derive(name, orders, constructor):
        rows = []
        for hundredth in range(80, 101):
            factor = hundredth / 100.0
            modeled = constructor + (W080 - c4) * factor / BASE_FACTOR
            rows.append({
                'factor': factor,
                'modeledWallSeconds': modeled,
                'modeledPlusKappaSeconds': modeled + kappa,
                'eligible': modeled + kappa <= 10.000,
            })
        eligible = [row['factor'] for row in rows if row['eligible']]
        if not eligible:
            raise RuntimeError(f'{name}: no eligible factor in [0.80, 1.00]')
        return {
            'name': name,
            'orders': orders,
            'constructorP95Seconds': constructor,
            'candidates': rows,
            'chosenFactor': max(eligible),
        }

    return {
        'spec': 'docs/deterministic-30s-round-spec.md',
        'formula': 'Wo(f) = Co + (W0.80 - C4) * f / 0.80',
        'holdbackFormula':
            'kappa = max(0.050 s, 2 * largest within-arm Gate-0 constructor range)',
        'W0.80Seconds': W080,
        'C4Seconds': c4,
        'largestWithinArmGate0RangeSeconds': largest_range,
        'kappaSeconds': kappa,
        'rangesSeconds': ranges,
        'f4': derive('f4*', 4, c4),
        'f1': derive('f1*', 1, c1),
        'fixedBeforeCalibrationOrQuality': True,
    }


def adjusted_phase(phase, factor, arm_names):
    adjusted = dict(phase)
    adjusted['safeUnitsPerSecond'] = (
        phase['measuredUnitsPerSecond'] * factor)
    adjusted['safetyFactor'] = factor
    adjusted['derivation'] = (
        f'{LABEL}. {phase["derivation"]} Safety factor {factor:.2f}, '
        f'pre-derived for {"/".join(arm_names)} from Gate 0 before any '
        f'quality cell.')
    return adjusted


def main():
    os.makedirs(f'{OUT}/cells', exist_ok=True)
    with open(GATE0) as handle:
        gate0 = json.load(handle)
    if not gate0.get('GATE0_PASS'):
        raise RuntimeError('Gate 0 did not pass; plans must not be built')
    binary_sha = sha256_of(lib.BIN)

    # This file is written before either calibration process starts. It is the
    # immutable input to every plan below, not a summary fitted afterwards.
    derivation = factors(gate0)
    derivation.update({
        'gate0Path': GATE0,
        'gate0Sha256': sha256_of(GATE0),
        'binary': lib.BIN,
        'binarySha256': binary_sha,
    })
    factor_path = f'{OUT}/factor-derivation.json'
    write_json(factor_path, derivation)

    # Explore: exercise the repaired shelf_work_plan writer itself. Its
    # observedUnits must be the shelf bite's local counter, not the trajectory
    # cumulative vector that made the old writer 16% fast.
    shelf_path = f'{OUT}/cells/shelf-probe.json'
    shelf_plan_path = f'{OUT}/cells/shelf-writer.icscal.json'
    shelf, shelf_process_wall = run(
        'spawntax', shelf_path, orders=4, workers=WORKERS,
        prefixworkers=WORKERS, seed=0, shelfbites=SHELF_BITES,
        prefixiters=PROBE_ITERS, probeiters=PROBE_ITERS,
        icscal=shelf_plan_path, icscalsafety=BASE_FACTOR)
    shelf_written = (shelf.get('icscal') or {}).get('plan') or {}
    explore = next((row for row in shelf_written.get('phases', [])
                    if row.get('phase') == 'explore'), None)
    shelf_bite = ((shelf.get('outcome') or {}).get('bites') or [None])[0] or {}
    shelf_local_units = (shelf_bite.get('profile') or {}).get(
        'sampleEvaluations')
    cumulative_units = ((shelf.get('spawnTax') or {}).get('work') or {}).get(
        'sampleEvaluations')
    if explore is None:
        raise RuntimeError('the repaired shelf writer emitted no explore phase')
    witness = {
        'sourcePath': shelf_path,
        'sourceSha256': sha256_of(shelf_path),
        'writerPlanPath': shelf_plan_path,
        'writerPlanSha256': sha256_of(shelf_plan_path),
        'processWallSeconds': shelf_process_wall,
        'shelfLocalSampleEvaluations': shelf_local_units,
        'trajectoryCumulativeSampleEvaluations': cumulative_units,
        'writerObservedUnits': explore.get('observedUnits'),
        'writerObservedSeconds': explore.get('observedSeconds'),
        'writerMeasuredUnitsPerSecond': explore.get(
            'measuredUnitsPerSecond'),
        'oldNumeratorWouldHaveBeen': cumulative_units,
        'oldNumeratorDiffers': cumulative_units != shelf_local_units,
        'green': bool(
            shelf_local_units
            and cumulative_units
            and explore.get('observedUnits') == shelf_local_units
            and cumulative_units != shelf_local_units
            and 'probe-only wall and sampleEvaluations charged to that bite'
            in explore.get('derivation', '')),
    }
    if not witness['green']:
        raise RuntimeError('the shelf_work_plan red-to-green witness is red')

    # Compress: the shelf is an explore stall, so the other phase keeps its
    # own existing wall-calibration entry point.
    calibration_path = f'{OUT}/cells/compress-calibration.json'
    calibration_plan_path = f'{OUT}/cells/compress-writer.icscal.json'
    calibration, calibration_process_wall = run(
        'cutclose', calibration_path, orders=4, mode='wall',
        wall=CALIBRATION_WALL_SECONDS, workers=WORKERS, seed=0,
        revalidate=1, icscal=calibration_plan_path,
        icscalsafety=BASE_FACTOR)
    calibration_written = (calibration.get('icscal') or {}).get('plan') or {}
    compress = next((row for row in calibration_written.get('phases', [])
                     if row.get('phase') == 'compress'), None)
    if compress is None:
        raise RuntimeError('the calibration writer emitted no compress phase')
    if shelf_written.get('key') != calibration_written.get('key'):
        raise RuntimeError('explore and compress calibration keys differ')

    f4 = derivation['f4']['chosenFactor']
    f1 = derivation['f1']['chosenFactor']
    plan_specs = {
        'f080': (BASE_FACTOR, ['control', 'orders']),
        f'f{round(f4 * 100):03d}': (f4, ['factor']),
        f'f{round(f1 * 100):03d}': (f1, ['composed']),
    }
    plans = {}
    for name, (factor, arm_names) in plan_specs.items():
        plan = {
            'schema': 'icscal/v1',
            'key': shelf_written['key'],
            'phases': [
                adjusted_phase(explore, factor, arm_names),
                adjusted_phase(compress, factor, arm_names),
            ],
            'provenance': (
                f'{LABEL}. deterministic-30s-round/budget.py; explore from '
                f'the repaired shelf_work_plan writer at {shelf_path}; '
                f'compress from the phase writer at {calibration_path}; '
                f'factor={factor:.2f}, fixed by {factor_path} before '
                f'calibration and quality.'),
        }
        path = f'{OUT}/plan-{name}.icscal.json'
        write_json(path, plan)
        plans[name] = {
            'path': path,
            'sha256': sha256_of(path),
            'factor': factor,
            'arms': arm_names,
            'plan': plan,
        }

    arm_plan_name = {
        'control': 'f080',
        'orders': 'f080',
        'factor': f'f{round(f4 * 100):03d}',
        'composed': f'f{round(f1 * 100):03d}',
    }
    arm_orders = {'control': 4, 'orders': 1, 'factor': 4, 'composed': 1}
    plan_hits = {}
    for arm in ('control', 'orders', 'factor', 'composed'):
        plan_record = plans[arm_plan_name[arm]]
        path = f'{OUT}/cells/plan-hit-{arm}.json'
        check, process_wall = run(
            'cutclose', path, orders=arm_orders[arm], mode='calibrated',
            plan=plan_record['path'], currency='U0', wall=0.05,
            workers=WORKERS, seed=0, arm='control')
        spent = (check.get('schedule') or {}).get('calibratedPlan') or {}
        plan_hits[arm] = {
            'sourcePath': path,
            'sourceSha256': sha256_of(path),
            'processWallSeconds': process_wall,
            'orders': arm_orders[arm],
            'factor': plan_record['factor'],
            'match': spent.get('match'),
            'planSha256Expected': plan_record['sha256'],
            'planSha256AsRead': spent.get('sourceSha256'),
            'hit': bool(spent.get('match') == 'hit'
                        and spent.get('sourceSha256') == plan_record['sha256']),
        }

    c4 = derivation['C4Seconds']
    document = {
        'experiment': 'overlap-ics',
        'battery': 'deterministic-30s-round-budget',
        'label': LABEL,
        'spec': 'docs/deterministic-30s-round-spec.md',
        'binary': lib.BIN,
        'binarySha256': binary_sha,
        'gate0Path': GATE0,
        'gate0Sha256': sha256_of(GATE0),
        'factorDerivationPath': factor_path,
        'factorDerivationSha256': sha256_of(factor_path),
        'factorDerivation': derivation,
        'shelfWorkPlanWitness': witness,
        'compressCalibration': {
            'sourcePath': calibration_path,
            'sourceSha256': sha256_of(calibration_path),
            'writerPlanPath': calibration_plan_path,
            'writerPlanSha256': sha256_of(calibration_plan_path),
            'processWallSeconds': calibration_process_wall,
            'compressBites': (calibration.get('outcome') or {}).get(
                'compressBites'),
            'phase': compress,
            'qualityDepthNotUsed': True,
        },
        'plans': plans,
        'arms': {
            arm: {
                'orders': arm_orders[arm],
                'factor': plans[arm_plan_name[arm]]['factor'],
                'planName': arm_plan_name[arm],
                'planPath': plans[arm_plan_name[arm]]['path'],
                'planSha256': plans[arm_plan_name[arm]]['sha256'],
            }
            for arm in ('control', 'orders', 'factor', 'composed')
        },
        # All four arms receive the same search-duration denomination. The
        # orders=1 saving becomes extra work only through f1*, exactly as the
        # pre-committed wall model defines it.
        'budget': {
            'constructorReference': 'Gate-0 mixed-61 orders=4 p95 C4',
            'pinnedConstructorSeconds': c4,
            'searchBudgetSecondsByBudget': {
                str(seconds): seconds - c4 for seconds in BUDGETS
            },
            'exploreRatio': 0.80,
            'retuned': False,
            'fixedBeforeQuality': True,
        },
        'planHitChecks': plan_hits,
        'ALL_PLAN_HITS': all(row['hit'] for row in plan_hits.values()),
        'binarySha256After': sha256_of(lib.BIN),
        'cellSources': lib.MANIFEST,
    }
    document['binaryUnchanged'] = (
        document['binarySha256'] == document['binarySha256After'])
    document['BUDGET_PASS'] = bool(
        witness['green'] and document['ALL_PLAN_HITS']
        and document['binaryUnchanged'])
    write_json(f'{OUT}/budget.json', document)
    print(json.dumps({
        'BUDGET_PASS': document['BUDGET_PASS'],
        'f4*': f4,
        'f1*': f1,
        'kappaSeconds': derivation['kappaSeconds'],
        'shelfWriterWitnessGreen': witness['green'],
        'planHits': {arm: row['hit'] for arm, row in plan_hits.items()},
        'plans': {name: {'factor': row['factor'], 'sha256': row['sha256']}
                  for name, row in plans.items()},
    }, indent=1))
    return 0 if document['BUDGET_PASS'] else 2


if __name__ == '__main__':
    sys.exit(main())
