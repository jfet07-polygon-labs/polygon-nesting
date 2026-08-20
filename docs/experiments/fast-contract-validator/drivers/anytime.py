#!/usr/bin/env python3
"""The compound battery: does a cheaper confirmation buy depth at a wall budget?

    python3 anytime.py OUTDIR OFF_BINARY ON_BINARY SECONDS SEEDS ROUNDS [ARM]

A faster validator is not a result. The result is what the saved milliseconds
are spent on, and the only denomination the binding user priority accepts is
quality at a wall budget from a bare request. So both arms here run the SAME
shipping coordinator spec - `v3=1` with `m34pconfirm=1`, which is what the 10 s
round shipped - and differ only in whether the binary carries
`fast-contract-validator`. Everything the flag buys has to arrive as depth.

`scheduleActions` and `schedulePublications` are carried per run because the
mechanism is a prediction: cheaper confirmations should let the clock fit MORE
mode-34 slices in the same wall, and if the depth moves without the slice count
moving then the depth came from somewhere else and the story is wrong.

Paired and interleaved on the campaign's protocol: one round runs both arms on
every seed before the next starts, arm order reverses on odd rounds, and the
report carries the within-arm spread beside the paired delta because this box is
shared with another measurement agent.
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

BASE = 'wall={ms},cells={cells},v3=1'
DEFAULT_ARM = ',m34lanes=1,m34pconfirm=1'


def run(binary, seed, spec, path):
    args = [a.format(seed=seed) for a in runlib.ARGS]
    tail = ['0', '', '', '', runlib.DEFAULT_ALLOWANCE, spec]
    env = dict(os.environ)
    env.pop('POLYGON_NESTING_PROFILE', None)
    env.pop('POLYGON_NESTING_COMPRESSION_SCHEDULE', None)
    env.pop('POLYGON_NESTING_CONTRACT_VALIDATOR_CENSUS', None)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    started = time.monotonic()
    with open(path, 'w') as handle:
        proc = subprocess.run([binary, runlib.REQUESTS['mixed-61']] + args + tail,
                              stdout=handle, stderr=subprocess.PIPE,
                              check=False, env=env)
    wall = time.monotonic() - started
    try:
        doc = json.load(open(path))
    except json.JSONDecodeError:
        return {'error': (proc.stderr or b'').decode()[-500:],
                'processWallSeconds': wall}
    portfolio = doc.get('portfolio') or {}
    incumbent = portfolio.get('incumbent') or {}
    calls = portfolio.get('operatorCalls') or []
    schedule_calls = [c for c in calls if c.get('phase') == 'schedule']
    return {
        'processWallSeconds': wall,
        'rawDepthMm': incumbent.get('rawDepthMm'),
        'independentDepthMm': doc.get('independentUsedLongAxisDepthMm'),
        'dualGateValid': incumbent.get('dualGateValid'),
        'fingerprint': incumbent.get('fingerprint'),
        'workUnits': portfolio.get('workUnits'),
        'engineElapsedSeconds': portfolio.get('elapsedSeconds'),
        'operatorCalls': len(calls),
        'publications': len(portfolio.get('publications') or []),
        'scheduleActions': len(schedule_calls),
        'schedulePublications': sum(1 for c in schedule_calls
                                    if c.get('published')),
        'scheduleSeconds': sum(c.get('elapsedSeconds') or 0.0
                               for c in schedule_calls),
    }


def main():
    outdir, off_binary, on_binary = sys.argv[1], sys.argv[2], sys.argv[3]
    seconds = [float(s) for s in sys.argv[4].split(',')]
    seeds = [int(s) for s in sys.argv[5].split(',')]
    rounds = int(sys.argv[6])
    arm_spec = sys.argv[7] if len(sys.argv) > 7 else DEFAULT_ARM
    arms = {'off': off_binary, 'on': on_binary}
    result = {'arms': arms,
              'armSha256': {k: hashlib.sha256(open(v, 'rb').read()).hexdigest()
                            for k, v in arms.items()},
              'armSpec': arm_spec, 'seconds': seconds, 'seeds': seeds,
              'rounds': rounds,
              'protocol': 'paired interleaved; arm order reversed on odd '
                          'rounds; identical spec, binaries differ only in '
                          'fast-contract-validator',
              'observations': []}
    os.makedirs(outdir, exist_ok=True)
    for rnd in range(rounds):
        order = ['off', 'on'] if rnd % 2 == 0 else ['on', 'off']
        for budget in seconds:
            for seed in seeds:
                for arm in order:
                    spec = (BASE.format(
                        ms=int(budget * 1000),
                        cells=runlib.SALT_SETS[seed % len(runlib.SALT_SETS)])
                        + arm_spec)
                    row = run(arms[arm], seed, spec,
                              f'{outdir}/r{rnd}-b{budget:g}-s{seed}-{arm}.json')
                    row.update({'round': rnd, 'budgetSeconds': budget,
                                'seed': seed, 'arm': arm, 'spec': spec})
                    result['observations'].append(row)
            json.dump(result, open(f'{outdir}/anytime.json', 'w'), indent=1)
        print(f'round {rnd} done', file=sys.stderr)
    result['summary'] = summarise(result, seconds, seeds, rounds)
    json.dump(result, open(f'{outdir}/anytime.json', 'w'), indent=1)
    print(json.dumps(result['summary'], indent=1))


def summarise(result, seconds, seeds, rounds):
    obs = result['observations']
    out = {}
    for budget in seconds:
        cell = {}
        for arm in ('off', 'on'):
            rows = [r for r in obs
                    if r['arm'] == arm and r['budgetSeconds'] == budget]
            depths = [r['rawDepthMm'] for r in rows
                      if r.get('rawDepthMm') is not None]
            cell[arm] = {
                'n': len(depths),
                'medianRawDepthMm': statistics.median(depths) if depths else None,
                'minRawDepthMm': min(depths, default=None),
                'maxRawDepthMm': max(depths, default=None),
                'withinArmSpreadMm': (max(depths) - min(depths))
                if depths else None,
                'medianProcessWallSeconds': statistics.median(
                    [r['processWallSeconds'] for r in rows]) if rows else None,
                'medianScheduleActions': statistics.median(
                    [r['scheduleActions'] for r in rows]) if rows else None,
                'totalScheduleActions': sum(r['scheduleActions'] for r in rows),
                'totalSchedulePublications': sum(r['schedulePublications']
                                                 for r in rows),
                'medianWorkUnits': statistics.median(
                    [r['workUnits'] for r in rows
                     if r.get('workUnits') is not None]) if rows else None,
            }

        def pick(arm, seed, rnd, field):
            for r in obs:
                if (r['arm'] == arm and r['seed'] == seed and r['round'] == rnd
                        and r['budgetSeconds'] == budget):
                    return r.get(field)
            return None

        for field in ('rawDepthMm', 'scheduleActions'):
            paired = []
            for rnd in range(rounds):
                for seed in seeds:
                    a, b = pick('on', seed, rnd, field), pick('off', seed, rnd,
                                                              field)
                    if a is not None and b is not None:
                        paired.append(b - a if field == 'rawDepthMm' else a - b)
            cell[f'on-better-than-off-{field}'] = {
                'n': len(paired),
                'median': statistics.median(paired) if paired else None,
                'mean': statistics.fmean(paired) if paired else None,
                'wins': sum(1 for d in paired if d > 1e-9),
                'ties': sum(1 for d in paired if abs(d) <= 1e-9),
                'losses': sum(1 for d in paired if d < -1e-9),
                'perPair': paired,
            }
        out[f'{budget:g}s'] = cell
    return out


if __name__ == '__main__':
    main()
