#!/usr/bin/env python3
"""The paired interleaved arm battery.

Every timing and quality claim in this stage is a per-round paired difference:
each round runs every arm once per seed, back to back, with the arm order
rotating every round, because another agent benchmarks on this box
concurrently and an unpaired number here would be worthless.

Usage:
    battery.py NAME ROUNDS REQUEST SEEDS ARMSPEC [ARMSPEC ...]

`SEEDS` is comma separated. An `ARMSPEC` is `label:binary:spec`, where
`binary` is `v1` or `v2` or an absolute path, and `spec` is the portfolio spec
with `{cells}` and `{wall}` substituted per seed/budget, or the empty string
for the bare engine baseline.
"""
import json
import os
import statistics
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib  # noqa: E402

BINARIES = {'v1': lib.V1_BIN, 'v2': lib.V2_BIN}


def main():
    name = sys.argv[1]
    rounds = int(sys.argv[2])
    request = sys.argv[3]
    seeds = [int(value) for value in sys.argv[4].split(',')]
    arms = []
    for entry in sys.argv[5:]:
        label, binary, spec = entry.split(':', 2)
        arms.append((label, BINARIES.get(binary, binary), spec))

    out_dir = f'{lib.OUT}/{name}'
    result = {'name': name, 'request': request, 'rounds': rounds,
              'seeds': seeds,
              'arms': [{'label': a, 'binary': b, 'spec': c} for a, b, c in arms],
              'rows': []}
    for round_index in range(rounds):
        for seed in seeds:
            cells = lib.SALT_SETS[seed % len(lib.SALT_SETS)]
            ordered = arms[round_index % len(arms):] + \
                arms[:round_index % len(arms)]
            for label, binary, spec in ordered:
                tag = f'{label}-s{seed}-r{round_index}'
                resolved = spec.format(cells=cells) if spec else None
                trace = f'{out_dir}/traces/{tag}.jsonl'
                doc, seconds, _ = lib.run(
                    binary, request, seed, resolved,
                    f'{out_dir}/runs/{tag}.json', trace_path=trace)
                row = lib.summarize(tag, doc, seconds, trace)
                row.update({'arm': label, 'seed': seed, 'round': round_index})
                result['rows'].append(row)
                print(f"{tag}: engine={row['engineDepthMm']} "
                      f"raw={row.get('rawDepthMm')} "
                      f"process={seconds:.2f}s "
                      f"stalled={row.get('descentStalled')}", flush=True)
    os.makedirs(out_dir, exist_ok=True)
    json.dump(result, open(f'{out_dir}/battery.json', 'w'), indent=1)
    report(result)
    json.dump(result, open(f'{out_dir}/battery.json', 'w'), indent=1)


def report(result):
    rows = result['rows']
    labels = [arm['label'] for arm in result['arms']]
    by_key = {(row['arm'], row['seed'], row['round']): row for row in rows}
    seeds = sorted({row['seed'] for row in rows})
    rounds = sorted({row['round'] for row in rows})
    control = labels[0]
    summary = {'control': control, 'perSeed': {}, 'paired': {}}
    for seed in seeds:
        summary['perSeed'][str(seed)] = {
            label: sorted(
                by_key[(label, seed, r)]['engineDepthMm']
                for r in rounds if (label, seed, r) in by_key)
            for label in labels}
    for label in labels[1:]:
        deltas = []
        for seed in seeds:
            for r in rounds:
                base = by_key.get((control, seed, r))
                arm = by_key.get((label, seed, r))
                if not base or not arm:
                    continue
                if base['engineDepthMm'] is None or arm['engineDepthMm'] is None:
                    continue
                deltas.append(arm['engineDepthMm'] - base['engineDepthMm'])
        if deltas:
            summary['paired'][label] = {
                'medianMm': statistics.median(deltas),
                'minMm': min(deltas), 'maxMm': max(deltas),
                'roundsBetter': sum(1 for d in deltas if d < 0),
                'roundsWorse': sum(1 for d in deltas if d > 0),
                'roundsEqual': sum(1 for d in deltas if d == 0),
                'rounds': len(deltas),
            }
    # Every ordered arm pair, because "v2 against the v1 champion" and "v2
    # against the bare engine" are different questions and both are asked.
    summary['pairwise'] = {}
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

    # Per-operator economics, pooled per arm.
    cost = {}
    for row in rows:
        for call in row.get('operatorCalls', []):
            key = f"{row['arm']}/{call['phase']}/{call['operator']}"
            entry = cost.setdefault(key, {'calls': 0, 'seconds': 0.0,
                                          'published': 0, 'exactValid': 0})
            entry['calls'] += 1
            entry['seconds'] += call['elapsedSeconds']
            entry['published'] += int(call['published'])
            entry['exactValid'] += int(call['exactValid'])
    summary['operatorCost'] = {
        key: {**value, 'meanSeconds': value['seconds'] / value['calls']}
        for key, value in sorted(cost.items())}
    phase_seconds = {}
    for row in rows:
        for phase in row.get('phases', []):
            phase_seconds.setdefault(
                f"{row['arm']}/{phase['name']}", []).append(
                    phase['elapsedSeconds'])
    summary['phaseSeconds'] = {
        key: {'median': statistics.median(v), 'min': min(v), 'max': max(v)}
        for key, v in sorted(phase_seconds.items())}
    summary['processSeconds'] = {
        label: statistics.median([row['processSeconds'] for row in rows
                                  if row['arm'] == label])
        for label in labels}
    summary['errors'] = [
        {'tag': row['tag'], 'error': row['loadError'][-300:]}
        for row in rows if 'loadError' in row]
    result['summary'] = summary
    print(json.dumps(summary, indent=1))


if __name__ == '__main__':
    main()
