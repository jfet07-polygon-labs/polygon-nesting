#!/usr/bin/env python3
"""The re-baseline: three fixtures, three budgets, two arms, two processes.

    python3 anytime.py OUTDIR BINARY REQUESTS SEEDS TARGETS_MS [ARMS]

The campaign's production number. Each cell is run **twice, in two processes**,
because the plan arm's whole claim is that it reproduces and a claim measured
once is not measured.

    plan    `plan=<ms>`  - the shipping mode: a wall target, spent as work
    wall    `wall=<ms>`  - the incumbent, and the arm every previous round's
                           millimetre in this campaign was measured on

The two arms are not the same measurement and the table must not be read as if
they were:

  * the `wall` arm gets the *whole* target as useful search and is not
    reproducible - `sparse-rotation` §7.2 measures the same unchanged arm
    2-5 mm apart between sessions;
  * the `plan` arm spends part of the target on the work counters, part on the
    quantisation floor, and part on whatever the shipped bias over-charged,
    and *is* reproducible.

The difference between the two columns is therefore the price of
reproducibility, in millimetres, at this budget on this box - which is the
number Sol review 5 §5 asks for and nothing in this repository had.
"""
import json
import os
import statistics
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import planbattery  # noqa: E402
import runlib  # noqa: E402

ARM_SPECS = {'plan': ('plan', ''), 'wall': ('wall', '')}


def main():
    outdir, binary = sys.argv[1:3]
    requests = sys.argv[3].split(',')
    seeds = [int(v) for v in sys.argv[4].split(',')]
    targets = sys.argv[5].split(',')
    arms = sys.argv[6].split(',') if len(sys.argv) > 6 else ['plan', 'wall']
    os.makedirs(outdir, exist_ok=True)
    rows = []
    for target in targets:
        for request in requests:
            for seed in seeds:
                for arm in arms:
                    key, extra = ARM_SPECS[arm]
                    spec = runlib.spec_for(seed, key, target, True, extra)
                    cell = {'request': request, 'seed': seed, 'arm': arm,
                            'targetMs': int(target), 'spec': spec,
                            'processes': []}
                    for process in ('a', 'b'):
                        tag = f'{request}-{target}-{arm}-s{seed}-{process}'
                        doc, wall, err = runlib.run(binary, request, seed, spec,
                                                    f'{outdir}/{tag}.json')
                        portfolio = doc.get('portfolio') or {}
                        cell['processes'].append({
                            'process': process,
                            'processWallSeconds': wall,
                            'rawDepthMm': (portfolio.get('incumbent') or {})
                            .get('rawDepthMm'),
                            'dualGateValid': (portfolio.get('incumbent') or {})
                            .get('dualGateValid'),
                            'planUnits': (portfolio.get('plan') or {})
                            .get('units'),
                            'digest': (planbattery.digest(doc) if portfolio
                                       else None),
                            'error': None if portfolio else err[-300:],
                        })
                    first, second = cell['processes']
                    cell['reproduced'] = (
                        first['digest'] == second['digest']
                        and first['digest'] is not None)
                    cell['depthsEqual'] = (
                        first['rawDepthMm'] == second['rawDepthMm'])
                    cell['rawDepthMm'] = first['rawDepthMm']
                    cell['wallMaxSeconds'] = max(
                        p['processWallSeconds'] for p in cell['processes'])
                    rows.append(cell)
                    print(f'{request} {target}ms {arm} s{seed}: '
                          f'depth={cell["rawDepthMm"]} '
                          f'reproduced={cell["reproduced"]} '
                          f'wallMax={cell["wallMaxSeconds"]:.2f}', flush=True)

    summary = {'binary': binary, 'requests': requests, 'seeds': seeds,
               'targets': targets, 'arms': arms, 'rows': rows, 'table': {}}
    for request in requests:
        for target in targets:
            for arm in arms:
                cell = [r for r in rows
                        if r['request'] == request
                        and str(r['targetMs']) == target and r['arm'] == arm
                        and r['rawDepthMm'] is not None]
                if not cell:
                    continue
                summary['table'][f'{request}|{target}|{arm}'] = {
                    'n': len(cell),
                    'medianDepthMm': statistics.median(
                        r['rawDepthMm'] for r in cell),
                    'perSeedDepthMm': {str(r['seed']): r['rawDepthMm']
                                       for r in cell},
                    'reproducedCells': sum(1 for r in cell if r['reproduced']),
                    'wallMaxSeconds': max(r['wallMaxSeconds'] for r in cell),
                    'wallMedianSeconds': statistics.median(
                        r['wallMaxSeconds'] for r in cell),
                    'overrunCells': sum(
                        1 for r in cell
                        if r['wallMaxSeconds'] > int(target) / 1000.0),
                }
    summary['allPlanCellsReproduced'] = all(
        r['reproduced'] for r in rows if r['arm'] == 'plan')
    summary['allWallCellsReproduced'] = all(
        r['reproduced'] for r in rows if r['arm'] == 'wall')
    json.dump(summary, open(f'{outdir}/anytime.json', 'w'), indent=1)
    print(json.dumps(summary['table'], indent=1))
    print(f'allPlanCellsReproduced={summary["allPlanCellsReproduced"]}')
    print(f'allWallCellsReproduced={summary["allWallCellsReproduced"]}')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
