#!/usr/bin/env python3
"""Solves for the phase-zero bias each cell actually has.

    python3 biascalib.py OUTDIR BINARY TARGET_MS SEEDS ROUNDS [REQUESTS]

The plan's model is one line:

    wall = C + t0 + (T*h - t0) * b_true / b_ship

where `C` is the process overhead outside the coordinator (request load and
result serialisation), `t0` and `T*h` come out of the run's own plan block, and
`b_ship` is whatever [`PLAN_PHASE_ZERO_BIAS`] was compiled as. Every term but
`b_true` is measured, so one run per cell solves it:

    b_true = b_ship * (wall - C - t0) / (T*h - t0)

Run with `planq=1`, deliberately: quantisation floors the plan by up to
`1 - 1/step`, which would be indistinguishable from a bias error and would
pollute the constant it is being used to fit.

The constant this reports is the **maximum** over cells, not the median.
Overestimating the bias shortens the plan and costs depth; underestimating it
overruns the wall target, and the wall target is the promise. The per-cell
spread is printed beside it so the size of that choice is visible rather than
implied.
"""
import json
import os
import statistics
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import runlib  # noqa: E402


def main():
    outdir, binary, target_ms = sys.argv[1:4]
    seeds = [int(v) for v in sys.argv[4].split(',')]
    rounds = int(sys.argv[5])
    requests = (sys.argv[6].split(',') if len(sys.argv) > 6
                else ['mixed-61', 'shapes-17', 'triangle-20'])
    target_s = int(target_ms) / 1000.0
    os.makedirs(outdir, exist_ok=True)
    rows = []
    for rnd in range(rounds):
        for request in requests:
            for seed in seeds:
                spec = runlib.spec_for(seed, 'plan', target_ms, True, 'planq=1')
                tag = f'{request}-s{seed}-r{rnd}'
                doc, wall, err = runlib.run(binary, request, seed, spec,
                                            f'{outdir}/{tag}.json')
                portfolio = doc.get('portfolio') or {}
                plan = portfolio.get('plan') or {}
                cal = portfolio.get('planCalibration') or {}
                if not plan:
                    rows.append({'tag': tag, 'error': err[-300:]})
                    print(f'{tag}: FAILED {err[-200:]}', flush=True)
                    continue
                t0 = cal['probeSeconds']
                b_ship = plan['bias']
                h = plan['headroom']
                overhead = wall - portfolio['elapsedSeconds']
                aim = target_s * h
                engine_wall = wall - overhead - t0
                engine_aim = aim - t0
                b_true = (b_ship * engine_wall / engine_aim
                          if engine_aim > 0 else None)
                row = {'tag': tag, 'request': request, 'seed': seed,
                       'round': rnd,
                       'processWallSeconds': wall,
                       'coordinatorSeconds': portfolio['elapsedSeconds'],
                       'overheadSeconds': overhead,
                       'probeSeconds': t0,
                       'planUnits': plan['units'],
                       'biasShipped': b_ship, 'headroom': h,
                       'biasTrue': b_true,
                       'rawDepthMm': portfolio['incumbent']['rawDepthMm']}
                rows.append(row)
                print(f'{tag}: wall={wall:6.3f} C={overhead:.3f} t0={t0:.3f} '
                      f'units={plan["units"]} bTrue={b_true:.4f} '
                      f'depth={row["rawDepthMm"]:.3f}', flush=True)
    good = [r for r in rows if 'error' not in r and r['biasTrue']]
    summary = {'binary': binary, 'targetMs': target_ms, 'seeds': seeds,
               'rounds': rounds, 'requests': requests, 'rows': rows}
    if good:
        biases = [r['biasTrue'] for r in good]
        summary['biasTrue'] = {
            'min': min(biases), 'median': statistics.median(biases),
            'max': max(biases), 'n': len(biases)}
        summary['overheadSeconds'] = {
            'min': min(r['overheadSeconds'] for r in good),
            'median': statistics.median(r['overheadSeconds'] for r in good),
            'max': max(r['overheadSeconds'] for r in good)}
        summary['perCell'] = {}
        for request in requests:
            for seed in seeds:
                cell = [r for r in good
                        if r['request'] == request and r['seed'] == seed]
                if not cell:
                    continue
                summary['perCell'][f'{request}-s{seed}'] = {
                    'n': len(cell),
                    'biasTrueMedian': statistics.median(
                        r['biasTrue'] for r in cell),
                    'biasTrueMin': min(r['biasTrue'] for r in cell),
                    'biasTrueMax': max(r['biasTrue'] for r in cell),
                    'probeSecondsMedian': statistics.median(
                        r['probeSeconds'] for r in cell),
                    'planUnitsDistinct': sorted({r['planUnits'] for r in cell}),
                }
        summary['recommendedBias'] = summary['biasTrue']['max']
    json.dump(summary, open(f'{outdir}/biascalib.json', 'w'), indent=1)
    print(json.dumps({k: v for k, v in summary.items() if k != 'rows'},
                     indent=1))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
