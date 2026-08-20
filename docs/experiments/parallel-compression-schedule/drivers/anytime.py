#!/usr/bin/env python3
"""The anytime battery: 3 / 10 / 30 s from the bare request, paired.

    python3 anytime.py OUTDIR BINARY ARMS SECONDS SEEDS ROUNDS

Measurement (d) of Grok's action 2, and the only one denominated in the thing
the binding user priority names: quality at a wall budget from a bare request.
Every arm is the same binary and the same spec except for the two keys that arm
the coordinator's own mode-34 slice, so the comparison is a comparison of the
slice and not of a build.

Paired and interleaved on the same protocol as `wall.py`: one round runs every
arm on every seed before the next round starts, and the arm order reverses on
odd rounds. The statistic is the published raw source depth; the report carries
the per-arm spread next to the paired delta because this box is shared.

Grok's hypothesis, tested here as written: the intra-arm parallel schedule
"brings 10s toward the 40M-work quality (~166), not to 150". The 40M-work
anchor is `docs/experiments/coordinator-v4/` (165.8-171.4) and the 10 s wall
anchor is that round's 173.575 / 171.362 / 176.162.
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

# The shipping v3 coordinator, armed the way the 10 s curve is measured:
# `v3=1` (Grok F1 - a `v3=0` spec never enters the loop mode 34 lives in),
# `sched`/`barren`/`divq` at their shipping defaults.
BASE = 'wall={ms},cells={cells},v3=1'
ARMS = {
    'serial': '',
    'pconfirm': ',m34lanes=1,m34pconfirm=1',
    'lanes8': ',m34lanes=8,m34pconfirm=0',
    'both': ',m34lanes=8,m34pconfirm=1',
}


def run(binary, seed, spec, path):
    args = [a.format(seed=seed) for a in runlib.ARGS]
    tail = ['0', '', '', '', runlib.DEFAULT_ALLOWANCE, spec]
    env = dict(os.environ)
    env.pop('POLYGON_NESTING_PROFILE', None)
    env.pop('POLYGON_NESTING_COMPRESSION_SCHEDULE', None)
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
    # The mode-34 slice is the `schedule` phase's operator call. Counting the
    # calls and their publications is what makes "the arm did or did not buy
    # its millimetres from m34" a reading rather than an inference.
    schedule_calls = [c for c in calls if c.get('phase') == 'schedule']
    return {
        'processWallSeconds': wall,
        'rawDepthMm': incumbent.get('rawDepthMm'),
        'independentDepthMm': doc.get('independentUsedLongAxisDepthMm'),
        'dualGateValid': incumbent.get('dualGateValid'),
        'fingerprint': incumbent.get('fingerprint'),
        'incumbentSource': incumbent.get('source'),
        'workUnits': portfolio.get('workUnits'),
        'engineElapsedSeconds': portfolio.get('elapsedSeconds'),
        'exitCause': (portfolio.get('schedule') or {}).get('exitCause'),
        'operatorCalls': len(calls),
        'publications': len(portfolio.get('publications') or []),
        'scheduleActions': len(schedule_calls),
        'schedulePublications': sum(1 for c in schedule_calls
                                    if c.get('published')),
        'scheduleSeconds': sum(c.get('elapsedSeconds') or 0.0
                               for c in schedule_calls),
        'scheduleWorkUnits': sum(c.get('workUnits') or 0
                                 for c in schedule_calls),
    }


def main():
    outdir, binary = sys.argv[1], sys.argv[2]
    arms = sys.argv[3].split(',')
    seconds = [float(s) for s in sys.argv[4].split(',')]
    seeds = [int(s) for s in sys.argv[5].split(',')]
    rounds = int(sys.argv[6])
    result = {'binary': binary,
              'binarySha256': hashlib.sha256(open(binary, 'rb').read()).hexdigest(),
              'arms': arms, 'seconds': seconds, 'seeds': seeds,
              'rounds': rounds, 'observations': []}
    for rnd in range(rounds):
        order = arms if rnd % 2 == 0 else list(reversed(arms))
        for budget in seconds:
            for seed in seeds:
                for arm in order:
                    spec = (BASE.format(ms=int(budget * 1000),
                                        cells=runlib.SALT_SETS[
                                            seed % len(runlib.SALT_SETS)])
                            + ARMS[arm])
                    row = run(binary, seed, spec,
                              f'{outdir}/r{rnd}-b{budget:g}-s{seed}-{arm}.json')
                    row.update({'round': rnd, 'budgetSeconds': budget,
                                'seed': seed, 'arm': arm, 'spec': spec})
                    result['observations'].append(row)
            json.dump(result, open(f'{outdir}/anytime.json', 'w'), indent=1)
    result['summary'] = summarise(result, arms, seconds, seeds, rounds)
    json.dump(result, open(f'{outdir}/anytime.json', 'w'), indent=1)
    print(json.dumps(result['summary'], indent=1))


def summarise(result, arms, seconds, seeds, rounds):
    obs = result['observations']
    out = {}
    for budget in seconds:
        cell = {}
        for arm in arms:
            depths = [r['rawDepthMm'] for r in obs
                      if r['arm'] == arm and r['budgetSeconds'] == budget
                      and r.get('rawDepthMm') is not None]
            walls = [r['processWallSeconds'] for r in obs
                     if r['arm'] == arm and r['budgetSeconds'] == budget]
            slices = [r['scheduleActions'] for r in obs
                      if r['arm'] == arm and r['budgetSeconds'] == budget]
            slice_seconds = [r['scheduleSeconds'] for r in obs
                             if r['arm'] == arm
                             and r['budgetSeconds'] == budget
                             and r.get('scheduleSeconds') is not None]
            cell[arm] = {
                'n': len(depths),
                'medianRawDepthMm': statistics.median(depths) if depths else None,
                'minRawDepthMm': min(depths, default=None),
                'maxRawDepthMm': max(depths, default=None),
                'withinArmSpreadMm': (max(depths) - min(depths)) if depths else None,
                'medianProcessWallSeconds': statistics.median(walls) if walls else None,
                'medianScheduleActions': statistics.median(slices) if slices else None,
                'medianScheduleSeconds': statistics.median(slice_seconds)
                if slice_seconds else None,
            }
        control = arms[0]
        for arm in arms[1:]:
            paired = []
            for rnd in range(rounds):
                for seed in seeds:
                    def pick(a):
                        for r in obs:
                            if (r['arm'] == a and r['seed'] == seed
                                    and r['round'] == rnd
                                    and r['budgetSeconds'] == budget):
                                return r.get('rawDepthMm')
                        return None
                    a, b = pick(arm), pick(control)
                    if a is not None and b is not None:
                        paired.append(b - a)
            cell[f'{arm}-deeper-than-{control}-mm'] = {
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
