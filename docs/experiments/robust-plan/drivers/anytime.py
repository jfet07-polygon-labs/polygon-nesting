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

ARM_SPECS = {
    'plan': ('plan', ''),
    'wall': ('wall', ''),
    # The previous round's arm. Same wall target, same ladder, same quantised
    # plan - and the part of the target the shipped bias would have left unspent
    # is bought back at the rate this run measured.
    'replan': ('plan', 'replan=1'),
    # This round's. The arm names are the ones `planbattery.py` defines, and
    # they are read from it rather than restated, so the two batteries cannot
    # drift apart on what `calprobe` means.
    'probe': ('plan', 'planprobe=8'),
    'callive': ('plan', 'plancal={live}'),
    'calprobe': ('plan', 'planprobe=8,plancal={probe}'),
    # The winner of the confirmation-density sweep, on top of the winning
    # calibration arm. Substituted at the call site so the sweep's own result is
    # named on the command line rather than frozen in a driver.
    'density': ('plan', 'plancal={live},{density}'),
}


def main():
    outdir, binary = sys.argv[1:3]
    requests = sys.argv[3].split(',')
    seeds = [int(v) for v in sys.argv[4].split(',')]
    targets = sys.argv[5].split(',')
    arms = sys.argv[6].split(',') if len(sys.argv) > 6 else ['plan', 'wall']
    # The confirmation-density winner, as a spec fragment. Empty by default, so
    # the `density` arm degrades to the calibration arm and says so in the spec
    # the table prints rather than silently measuring something else.
    density = os.environ.get('PLAN_DENSITY', '')
    os.makedirs(outdir, exist_ok=True)
    rows = []
    for target in targets:
        for request in requests:
            for seed in seeds:
                for arm in arms:
                    key, extra = ARM_SPECS[arm]
                    extra = extra.format(live=planbattery.CAL_LIVE,
                                         probe=planbattery.CAL_PROBE,
                                         density=density).strip(',')
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
                            'tranches': len(portfolio.get('tranches') or []),
                            'finalUnits': (
                                (portfolio.get('tranches') or [{}])[-1]
                                .get('units')
                                or (portfolio.get('plan') or {}).get('units')),
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
                    cell['dualGateValid'] = all(
                        p['dualGateValid'] for p in cell['processes'])
                    cell['tranches'] = first['tranches']
                    cell['trancheCountsAgree'] = (
                        first['tranches'] == second['tranches'])
                    cell['finalUnitsAgree'] = (
                        first['finalUnits'] == second['finalUnits'])
                    cell['wallMaxSeconds'] = max(
                        p['processWallSeconds'] for p in cell['processes'])
                    rows.append(cell)
                    print(f'{request} {target}ms {arm} s{seed}: '
                          f'depth={cell["rawDepthMm"]} '
                          f'reproduced={cell["reproduced"]} '
                          f'wallMax={cell["wallMaxSeconds"]:.2f} '
                          f'tr={cell["tranches"]}', flush=True)

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
                    'worstOverrunRatio': max(
                        r['wallMaxSeconds'] / (int(target) / 1000.0)
                        for r in cell),
                    'trancheCounts': sorted({r['tranches'] for r in cell}),
                    'trancheCountsAgree': all(
                        r['trancheCountsAgree'] for r in cell),
                    'allDualGateValid': all(r['dualGateValid'] for r in cell),
                }
    summary['allPlanCellsReproduced'] = all(
        r['reproduced'] for r in rows if r['arm'] == 'plan')
    summary['allWallCellsReproduced'] = all(
        r['reproduced'] for r in rows if r['arm'] == 'wall')
    summary['allReplanCellsReproduced'] = all(
        r['reproduced'] for r in rows if r['arm'] == 'replan')
    loads = [row['before'] for row in runlib.LOAD
             if row['before'] is not None]
    summary['boxLoad'] = {
        'n': len(loads),
        'min': min(loads) if loads else None,
        'median': statistics.median(loads) if loads else None,
        'max': max(loads) if loads else None,
    }
    json.dump(summary, open(f'{outdir}/anytime.json', 'w'), indent=1)
    print(json.dumps(summary['table'], indent=1))
    print(f'allPlanCellsReproduced={summary["allPlanCellsReproduced"]}')
    print(f'allWallCellsReproduced={summary["allWallCellsReproduced"]}')
    print(f'allReplanCellsReproduced='
          f'{summary["allReplanCellsReproduced"]}')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
