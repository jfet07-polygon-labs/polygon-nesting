#!/usr/bin/env python3
"""The paired interleaved arm battery.

A diffable copy of `docs/experiments/coordinator-v4/drivers/battery.py` with two
additions and nothing else changed: it records the coordinator's own overrun
against its budget, and it aggregates the mode-34 slice telemetry this round
adds (slice count, first-slice wall, probe aborts, entry feasibility).

Every timing and quality claim in this stage is a per-round paired difference:
each round runs every arm once per seed, back to back, with the arm order
rotating every round, because another agent benchmarks on this box concurrently
and an unpaired number here would be worthless.

    battery.py NAME ROUNDS REQUEST SEEDS ARMSPEC [ARMSPEC ...]

`SEEDS` is comma separated. An `ARMSPEC` is
`label:budgetkey:budgetvalue:v3[:extra]`.
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
                if key == 'wall' and row.get('coordinatorSeconds') is not None:
                    row['budgetSeconds'] = int(value) / 1000.0
                    row['overrunSeconds'] = \
                        row['coordinatorSeconds'] - row['budgetSeconds']
                row['m34'] = m34_rows(doc)
                result['rows'].append(row)
                print(f"{tag}: engine={row['engineDepthMm']} "
                      f"raw={row.get('rawDepthMm')} "
                      f"coord={row.get('coordinatorSeconds')} "
                      f"m34={len(row['m34'])} "
                      f"process={seconds:.2f}s", flush=True)
    os.makedirs(out_dir, exist_ok=True)
    report(result)
    json.dump(result, open(f'{out_dir}/battery.json', 'w'), indent=1)


def m34_rows(doc):
    portfolio = doc.get('portfolio')
    if not portfolio:
        return []
    rows = []
    for call in portfolio['operatorCalls']:
        if call['operator'] != 'mode34':
            continue
        slice_report = call.get('scheduleSlice') or {}
        rows.append({
            'seconds': call['elapsedSeconds'],
            'published': call['published'],
            'abortedBarrenProbe': slice_report.get('abortedBarrenProbe'),
            'skippedInfeasibleEntry':
                slice_report.get('skippedInfeasibleEntry'),
            'parentProxyFeasible': slice_report.get('parentProxyFeasible'),
            'entryProxyFeasible': slice_report.get('entryProxyFeasible'),
            'entryDepthLossMm': slice_report.get('entryDepthLossMm'),
            'requestedDropMm': slice_report.get('requestedDropMm'),
            'stepsTaken': slice_report.get('stepsTaken'),
            'stepsPlanned': slice_report.get('stepsPlanned'),
            'confirmationMs': slice_report.get('confirmationMs'),
            'repairMs': slice_report.get('repairMs'),
        })
    return rows


def report(result):
    rows = result['rows']
    labels = [arm['label'] for arm in result['arms']]
    by_key = {(row['arm'], row['seed'], row['round']): row for row in rows}
    seeds = sorted({row['seed'] for row in rows})
    rounds = sorted({row['round'] for row in rows})
    summary = {'perSeed': {}, 'pairwise': {}, 'processSeconds': {},
               'coordinatorSeconds': {}, 'overrun': {}, 'm34': {}}
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
        values = [row['processSeconds'] for row in rows if row['arm'] == label]
        summary['processSeconds'][label] = {
            'median': statistics.median(values),
            'min': min(values), 'max': max(values)}
        coordinator = [row['coordinatorSeconds'] for row in rows
                       if row['arm'] == label
                       and row.get('coordinatorSeconds') is not None]
        if coordinator:
            summary['coordinatorSeconds'][label] = {
                'median': statistics.median(coordinator),
                'min': min(coordinator), 'max': max(coordinator)}
        overruns = [row['overrunSeconds'] for row in rows
                    if row['arm'] == label
                    and row.get('overrunSeconds') is not None]
        if overruns:
            summary['overrun'][label] = {
                'runs': len(overruns),
                'over': sum(1 for value in overruns if value > 0),
                'maxSeconds': max(overruns),
                'medianSeconds': statistics.median(overruns)}
        slices = [entry for row in rows if row['arm'] == label
                  for entry in row['m34']]
        runs = [row for row in rows if row['arm'] == label]
        if runs:
            firsts = [row['m34'][0]['seconds'] for row in runs if row['m34']]
            summary['m34'][label] = {
                'runs': len(runs),
                'slices': len(slices),
                'slicesPerRun': len(slices) / len(runs),
                'published': sum(1 for s in slices if s['published']),
                'abortedByProbe':
                    sum(1 for s in slices if s['abortedBarrenProbe']),
                'skippedAtEntry':
                    sum(1 for s in slices if s['skippedInfeasibleEntry']),
                'entryFeasible':
                    sum(1 for s in slices if s['entryProxyFeasible']),
                'sliceSecondsTotal': sum(s['seconds'] for s in slices),
                'sliceSecondsPerRun':
                    sum(s['seconds'] for s in slices) / len(runs),
                'firstSliceSeconds': {
                    'n': len(firsts),
                    'min': min(firsts), 'max': max(firsts),
                    'median': statistics.median(firsts)} if firsts else None,
                'sterileSlicesOver1s':
                    sum(1 for s in slices
                        if not s['published'] and s['seconds'] > 1.0),
            }
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
