#!/usr/bin/env python3
"""The coordinator's own overhead on the phase it shares with the baseline.

Two arms, both from the bare request, both doing exactly one thing - the
protected mode-0 search:

  outside  the plain engine run: constructor, relaxed epochs, coupled arms
  inside   the same search run as the coordinator's phase 0, with a zero
           budget so every later phase is entered, found out of room, and
           skipped

The difference between them is the coordinator's fixed cost on the work it did
not add: five archive offers, each of which re-measures a raw depth and re-runs
the composite exact validator, plus the incumbent's own fingerprint and
validation. It is the price of having the two state objects at all.

Ten interleaved rounds, arm order alternating every round, statistic the
per-round paired ratio of the benchmark's own measured stream
(`medianElapsedMs`), which excludes request loading and reporting.

Usage: overhead.py ROUNDS BINARY [SEED]
"""
import json
import os
import statistics
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib  # noqa: E402

OUT = '/var/lib/t3/tmp/pr7/overhead'
DEFAULT_ALLOWANCE = '0.002'


def invoke(binary, tag, seed, spec):
    argv = ([binary, lib.REQ]
            + [a.format(clamp='0', seed=seed) for a in lib.ARGS]
            + ['0', '', '', '', DEFAULT_ALLOWANCE])
    if spec:
        argv += [spec]
    env = dict(os.environ)
    env.pop('POLYGON_NESTING_QUALITY_TRACE', None)
    env.pop('POLYGON_NESTING_PROFILE', None)
    os.makedirs(OUT, exist_ok=True)
    path = f'{OUT}/{tag}.json'
    with open(path, 'w') as handle:
        subprocess.run(argv, stdout=handle, stderr=subprocess.DEVNULL,
                       check=False, env=env)
    return json.load(open(path))


def main():
    rounds, binary = int(sys.argv[1]), sys.argv[2]
    seed = int(sys.argv[3]) if len(sys.argv) > 3 else 1
    result = {'binary': binary, 'seed': seed, 'rounds': rounds, 'rows': []}
    ratios = []
    for round_index in range(rounds):
        arms = [('outside', None), ('inside', 'wall=0')]
        if round_index % 2:
            arms.reverse()
        measured = {}
        for label, spec in arms:
            doc = invoke(binary, f'{label}-r{round_index}', seed, spec)
            measured[label] = {
                'medianElapsedMs': doc['medianElapsedMs'],
                'engineDepthMm': doc.get('independentUsedLongAxisDepthMm'),
                'fingerprint': doc.get('placementFingerprint'),
            }
            portfolio = doc.get('portfolio')
            if portfolio:
                measured[label]['phases'] = [
                    {'name': phase['name'], 'skipped': phase['skipped'],
                     'elapsedSeconds': phase['elapsedSeconds']}
                    for phase in portfolio['phases']
                ]
                measured[label]['archiveOccupancy'] = \
                    portfolio['archive']['occupancy']
        ratio = (measured['inside']['medianElapsedMs']
                 / measured['outside']['medianElapsedMs'])
        ratios.append(ratio)
        result['rows'].append({'round': round_index, 'ratio': ratio,
                               **measured})
        print(f"round {round_index}: outside="
              f"{measured['outside']['medianElapsedMs']:.1f}ms inside="
              f"{measured['inside']['medianElapsedMs']:.1f}ms ratio={ratio:.4f}",
              flush=True)
    depths = {label: sorted({row[label]['engineDepthMm']
                             for row in result['rows']})
              for label in ('outside', 'inside')}
    result['pairedRatio'] = {
        'median': statistics.median(ratios),
        'min': min(ratios),
        'max': max(ratios),
        'roundsBelowParity': sum(1 for value in ratios if value < 1.0),
        'rounds': len(ratios),
    }
    result['engineDepths'] = depths
    result['sameResultBothArms'] = depths['outside'] == depths['inside']
    print(json.dumps(result['pairedRatio'], indent=1))
    print('engine depths:', depths)
    json.dump(result, open(f'{OUT}/overhead.json', 'w'), indent=1)


if __name__ == '__main__':
    main()
