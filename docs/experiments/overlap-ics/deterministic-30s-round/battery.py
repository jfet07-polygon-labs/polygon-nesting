#!/usr/bin/env python3
"""Run one four-arm quality battery from the frozen round budget.

Usage:

    python3 battery.py curve30
    python3 battery.py gate10
    python3 battery.py curve3
    python3 battery.py curve60

The arm order is always control, orders, factor, composed. Gate10 runs five
fresh processes per (seed, arm); the other curves run one. Raw cells live in a
temporary evidence spool while a compact, source-hashed reduction is written
to the committed round directory.
"""

import hashlib
import json
import os
import statistics
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, '..', '..', '..', '..'))
sys.path.insert(0, f'{ROOT}/docs/experiments/overlap-ics/drivers')
import lib  # noqa: E402

BUDGET_PATH = os.environ.get(
    'ICS_DET30_BUDGET', f'{HERE}/evidence/budget/budget.json')
SPOOL_ROOT = os.environ.get(
    'ICS_DET30_SPOOL', '/var/lib/t3/tmp/overlapics/deterministic-30s-round')
EVIDENCE_ROOT = os.environ.get(
    'ICS_DET30_EVIDENCE', f'{HERE}/evidence')
ARMS = ('control', 'orders', 'factor', 'composed')
SEEDS = tuple(range(9))
WORKERS = 8
BUDGETS = {'curve3': 3.0, 'gate10': 10.0,
           'curve30': 30.0, 'curve60': 60.0}
REPETITIONS = {'curve3': 1, 'gate10': 5, 'curve30': 1, 'curve60': 1}
BAR_MM = 168.484


def sha256_of(path):
    with open(path, 'rb') as handle:
        return hashlib.sha256(handle.read()).hexdigest()


def loadavg():
    with open('/proc/loadavg') as handle:
        return [float(value) for value in handle.read().split()[:3]]


def document_digest(path):
    with open(path) as handle:
        document = json.load(handle)
    for field in lib.WALL_FIELDS:
        document.pop(field, None)
    payload = json.dumps(document, sort_keys=True, separators=(',', ':'))
    return hashlib.sha256(payload.encode()).hexdigest()


def arm_plan_path(arm_record):
    # Evidence records retain the absolute acquisition path, but execution is
    # relocatable: the committed plan beside this driver is the authority.
    name = arm_record['planName']
    return f'{HERE}/evidence/budget/plan-{name}.icscal.json'


def run_cell(stage, repetition, seed, arm, budget, search_seconds,
             arm_record, spool):
    tag = f'{stage}-{arm}-seed{seed}-r{repetition}'
    path = f'{spool}/{tag}.json'
    plan_path = arm_plan_path(arm_record)
    before_sha = sha256_of(plan_path)
    if before_sha != arm_record['planSha256']:
        raise RuntimeError(
            f'{arm}: plan changed: {before_sha} != {arm_record["planSha256"]}')
    document, process_wall, status, stderr = lib.run(
        'cutclose', 'mixed-61', path, orders=arm_record['orders'],
        mode='calibrated', plan=plan_path, currency='U0', wall=search_seconds,
        workers=WORKERS, seed=seed, arm='control', revalidate=1)
    row = {
        'stage': stage,
        'repetition': repetition,
        'seed': seed,
        'arm': arm,
        'orders': arm_record['orders'],
        'factor': arm_record['factor'],
        'budgetSeconds': budget,
        'searchBudgetSeconds': search_seconds,
        'exit': status,
        'stderr': stderr[-800:],
        'processWallSeconds': process_wall,
        'sourcePath': path,
        'sourceSha256': lib.source_sha256(path),
        'documentDigestWithoutWall': (
            document_digest(path) if status == 0 else None),
    }
    if status != 0:
        row.update(valid=False, qualifies=False)
        return row

    outcome = document.get('outcome') or {}
    constructor = document.get('constructor') or {}
    wall = document.get('wall') or {}
    publications = outcome.get('publications') or []
    strict = [
        publication for publication in publications
        if publication.get('placementFingerprint')
        != constructor.get('placementFingerprint')
    ]
    best = min((publication.get('publishedRawDepthMm')
                for publication in strict
                if publication.get('publishedRawDepthMm') is not None),
               default=None)
    revalidations = [publication.get('revalidation')
                     for publication in publications]
    every_revalidated = all(
        revalidation is not None
        and revalidation.get('depthMatchesBitwise')
        and revalidation.get('fingerprintMatches')
        for revalidation in revalidations)
    ledger = outcome.get('calibrated') or {}
    spent = (document.get('schedule') or {}).get('calibratedPlan') or {}
    plan_sha_after = sha256_of(plan_path)
    plan_ok = bool(
        spent.get('match') == 'hit'
        and spent.get('sourceSha256') == arm_record['planSha256']
        and plan_sha_after == before_sha)
    ledger_ok = bool(
        ledger.get('chargeIdentityHolds')
        and ledger.get('consumedUnitsMatchCharged')
        and ledger.get('planKey') == spent.get('wantedKey'))
    publications_carry_poses = all(
        'poses' in publication and 'placements' in publication
        for publication in publications)
    invalid = outcome.get('invalidPublications')
    valid = bool(
        invalid == 0 and every_revalidated and publications_carry_poses
        and plan_ok and ledger_ok)
    row.update({
        'valid': valid,
        'qualifies': bool(valid and best is not None and best <= BAR_MM),
        'constructorDepthMm': constructor.get('rawSourceDepthMm'),
        'constructorFingerprint': constructor.get('placementFingerprint'),
        'constructorSeconds': wall.get('constructorSeconds'),
        'searchSeconds': wall.get('searchSeconds'),
        'totalSeconds': wall.get('totalSeconds'),
        'bestStrictChildMm': best,
        'incumbentMm': (outcome.get('incumbent') or {}).get(
            'rawSourceDepthMm'),
        'incumbentIsConstructor': (outcome.get('incumbent') or {}).get(
            'fromConstructor'),
        'finalPoseDigest': document.get('finalPoseDigest'),
        'publicationsTotal': len(publications),
        'strictChildren': len(strict),
        'invalidPublications': invalid,
        'everyPublicationRevalidated': every_revalidated,
        'publicationsCarryPoses': publications_carry_poses,
        'maxRepairDisplacementMm': outcome.get(
            'repairMaxDisplacementMm'),
        'maxRepairGivebackMm': outcome.get('repairMaxGivebackMm'),
        'planMatch': spent.get('match'),
        'planSha256': spent.get('sourceSha256'),
        'planUnchangedDuringCell': plan_sha_after == before_sha,
        'planAndKeyHold': plan_ok,
        'chargeIdentityHolds': ledger.get('chargeIdentityHolds'),
        'consumedUnitsMatchCharged': ledger.get(
            'consumedUnitsMatchCharged'),
        'ledgerAndKeyHold': ledger_ok,
        'exploreAllocationUnits': ledger.get('exploreAllocationUnits'),
        'compressAllocationUnits': ledger.get('compressAllocationUnits'),
        'exploreConsumedUnits': ledger.get('exploreConsumedUnits'),
        'compressConsumedUnits': ledger.get('compressConsumedUnits'),
        'exploreBatches': ledger.get('exploreBatches'),
        'compressBatches': ledger.get('compressBatches'),
        'exploreBites': outcome.get('exploreBites'),
        'compressBites': outcome.get('compressBites'),
        'strikesTotal': sum(
            bite.get('strikes') or 0 for bite in outcome.get('bites') or []),
        'disruptionsTotal': sum(
            bite.get('disruptions') or 0
            for bite in outcome.get('bites') or []),
        'frozenStrikeArm': outcome.get('strikeArm'),
        'frozenLiteralsIntact': ((document.get('schedule') or {})
                                 .get('strikePolicy') or {})
        .get('frozenLiteralsIntact'),
    })
    return row


def representative_rows(rows):
    return [row for row in rows if row['repetition'] == 0]


def median_depth(rows):
    values = [row['bestStrictChildMm']
              if row.get('bestStrictChildMm') is not None
              else float('inf') for row in rows]
    median = statistics.median(values)
    return None if median == float('inf') else median


def summarize(rows, stage):
    summaries = {}
    for arm in ARMS:
        all_rows = [row for row in rows if row['arm'] == arm]
        seeds = [row for row in all_rows if row['repetition'] == 0]
        walls = sorted(row['processWallSeconds'] for row in all_rows)
        summaries[arm] = {
            'orders': seeds[0]['orders'],
            'factor': seeds[0]['factor'],
            'seedRows': seeds,
            'bestMm': min((row['bestStrictChildMm'] for row in seeds
                           if row.get('bestStrictChildMm') is not None),
                          default=None),
            'medianMm': median_depth(seeds),
            'qualifyingSeeds': [row['seed'] for row in seeds
                                if row.get('qualifies')],
            'quorumReached': sum(1 for row in seeds
                                 if row.get('qualifies')),
            'wallReadings': len(walls),
            'minWallSeconds': min(walls),
            'medianWallSeconds': statistics.median(walls),
            'p95WallSeconds': (
                statistics.quantiles(walls, n=100, method='inclusive')[94]
                if len(walls) > 1 else None),
            'maxWallSeconds': max(walls),
            'invalidPublications': sum(
                row.get('invalidPublications') or 0 for row in all_rows),
            'allCellsValid': all(row.get('valid') for row in all_rows),
            'allPlansAndKeysHold': all(
                row.get('planAndKeyHold') for row in all_rows),
            'allLedgersAndKeysHold': all(
                row.get('ledgerAndKeyHold') for row in all_rows),
            'allFrozenLiteralsIntact': all(
                row.get('frozenLiteralsIntact') for row in all_rows),
        }
    paired = []
    control = {row['seed']: row for row in summaries['control']['seedRows']}
    for composed in summaries['composed']['seedRows']:
        base = control[composed['seed']]
        gain = (None if base.get('bestStrictChildMm') is None
                or composed.get('bestStrictChildMm') is None else
                base['bestStrictChildMm'] - composed['bestStrictChildMm'])
        paired.append({
            'seed': composed['seed'],
            'controlMm': base.get('bestStrictChildMm'),
            'composedMm': composed.get('bestStrictChildMm'),
            'gainMm': gain,
        })
    gains = [row['gainMm'] for row in paired if row['gainMm'] is not None]
    return {
        'stage': stage,
        'arms': summaries,
        'primaryContrast': {
            'pairs': paired,
            'comparablePairs': len(gains),
            'medianGainMm': statistics.median(gains) if gains else None,
            'meanGainMm': statistics.fmean(gains) if gains else None,
        },
    }


def main():
    stage = sys.argv[1] if len(sys.argv) > 1 else 'curve30'
    if stage not in BUDGETS:
        raise SystemExit(f'unknown stage {stage!r}; expected {sorted(BUDGETS)}')
    with open(BUDGET_PATH) as handle:
        budget_document = json.load(handle)
    if not budget_document.get('BUDGET_PASS'):
        raise RuntimeError('the frozen budget did not pass')
    budget = BUDGETS[stage]
    search_seconds = budget_document['budget'][
        'searchBudgetSecondsByBudget'][str(budget)]
    repetitions = REPETITIONS[stage]
    spool = f'{SPOOL_ROOT}/{stage}'
    os.makedirs(spool, exist_ok=True)
    os.makedirs(EVIDENCE_ROOT, exist_ok=True)
    binary_sha = sha256_of(lib.BIN)
    if binary_sha != budget_document['binarySha256']:
        raise RuntimeError('the gate binary differs from the frozen plan binary')
    document = {
        'experiment': 'overlap-ics',
        'battery': f'deterministic-30s-round-{stage}',
        'spec': 'docs/deterministic-30s-round-spec.md',
        'fixture': 'mixed-61',
        'budgetSeconds': budget,
        'searchBudgetSeconds': search_seconds,
        'repetitions': repetitions,
        'seeds': list(SEEDS),
        'arms': list(ARMS),
        'workers': WORKERS,
        'revalidate': 1,
        'binary': lib.BIN,
        'binarySha256': binary_sha,
        'budgetPath': BUDGET_PATH,
        'budgetSha256': sha256_of(BUDGET_PATH),
        'planRecords': budget_document['arms'],
        'machine': {'cpus': os.cpu_count(), 'loadBefore': loadavg()},
    }
    rows = []
    started = time.monotonic()
    for repetition in range(repetitions):
        for seed in SEEDS:
            for arm in ARMS:
                row = run_cell(
                    stage, repetition, seed, arm, budget, search_seconds,
                    budget_document['arms'][arm], spool)
                rows.append(row)
                print(
                    f'[{stage}] r{repetition} seed{seed} {arm} '
                    f'depth={row.get("bestStrictChildMm")} '
                    f'wall={row["processWallSeconds"]:.3f}s '
                    f'valid={row.get("valid")}',
                    file=sys.stderr, flush=True)
    document['batterySeconds'] = time.monotonic() - started
    document['cells'] = rows
    document['summary'] = summarize(rows, stage)
    document['machine']['loadAfter'] = loadavg()
    document['cellSources'] = lib.MANIFEST
    document['binarySha256After'] = sha256_of(lib.BIN)
    document['binaryUnchangedDuringBattery'] = (
        document['binarySha256'] == document['binarySha256After'])
    if repetitions > 1:
        identities = []
        for seed in SEEDS:
            for arm in ARMS:
                selected = [row for row in rows
                            if row['seed'] == seed and row['arm'] == arm]
                digests = [row['documentDigestWithoutWall'] for row in selected]
                identities.append({
                    'seed': seed,
                    'arm': arm,
                    'digests': digests,
                    'twoProcessBitIdentical': len(set(digests[:2])) == 1,
                    'allFiveBitIdentical': len(set(digests)) == 1,
                })
        document['processIdentity'] = identities
        document['ALL_TWO_PROCESS_BIT_IDENTICAL'] = all(
            row['twoProcessBitIdentical'] for row in identities)
        document['ALL_FIVE_BIT_IDENTICAL'] = all(
            row['allFiveBitIdentical'] for row in identities)
    output_path = f'{EVIDENCE_ROOT}/{stage}.json'
    with open(output_path, 'w') as handle:
        json.dump(document, handle, indent=1)
    print(json.dumps({
        'stage': stage,
        'cells': len(rows),
        'batterySeconds': document['batterySeconds'],
        'binaryUnchanged': document['binaryUnchangedDuringBattery'],
        'arms': {
            arm: {
                'bestMm': document['summary']['arms'][arm]['bestMm'],
                'medianMm': document['summary']['arms'][arm]['medianMm'],
                'quorum': document['summary']['arms'][arm]['quorumReached'],
                'p95WallSeconds':
                    document['summary']['arms'][arm]['p95WallSeconds'],
                'valid': document['summary']['arms'][arm]['allCellsValid'],
            } for arm in ARMS
        },
        'pairedMedianGainMm':
            document['summary']['primaryContrast']['medianGainMm'],
        'allTwoProcessBitIdentical':
            document.get('ALL_TWO_PROCESS_BIT_IDENTICAL'),
        'allFiveBitIdentical': document.get('ALL_FIVE_BIT_IDENTICAL'),
    }, indent=1))
    all_valid = all(row.get('valid') for row in rows)
    return 0 if all_valid and document['binaryUnchangedDuringBattery'] else 2


if __name__ == '__main__':
    sys.exit(main())
