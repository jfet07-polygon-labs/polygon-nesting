#!/usr/bin/env python3
"""The paired interleaved per-confirmation wall A/B, flag-off against flag-on.

    python3 wall.py OUTDIR OFF_BINARY ON_BINARY SEEDS ROUNDS [SPEC] [DROP_MM]

The unit under test is **one accepted confirmation**, so the statistic is
`confirmationMs / confirmationsAccepted` - the schedule's own clock around the
exact validator, divided by the number of times it ran. That is the quantity
docs/experiments/parallel-compression-schedule/ §3 measured at 5.028 ms serial
and 1.091 ms under `pconfirm=1`, and it is the one this feature attacks.

Protocol, per the campaign's wall rules and identical in shape to
parallel-compression-schedule/drivers/wall.py:

  * paired and interleaved - one round runs both binaries on every parent
    before the next round starts, so a slow minute on a shared box lands on
    both arms;
  * arm order reverses every round, so neither binary always runs first into a
    cold page cache;
  * the report carries the **within-arm spread** next to the between-arm
    delta, because this box is shared with another measurement agent and a
    delta smaller than the spread is not a result.

Equal walk: `past=0`, no work cap, a fixed drop, so both arms take the same
number of depth steps and confirm the same layouts. `stepsTaken` and
`rawSourceDepthMm` are reported per arm and MUST agree - if they do not, the
arms did different work and the ratio is void, which the summary says out loud.
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

PARENTS = '/var/lib/t3/tmp/csched'
DEFAULT_SPEC = 'past=0,rollback=0,lanes=1,pconfirm=0'
DEFAULT_DROP_MM = 1.5


def parents():
    rows = {}
    for manifest in (f'{PARENTS}/parents/parents.json',
                     f'{PARENTS}/parents-rest/parents.json'):
        if not os.path.exists(manifest):
            continue
        for row in json.load(open(manifest))['rows']:
            if 'fixture' in row:
                rows[row['seed']] = row
    return rows


def run_arm(binary, seed, fixture, target, spec, out_path):
    args = [a.format(seed=seed) for a in runlib.ARGS]
    tail = ['34', fixture, f'{target:.17g}', '', runlib.DEFAULT_ALLOWANCE]
    command = [binary, runlib.REQUESTS['mixed-61']] + args + tail
    env = dict(os.environ)
    env.pop('POLYGON_NESTING_PROFILE', None)
    # The census is an O(n^2) pass of its own; a wall run never enables it.
    env.pop('POLYGON_NESTING_CONTRACT_VALIDATOR_CENSUS', None)
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
        return {'error': (proc.stderr or b'').decode()[-400:],
                'processWallSeconds': wall}
    pop = ((doc.get('relaxedDiagnostics') or {})
           .get('coupledDynamicSeparator') or {}).get(
               'persistentVacancyPopulation') or {}
    schedule = pop.get('compressionSchedule') or {}
    accepted = schedule.get('confirmationsAccepted') or 0
    confirmation_ms = schedule.get('confirmationMs')
    return {
        'processWallSeconds': wall,
        'confirmationMs': confirmation_ms,
        'confirmationsAccepted': accepted,
        'confirmationsAttempted': schedule.get('confirmationsAttempted'),
        'perConfirmationMs': (confirmation_ms / accepted
                              if confirmation_ms is not None and accepted
                              else None),
        'repairMs': schedule.get('repairMs'),
        'sliceMs': (schedule.get('repairMs') or 0) + (confirmation_ms or 0),
        'stepsTaken': schedule.get('stepsTaken'),
        'workUnits': schedule.get('workUnits'),
        'candidateQueries': schedule.get('candidateQueries'),
        'rawSourceDepthMm': pop.get('rawSourceDepthMm'),
        'fingerprint': pop.get('finalPlacementFingerprint'),
    }


def spread(values):
    values = [v for v in values if v is not None]
    if not values:
        return None
    median = statistics.median(values)
    return {'n': len(values), 'median': median, 'min': min(values),
            'max': max(values),
            'relSpread': (max(values) - min(values)) / median if median else None}


def main():
    outdir, off_binary, on_binary = sys.argv[1], sys.argv[2], sys.argv[3]
    seeds = [int(s) for s in sys.argv[4].split(',')]
    rounds = int(sys.argv[5])
    spec = sys.argv[6] if len(sys.argv) > 6 else DEFAULT_SPEC
    drop_mm = float(sys.argv[7]) if len(sys.argv) > 7 else DEFAULT_DROP_MM
    rows = parents()
    arms = {'off': off_binary, 'on': on_binary}
    result = {
        'arms': arms,
        'armSha256': {k: hashlib.sha256(open(v, 'rb').read()).hexdigest()
                      for k, v in arms.items()},
        'spec': spec, 'rounds': rounds, 'seeds': seeds, 'dropMm': drop_mm,
        'protocol': 'paired interleaved; arm order reversed on odd rounds; '
                    'equal walk (past=0, no work cap); census disabled',
        'observations': [],
    }
    os.makedirs(outdir, exist_ok=True)
    for rnd in range(rounds):
        order = ['off', 'on'] if rnd % 2 == 0 else ['on', 'off']
        for seed in seeds:
            parent = rows[seed]
            target = parent['rawDepthMm'] - drop_mm
            for arm in order:
                row = run_arm(arms[arm], seed, parent['fixture'], target, spec,
                              f'{outdir}/r{rnd}-s{seed}-{arm}.json')
                row.update({'round': rnd, 'seed': seed, 'arm': arm})
                result['observations'].append(row)
        json.dump(result, open(f'{outdir}/wall.json', 'w'), indent=1)
        print(f'round {rnd} done', file=sys.stderr)
    result['summary'] = summarise(result, seeds, rounds)
    json.dump(result, open(f'{outdir}/wall.json', 'w'), indent=1)
    print(json.dumps(result['summary'], indent=1))


def summarise(result, seeds, rounds):
    obs = result['observations']

    def pick(arm, seed, rnd, field):
        for row in obs:
            if row['arm'] == arm and row['seed'] == seed and row['round'] == rnd:
                return row.get(field)
        return None

    out = {'perArm': {}, 'paired': {}, 'equalWalk': {}}
    for arm in ('off', 'on'):
        for field in ('perConfirmationMs', 'confirmationMs', 'sliceMs',
                      'processWallSeconds'):
            out['perArm'].setdefault(arm, {})[field] = spread(
                [r.get(field) for r in obs if r['arm'] == arm])
    # Equal-walk integrity: same steps, same confirmations, same depth, same
    # fingerprint. If any of these differ the arms did different work.
    for field in ('stepsTaken', 'confirmationsAccepted', 'rawSourceDepthMm',
                  'fingerprint', 'candidateQueries', 'workUnits'):
        mismatches = []
        for rnd in range(rounds):
            for seed in seeds:
                a, b = pick('off', seed, rnd, field), pick('on', seed, rnd, field)
                if a != b:
                    mismatches.append({'round': rnd, 'seed': seed,
                                       'off': a, 'on': b})
        out['equalWalk'][field] = {'mismatches': len(mismatches),
                                   'examples': mismatches[:4]}
    out['equalWalkHolds'] = all(v['mismatches'] == 0
                                for v in out['equalWalk'].values())
    for field in ('perConfirmationMs', 'confirmationMs', 'sliceMs',
                  'processWallSeconds'):
        ratios = []
        for rnd in range(rounds):
            for seed in seeds:
                a, b = pick('on', seed, rnd, field), pick('off', seed, rnd, field)
                if a and b:
                    ratios.append(b / a)
        out['paired'][field] = {
            'n': len(ratios),
            'medianSpeedup': statistics.median(ratios) if ratios else None,
            'min': min(ratios, default=None), 'max': max(ratios, default=None),
            'cellsAboveParity': sum(1 for r in ratios if r > 1.0),
        }
    return out


if __name__ == '__main__':
    main()
