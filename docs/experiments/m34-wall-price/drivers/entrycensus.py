#!/usr/bin/env python3
"""The entry census: what the proxy tier thinks of every m34 parent, before and
after a translation-only repair, and what the slice then bought.

    entrycensus.py NAME ARM REQUESTS SEEDS BUDGETMS [BUDGETMS ...]

`ARM` is the extra portfolio spec, e.g. `m34entry=1` or
`m34entry=1,m34skip=1`. One row per *slice*, not per run, because a run can
take four of them and the fourth is a different measurement from the first.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import runlib  # noqa: E402


def main():
    name = sys.argv[1]
    arm = sys.argv[2]
    requests = sys.argv[3].split(',')
    seeds = [int(v) for v in sys.argv[4].split(',')]
    budgets = [int(v) for v in sys.argv[5:]]
    out_dir = f'{runlib.OUT}/{name}'
    result = {'name': name, 'arm': arm, 'binary': runlib.BIN, 'slices': [],
              'runs': []}
    for request in requests:
        for budget in budgets:
            for seed in seeds:
                tag = f'{request}-s{seed}-w{budget}'
                spec = runlib.spec_for(
                    seed, 'wall', str(budget), True,
                    arm if arm != '-' else '')
                doc, wall, err = runlib.run(
                    runlib.BIN, request, seed, spec,
                    f'{out_dir}/runs/{tag}.json')
                portfolio = doc.get('portfolio')
                if not portfolio:
                    print(f'{tag}: FAILED {doc.get("_loadError", "")[:200]}',
                          flush=True)
                    continue
                run_row = {
                    'tag': tag, 'request': request, 'seed': seed,
                    'budgetMs': budget, 'spec': spec,
                    'processSeconds': wall,
                    'coordinatorSeconds': portfolio['elapsedSeconds'],
                    'overrunSeconds':
                        portfolio['elapsedSeconds'] - budget / 1000.0,
                    'rawDepthMm': portfolio['incumbent']['rawDepthMm'],
                    'engineDepthMm':
                        doc.get('independentUsedLongAxisDepthMm'),
                    'exitCause': (portfolio.get('schedule') or {}).get(
                        'exitCause'),
                }
                result['runs'].append(run_row)
                calls = [c for c in portfolio['operatorCalls']
                         if c['operator'] == 'mode34']
                for index, call in enumerate(calls):
                    slice_report = call.get('scheduleSlice') or {}
                    row = {'tag': tag, 'request': request, 'seed': seed,
                           'budgetMs': budget, 'sliceIndex': index,
                           'callSeconds': call['elapsedSeconds'],
                           'published': call['published'],
                           'rawDepthMm': call['rawDepthMm']}
                    row.update(slice_report)
                    result['slices'].append(row)
                    print(f'{tag} #{index}: {call["elapsedSeconds"]:.3f}s '
                          f'pub={call["published"]} '
                          f'parentFeasible='
                          f'{slice_report.get("parentProxyFeasible")} '
                          f'pairs={slice_report.get("parentCollisionPairs")}'
                          f'->{slice_report.get("entryCollisionPairs")} '
                          f'entryFeasible='
                          f'{slice_report.get("entryProxyFeasible")} '
                          f'legalMs='
                          f'{slice_report.get("entryLegalizationMs", 0):.1f} '
                          f'accepted='
                          f'{slice_report.get("entryLegalizationAccepted")} '
                          f'skipped='
                          f'{slice_report.get("skippedInfeasibleEntry")} '
                          f'steps={slice_report.get("stepsTaken")} '
                          f'confMs='
                          f'{slice_report.get("confirmationMs", 0):.0f} '
                          f'repairMs='
                          f'{slice_report.get("repairMs", 0):.0f}',
                          flush=True)
                if not calls:
                    print(f'{tag}: no m34 calls', flush=True)
                if err.strip():
                    print(f'  stderr: {err[-200:]}', flush=True)
    summarize(result)
    os.makedirs(out_dir, exist_ok=True)
    json.dump(result, open(f'{out_dir}/entrycensus.json', 'w'), indent=1)
    print(f'wrote {out_dir}/entrycensus.json')


def summarize(result):
    summary = {}
    for row in result['slices']:
        bucket = summary.setdefault(row['request'], {
            'slices': 0, 'parentFeasible': 0, 'entryFeasible': 0,
            'legalizationRun': 0, 'legalizationResolved': 0,
            'legalizationAccepted': 0, 'skipped': 0, 'published': 0,
            'legalizationMs': [], 'callSeconds': []})
        bucket['slices'] += 1
        bucket['parentFeasible'] += bool(row.get('parentProxyFeasible'))
        bucket['entryFeasible'] += bool(row.get('entryProxyFeasible'))
        bucket['legalizationRun'] += bool(row.get('entryLegalizationRun'))
        bucket['legalizationResolved'] += \
            bool(row.get('entryLegalizationResolved'))
        bucket['legalizationAccepted'] += \
            bool(row.get('entryLegalizationAccepted'))
        bucket['skipped'] += bool(row.get('skippedInfeasibleEntry'))
        bucket['published'] += bool(row.get('published'))
        bucket['legalizationMs'].append(row.get('entryLegalizationMs', 0.0))
        bucket['callSeconds'].append(row['callSeconds'])
    for bucket in summary.values():
        for key in ('legalizationMs', 'callSeconds'):
            values = sorted(bucket[key])
            bucket[key] = {'min': values[0], 'max': values[-1],
                           'median': values[len(values) // 2]} if values \
                else None
    result['summary'] = summary
    print(json.dumps(summary, indent=1))


if __name__ == '__main__':
    main()
