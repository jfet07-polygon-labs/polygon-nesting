#!/usr/bin/env python3
"""The paired interleaved wall A/B: the honest multiplier of one m34 slice.

    python3 wall.py PARENTSDIR OUTDIR BINARY ARMS ROUNDS SEEDS [DROP_MM]

Equal **walk**, not equal work. Every arm is asked for the same drop with
`past=0` and no work cap, so the bound alone decides the step count and all
arms take exactly `drop / step_mm` steps. That is the arm in which the seven
idle lanes are free: at a fixed number of depth steps, a fan-out that runs
eight workers per step in parallel costs the wall of one worker, and anything
it finds is a gain. The equal-*work* comparison, where the fan-out has to pay
for all eight, is `workgate.py`, and the two must be read together.

Protocol, per the campaign's wall rules:

  * paired and interleaved - one round runs every arm on every parent before
    the next round starts, so a slow minute on a shared box lands on all arms;
  * arms alternate order every round, so no arm always runs first into a cold
    page cache;
  * the statistic is the per-round paired ratio, and the report carries the
    within-arm spread next to the between-arm delta, because this box is
    shared with another measurement agent.

Two clocks are reported and they are not the same measurement. `sliceMs` is
the engine's own `repairMs + confirmationMs` - the operator under test and
nothing else. `processWallSeconds` is the whole process including the
identical mode-0 preamble every arm pays, and it is the box's number, not the
arm's.
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

ARMS = {
    'serial': 'past=0,rollback=0,lanes=1,pconfirm=0',
    'lanes8': 'past=0,rollback=0,lanes=8,pconfirm=0',
    'lanes4': 'past=0,rollback=0,lanes=4,pconfirm=0',
    'pconfirm': 'past=0,rollback=0,lanes=1,pconfirm=1',
    'both': 'past=0,rollback=0,lanes=8,pconfirm=1',
}
DEFAULT_DROP_MM = 1.5


def run_arm(binary, seed, fixture, target, spec, out_path):
    args = [a.format(seed=seed) for a in runlib.ARGS]
    tail = ['34', fixture, f'{target:.17g}', '', runlib.DEFAULT_ALLOWANCE]
    command = [binary, runlib.REQUESTS['mixed-61']] + args + tail
    env = dict(os.environ)
    env.pop('POLYGON_NESTING_PROFILE', None)
    env['POLYGON_NESTING_COMPRESSION_SCHEDULE'] = spec
    os.makedirs(os.path.dirname(out_path), exist_ok=True)
    started = time.monotonic()
    with open(out_path, 'w') as handle:
        proc = subprocess.run(command, stdout=handle, stderr=subprocess.PIPE,
                              check=False, env=env)
    wall = time.monotonic() - started
    try:
        doc = json.load(open(out_path))
    except json.JSONDecodeError:
        return {'error': (proc.stderr or b'').decode()[-500:],
                'processWallSeconds': wall}
    pop = ((doc.get('relaxedDiagnostics') or {})
           .get('coupledDynamicSeparator') or {}).get(
               'persistentVacancyPopulation') or {}
    schedule = pop.get('compressionSchedule') or {}
    return {
        'processWallSeconds': wall,
        'repairMs': schedule.get('repairMs'),
        'confirmationMs': schedule.get('confirmationMs'),
        'sliceMs': (schedule.get('repairMs') or 0)
                   + (schedule.get('confirmationMs') or 0),
        'stepsTaken': schedule.get('stepsTaken'),
        'sweepsRun': schedule.get('sweepsRun'),
        'confirmationsAttempted': schedule.get('confirmationsAttempted'),
        'confirmationsAccepted': schedule.get('confirmationsAccepted'),
        'candidateQueries': schedule.get('candidateQueries'),
        'workUnits': schedule.get('workUnits'),
        'rawSourceDepthMm': pop.get('rawSourceDepthMm'),
        'fingerprint': pop.get('finalPlacementFingerprint'),
        'parallel': schedule.get('parallel'),
    }


def spread(values):
    values = [v for v in values if v is not None]
    if not values:
        return None
    return {
        'n': len(values),
        'median': statistics.median(values),
        'min': min(values),
        'max': max(values),
        'relSpread': (max(values) - min(values)) / statistics.median(values)
        if statistics.median(values) else None,
    }


def main():
    parents_dir = sys.argv[1]
    outdir = sys.argv[2]
    binary = sys.argv[3]
    arms = sys.argv[4].split(',')
    rounds = int(sys.argv[5])
    seeds = [int(s) for s in sys.argv[6].split(',')]
    drop_mm = float(sys.argv[7]) if len(sys.argv) > 7 else DEFAULT_DROP_MM
    parents = {p['seed']: p
               for p in json.load(open(f'{parents_dir}/parents.json'))['rows']}
    os.makedirs(outdir, exist_ok=True)
    result = {
        'binary': binary,
        'binarySha256': hashlib.sha256(open(binary, 'rb').read()).hexdigest(),
        'arms': {arm: ARMS[arm] for arm in arms},
        'rounds': rounds, 'seeds': seeds, 'dropMm': drop_mm,
        'protocol': 'paired interleaved; arm order reversed on odd rounds; '
                    'equal walk (past=0, no work cap) so every arm takes the '
                    'same number of steps',
        'observations': [],
    }
    for rnd in range(rounds):
        order = arms if rnd % 2 == 0 else list(reversed(arms))
        for seed in seeds:
            parent = parents[seed]
            target = parent['rawDepthMm'] - drop_mm
            for arm in order:
                row = run_arm(binary, seed, parent['fixture'], target,
                              ARMS[arm], f'{outdir}/r{rnd}-s{seed}-{arm}.json')
                row.update({'round': rnd, 'seed': seed, 'arm': arm})
                result['observations'].append(row)
        json.dump(result, open(f'{outdir}/wall.json', 'w'), indent=1)
    result['summary'] = summarise(result, arms, seeds)
    json.dump(result, open(f'{outdir}/wall.json', 'w'), indent=1)
    print(json.dumps(result['summary'], indent=1))


def summarise(result, arms, seeds):
    obs = result['observations']

    def pick(arm, seed, rnd, field):
        for row in obs:
            if (row['arm'] == arm and row['seed'] == seed
                    and row['round'] == rnd):
                return row.get(field)
        return None

    out = {'perArm': {}, 'pairedRatios': {}, 'quality': {}}
    for arm in arms:
        for field in ('sliceMs', 'processWallSeconds'):
            out['perArm'].setdefault(arm, {})[field] = spread(
                [r[field] for r in obs if r['arm'] == arm and field in r])
        # Every arm must have walked the same number of steps; if it did not,
        # this is not an equal-walk comparison and the ratios below are void.
        out['perArm'][arm]['stepsTaken'] = sorted(
            {r['stepsTaken'] for r in obs if r['arm'] == arm})
        out['perArm'][arm]['depths'] = sorted(
            {r['rawSourceDepthMm'] for r in obs if r['arm'] == arm})
    control = arms[0]
    for arm in arms[1:]:
        for field in ('sliceMs', 'processWallSeconds'):
            ratios = []
            for rnd in range(result['rounds']):
                for seed in seeds:
                    a, b = pick(arm, seed, rnd, field), pick(control, seed, rnd,
                                                             field)
                    if a and b:
                        ratios.append(b / a)
            out['pairedRatios'].setdefault(f'{control}-over-{arm}', {})[field] = {
                'n': len(ratios),
                'medianSpeedup': statistics.median(ratios) if ratios else None,
                'min': min(ratios, default=None),
                'max': max(ratios, default=None),
                'roundsAboveParity': sum(1 for r in ratios if r > 1.0),
            }
        deltas = []
        for rnd in range(result['rounds']):
            for seed in seeds:
                a = pick(arm, seed, rnd, 'rawSourceDepthMm')
                b = pick(control, seed, rnd, 'rawSourceDepthMm')
                if a is not None and b is not None:
                    deltas.append(b - a)
        out['quality'][f'{arm}-deeper-than-{control}-mm'] = {
            'n': len(deltas),
            'median': statistics.median(deltas) if deltas else None,
            'wins': sum(1 for d in deltas if d > 1e-12),
            'ties': sum(1 for d in deltas if abs(d) <= 1e-12),
            'losses': sum(1 for d in deltas if d < -1e-12),
        }
    return out


if __name__ == '__main__':
    main()
