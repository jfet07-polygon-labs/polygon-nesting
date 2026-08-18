#!/usr/bin/env python3
"""The paired interleaved v2/v3 arm battery.

Every timing and quality claim in this stage is a per-round paired difference:
each round runs every arm once per seed, back to back, with the arm order
rotating every round, because another agent benchmarks on this box
concurrently and an unpaired number here would be worthless.

    battery.py NAME ROUNDS REQUEST SEEDS ARMSPEC [ARMSPEC ...]

`SEEDS` is comma separated. An `ARMSPEC` is
`label:budgetkey:budgetvalue:v3[:extra]`, e.g. `v3at10:wall:10000:1:sched=0,barren=0,divq=0`
or `v4at30:wall:30000:1`. `budgetkey` may also be `bare`, which runs the engine
with no coordinator at all.
"""
import json
import os
import statistics
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import runlib  # noqa: E402


def main():
    name = sys.argv[1]
    rounds = int(sys.argv[2])
    request = sys.argv[3]
    seeds = [int(value) for value in sys.argv[4].split(',')]
    arms = []
    for entry in sys.argv[5:]:
        parts = entry.split(':')
        label, key, value, v3 = parts[:4]
        extra = parts[4] if len(parts) > 4 else ''
        arms.append((label, key, value, v3 == '1', extra))

    out_dir = f'{runlib.OUT}/{name}'
    result = {'name': name, 'request': request, 'rounds': rounds,
              'seeds': seeds, 'binary': runlib.BIN,
              'arms': [{'label': a, 'budget': f'{k}={v}', 'v3': t,
                        'extra': e}
                       for a, k, v, t, e in arms],
              'rows': []}
    for round_index in range(rounds):
        for seed in seeds:
            ordered = arms[round_index % len(arms):] + \
                arms[:round_index % len(arms)]
            for label, key, value, v3, extra in ordered:
                tag = f'{label}-s{seed}-r{round_index}'
                spec = None if key == 'bare' else \
                    runlib.spec_for(seed, key, value, v3, extra)
                doc, seconds, err = runlib.run(
                    runlib.BIN, request, seed, spec,
                    f'{out_dir}/runs/{tag}.json')
                row = runlib.summarize(tag, doc, seconds)
                row.update({'arm': label, 'seed': seed, 'round': round_index,
                            'spec': spec})
                result['rows'].append(row)
                print(f"{tag}: engine={row['engineDepthMm']} "
                      f"raw={row.get('rawDepthMm')} "
                      f"process={seconds:.2f}s", flush=True)
    os.makedirs(out_dir, exist_ok=True)
    report(result)
    json.dump(result, open(f'{out_dir}/battery.json', 'w'), indent=1)


def report(result):
    rows = result['rows']
    labels = [arm['label'] for arm in result['arms']]
    by_key = {(row['arm'], row['seed'], row['round']): row for row in rows}
    seeds = sorted({row['seed'] for row in rows})
    rounds = sorted({row['round'] for row in rows})
    summary = {'perSeed': {}, 'pairwise': {}, 'processSeconds': {}}
    for seed in seeds:
        summary['perSeed'][str(seed)] = {
            label: sorted(
                by_key[(label, seed, r)]['engineDepthMm']
                for r in rounds if (label, seed, r) in by_key
                and by_key[(label, seed, r)]['engineDepthMm'] is not None)
            for label in labels}
    for left in labels:
        for right in labels:
            if left >= right:
                continue
            deltas = []
            for seed in seeds:
                for r in rounds:
                    a = by_key.get((left, seed, r))
                    b = by_key.get((right, seed, r))
                    if not a or not b:
                        continue
                    if a['engineDepthMm'] is None or b['engineDepthMm'] is None:
                        continue
                    deltas.append(b['engineDepthMm'] - a['engineDepthMm'])
            if deltas:
                summary['pairwise'][f'{right}-minus-{left}'] = {
                    'medianMm': statistics.median(deltas),
                    'minMm': min(deltas), 'maxMm': max(deltas),
                    'roundsRightBetter': sum(1 for d in deltas if d < 0),
                    'roundsLeftBetter': sum(1 for d in deltas if d > 0),
                    'roundsEqual': sum(1 for d in deltas if d == 0),
                    'rounds': len(deltas),
                }
    for label in labels:
        values = [row['processSeconds'] for row in rows
                  if row['arm'] == label]
        summary['processSeconds'][label] = {
            'median': statistics.median(values),
            'min': min(values), 'max': max(values)}
    # Per-class economics for the v3 arms.
    classes = {}
    for row in rows:
        schedule = row.get('schedule')
        if not schedule:
            continue
        for entry in schedule['classes']:
            key = f"{row['arm']}/{entry['class']}"
            agg = classes.setdefault(key, {'actions': 0, 'publications': 0,
                                           'workUnits': 0, 'seconds': 0.0,
                                           'deltaRawMm': 0.0, 'runs': 0})
            agg['actions'] += entry['actions']
            agg['publications'] += entry['publications']
            agg['workUnits'] += entry['workUnits']
            agg['seconds'] += entry['seconds']
            agg['deltaRawMm'] += entry['deltaRawMm']
            agg['runs'] += 1
    summary['v3Classes'] = classes
    summary['errors'] = [{'tag': row['tag'], 'error': row['loadError'][-300:]}
                         for row in rows if 'loadError' in row]
    result['summary'] = summary
    print(json.dumps(summary, indent=1))


if __name__ == '__main__':
    main()
