#!/usr/bin/env python3
"""The first m34 slice's wall price, on every request the coordinator is
measured on.

    firstslice.py NAME REQUESTS SEEDS BUDGETMS [BUDGETMS ...]

One process per cell. For each cell it records the schedule class's
`firstEstimatedCost` and `firstActualCost` - both in the budget's own currency,
which under a wall budget is seconds - the run's own `phaseZeroCost`, and the
resulting multiples. The ratio `firstActual / phaseZero` is the number a wall
prior has to be, and `firstActual / firstEstimated` is how wrong the work-
denominated prior is on the clock.

This is the same construction `coordinator-v4/drivers/diversifyprior.py` used
for the diversify class's two currencies, on the class that needed it next.
"""
import json
import os
import statistics
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import runlib  # noqa: E402


def main():
    name = sys.argv[1]
    requests = sys.argv[2].split(',')
    seeds = [int(v) for v in sys.argv[3].split(',')]
    budgets = [int(v) for v in sys.argv[4:]]
    extra = os.environ.get('M34_EXTRA', '')
    out_dir = f'{runlib.OUT}/{name}'
    result = {'name': name, 'binary': runlib.BIN, 'extra': extra, 'rows': []}
    for request in requests:
        for budget in budgets:
            for seed in seeds:
                tag = f'{request}-s{seed}-w{budget}'
                spec = runlib.spec_for(seed, 'wall', str(budget), True, extra)
                doc, wall, err = runlib.run(
                    runlib.BIN, request, seed, spec,
                    f'{out_dir}/runs/{tag}.json')
                row = {'tag': tag, 'request': request, 'seed': seed,
                       'budgetMs': budget, 'processSeconds': wall,
                       'spec': spec}
                portfolio = doc.get('portfolio')
                if not portfolio:
                    row['loadError'] = doc.get('_loadError', 'no portfolio')
                    result['rows'].append(row)
                    print(f'{tag}: FAILED {row["loadError"][:200]}', flush=True)
                    continue
                schedule = portfolio.get('schedule') or {}
                row['coordinatorSeconds'] = portfolio['elapsedSeconds']
                row['overrunSeconds'] = \
                    portfolio['elapsedSeconds'] - budget / 1000.0
                row['rawDepthMm'] = portfolio['incumbent']['rawDepthMm']
                row['engineDepthMm'] = doc.get(
                    'independentUsedLongAxisDepthMm')
                row['phaseZeroCost'] = schedule.get('phaseZeroCost')
                row['exitCause'] = schedule.get('exitCause')
                row['iterations'] = schedule.get('iterations')
                sched = next((c for c in schedule.get('classes', [])
                              if c['class'] == 'schedule'), None)
                if sched:
                    row['scheduleActions'] = sched['actions']
                    row['schedulePublications'] = sched['publications']
                    row['scheduleSeconds'] = sched['seconds']
                    row['scheduleDeltaRawMm'] = sched['deltaRawMm']
                    row['firstEstimatedCost'] = sched['firstEstimatedCost']
                    row['firstActualCost'] = sched['firstActualCost']
                    if sched['firstActualCost'] and row['phaseZeroCost']:
                        row['firstActualPhaseZeros'] = \
                            sched['firstActualCost'] / row['phaseZeroCost']
                        row['actualOverEstimate'] = \
                            sched['firstActualCost'] / sched[
                                'firstEstimatedCost']
                else:
                    row['scheduleActions'] = 0
                    row['schedulePublications'] = 0
                    row['scheduleSeconds'] = 0.0
                    row['scheduleDeltaRawMm'] = 0.0
                # Every m34 operator call's own slice wall, so a run with more
                # than one is not summarised by its first.
                row['m34Calls'] = [
                    {'seconds': c['elapsedSeconds'],
                     'published': c['published'],
                     'rawDepthMm': c['rawDepthMm'],
                     'scheduleEntry': c.get('scheduleEntry')}
                    for c in portfolio['operatorCalls']
                    if c['operator'] == 'mode34']
                result['rows'].append(row)
                print(f'{tag}: actions={row["scheduleActions"]} '
                      f'pub={row["schedulePublications"]} '
                      f'est={row.get("firstEstimatedCost")} '
                      f'act={row.get("firstActualCost")} '
                      f'phase0={row.get("phaseZeroCost")} '
                      f'a/e={row.get("actualOverEstimate")} '
                      f'a/p0={row.get("firstActualPhaseZeros")}', flush=True)
                if err.strip():
                    print(f'  stderr: {err[-200:]}', flush=True)
    summarize(result)
    os.makedirs(out_dir, exist_ok=True)
    json.dump(result, open(f'{out_dir}/firstslice.json', 'w'), indent=1)
    print(f'wrote {out_dir}/firstslice.json')


def summarize(result):
    per_request = {}
    for row in result['rows']:
        value = row.get('firstActualPhaseZeros')
        if value is None:
            continue
        per_request.setdefault(row['request'], []).append(value)
    summary = {}
    for request, values in per_request.items():
        summary[request] = {
            'n': len(values), 'min': min(values), 'max': max(values),
            'median': statistics.median(values)}
    everything = [v for values in per_request.values() for v in values]
    if everything:
        summary['pooled'] = {
            'n': len(everything), 'min': min(everything),
            'max': max(everything), 'median': statistics.median(everything)}
    result['firstActualPhaseZeros'] = summary
    ratios = [row['actualOverEstimate'] for row in result['rows']
              if row.get('actualOverEstimate')]
    if ratios:
        result['actualOverEstimate'] = {
            'n': len(ratios), 'min': min(ratios), 'max': max(ratios),
            'median': statistics.median(ratios)}
    print(json.dumps(summary, indent=1))


if __name__ == '__main__':
    main()
