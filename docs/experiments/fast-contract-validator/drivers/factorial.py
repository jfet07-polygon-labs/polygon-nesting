#!/usr/bin/env python3
"""The 2x2: `fast-contract-validator` on/off crossed with `pconfirm` on/off.

    python3 factorial.py OUTDIR OFF_BINARY ON_BINARY SECONDS SEEDS ROUNDS

Sol review 7 §1 and Grok review 2 §3(c) both land on the same question, and it
is a question about *interaction* rather than about either lever:

  > "dopo il prune la conferma seriale costa ~0.86 ms: ora l'overhead di
  >  `pconfirm` puo' superare il parallelismo utile."

`pconfirm` spreads one confirmation's all-pairs loop over the job pool. That was
worth having when a confirmation cost 4.82 ms. The validator now costs 0.861 ms
serial, and a job-pool dispatch is not free: if its fixed cost is an appreciable
fraction of 0.861 ms, then arming both levers is paying twice for the same
millisecond and `pconfirm` has become a **tax** rather than a saving.

Neither of the previous rounds could see this. `parallel-compression-schedule`
priced `pconfirm` against a 4.82 ms confirmation, before this filter existed;
this round's §3.3 ran `m34pconfirm=1` in *both* arms, so the factorial cell that
would answer it was never run. Four cells, one battery:

    off/off   the shipped default before either lever
    on/off    the validator alone, serial confirmation
    off/on    parallel confirmation alone - the previous round's ship
    on/on     both, which is what §3.3 measured and what promotion would default

Paired and interleaved across all four arms per seed, arm order rotated by round
so no cell always runs first into a cold cache, and the within-arm spread beside
every delta. Three seeds x three rounds; the honest denominator is three seeds,
as §3.3 already says of itself - these runs are near-deterministic at a wall
budget and repeated rounds are a noise estimate, not independent samples.
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

BASE = 'wall={ms},cells={cells},v3=1,m34lanes=1'

# (fcv arm, pconfirm value)
CELLS = [
    ('off', 0),
    ('on', 0),
    ('off', 1),
    ('on', 1),
]


def cell_name(fcv, pconfirm):
    return f'fcv{fcv}-pconfirm{pconfirm}'


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

    # The per-confirmation microbenchmark, taken from the same runs rather than
    # from a separate battery: every schedule slice reports its own
    # `confirmationMs` and `confirmationsAccepted`, so summing both over the run
    # gives the cost of one accepted confirmation in the regime the depth was
    # actually produced in. This is the number the tax question turns on.
    confirmation_ms = 0.0
    confirmations = 0
    repair_ms = 0.0
    for call in schedule_calls:
        slice_row = call.get('scheduleSlice') or {}
        confirmation_ms += slice_row.get('confirmationMs') or 0.0
        confirmations += slice_row.get('confirmationsAccepted') or 0
        repair_ms += slice_row.get('repairMs') or 0.0
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
        'confirmationMs': confirmation_ms,
        'confirmationsAccepted': confirmations,
        'perConfirmationMs': (confirmation_ms / confirmations
                              if confirmations else None),
        'repairMs': repair_ms,
    }


def main():
    outdir, off_binary, on_binary = sys.argv[1], sys.argv[2], sys.argv[3]
    seconds = [float(s) for s in sys.argv[4].split(',')]
    seeds = [int(s) for s in sys.argv[5].split(',')]
    rounds = int(sys.argv[6])
    binaries = {'off': off_binary, 'on': on_binary}
    result = {
        'binaries': binaries,
        'binarySha256': {k: hashlib.sha256(open(v, 'rb').read()).hexdigest()
                         for k, v in binaries.items()},
        'seconds': seconds, 'seeds': seeds, 'rounds': rounds,
        'cells': [cell_name(f, p) for f, p in CELLS],
        'protocol': 'paired interleaved 2x2; cell order rotated by round; '
                    'identical spec except m34pconfirm; binaries differ only '
                    'in fast-contract-validator',
        'observations': [],
    }
    os.makedirs(outdir, exist_ok=True)
    for rnd in range(rounds):
        # Rotate rather than reverse: with four cells a reversal only ever
        # exercises two orderings.
        order = CELLS[rnd % len(CELLS):] + CELLS[:rnd % len(CELLS)]
        for budget in seconds:
            for seed in seeds:
                for fcv, pconfirm in order:
                    spec = (BASE.format(
                        ms=int(budget * 1000),
                        cells=runlib.SALT_SETS[seed % len(runlib.SALT_SETS)])
                        + f',m34pconfirm={pconfirm}')
                    name = cell_name(fcv, pconfirm)
                    row = run(binaries[fcv], seed, spec,
                              f'{outdir}/r{rnd}-b{budget:g}-s{seed}-{name}.json')
                    row.update({'round': rnd, 'budgetSeconds': budget,
                                'seed': seed, 'cell': name, 'fcv': fcv,
                                'pconfirm': pconfirm, 'spec': spec})
                    result['observations'].append(row)
            json.dump(result, open(f'{outdir}/factorial.json', 'w'), indent=1)
        print(f'round {rnd} done', file=sys.stderr)
    result['summary'] = summarise(result, seconds, seeds, rounds)
    json.dump(result, open(f'{outdir}/factorial.json', 'w'), indent=1)
    print(json.dumps(result['summary'], indent=1))


def summarise(result, seconds, seeds, rounds):
    obs = result['observations']
    out = {}
    for budget in seconds:
        cell_stats = {}
        for fcv, pconfirm in CELLS:
            name = cell_name(fcv, pconfirm)
            rows = [r for r in obs
                    if r['cell'] == name and r['budgetSeconds'] == budget]
            depths = [r['rawDepthMm'] for r in rows
                      if r.get('rawDepthMm') is not None]
            per_conf = [r['perConfirmationMs'] for r in rows
                        if r.get('perConfirmationMs')]
            cell_stats[name] = {
                'n': len(depths),
                'medianRawDepthMm': statistics.median(depths) if depths else None,
                'minRawDepthMm': min(depths, default=None),
                'maxRawDepthMm': max(depths, default=None),
                'withinArmSpreadMm': (max(depths) - min(depths))
                if depths else None,
                'medianPerConfirmationMs': (statistics.median(per_conf)
                                            if per_conf else None),
                'totalConfirmations': sum(r.get('confirmationsAccepted') or 0
                                          for r in rows),
                'medianScheduleActions': statistics.median(
                    [r['scheduleActions'] for r in rows]) if rows else None,
                'totalSchedulePublications': sum(r['schedulePublications']
                                                 for r in rows),
                'medianProcessWallSeconds': statistics.median(
                    [r['processWallSeconds'] for r in rows]) if rows else None,
            }

        def pick(name, seed, rnd, field):
            for r in obs:
                if (r['cell'] == name and r['seed'] == seed
                        and r['round'] == rnd and r['budgetSeconds'] == budget):
                    return r.get(field)
            return None

        # Every ordered pair of cells, paired per (seed, round). Depth is
        # smaller-is-better, so `delta` is `a - b` and positive means `b` won.
        contrasts = {}
        names = [cell_name(f, p) for f, p in CELLS]
        for first in names:
            for second in names:
                if first >= second:
                    continue
                paired = []
                for rnd in range(rounds):
                    for seed in seeds:
                        a = pick(first, seed, rnd, 'rawDepthMm')
                        b = pick(second, seed, rnd, 'rawDepthMm')
                        if a is not None and b is not None:
                            paired.append(a - b)
                contrasts[f'{first}_minus_{second}'] = {
                    'n': len(paired),
                    'medianMm': statistics.median(paired) if paired else None,
                    'secondWins': sum(1 for d in paired if d > 1e-9),
                    'ties': sum(1 for d in paired if abs(d) <= 1e-9),
                    'firstWins': sum(1 for d in paired if d < -1e-9),
                    'perPair': paired,
                }
        out[f'{budget:g}s'] = {'cells': cell_stats, 'contrasts': contrasts}
    return out


if __name__ == '__main__':
    main()
