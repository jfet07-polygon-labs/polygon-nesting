#!/usr/bin/env python3
"""What a work plan has to be calibrated against, measured before it is built.

    python3 calibrate.py OUTDIR BINARY REQUEST UNITS SEEDS ROUNDS

Three numbers decide whether a wall target can be met by a fixed work plan, and
none of them can be guessed:

  t0, W0    the protected phase-0 slice's own seconds and work units. `W0` is a
            deterministic function of (request, seed) - it is a counter, not a
            clock - so the *entire* run-to-run variation in a phase-0-calibrated
            plan is the variation in `t0`. Its spread is therefore the spread of
            the plan, and it sets how coarse a quantisation has to be before two
            processes agree on the plan at all.

  b         the phase-0 **bias**: `rate(phase 0) / rate(everything after it)`.
            A probe is only a rate estimator for the work it resembles. Phase 0
            is one mode-0 pipeline; the rest of the run is a ranked queue over
            eight classes. If `b != 1` a plan sized at phase 0's rate lands at
            the wrong wall, and the correction is a constant this measures.

  h         the headroom: the p50/p95 ratio of process wall at a **fixed** work
            plan. This is the part of the wall a plan cannot control, because
            the plan is already fixed when it happens.

Work budget throughout, deliberately: the work counters are only armed under a
work budget (`work_units_now` is constantly zero otherwise), so a wall-budget
run cannot report `W0` at all. That is also why the plan mode is a work mode.
"""
import json
import os
import statistics
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import runlib  # noqa: E402


def percentile(values, q):
    """Nearest-rank percentile on a sorted copy; no interpolation.

    Twenty samples do not support interpolation between order statistics, and
    a nearest-rank p95 of twenty runs is honestly "the second worst", which is
    what a wall promise is actually made of.
    """
    if not values:
        return None
    ordered = sorted(values)
    rank = max(1, min(len(ordered), int(q * len(ordered) + 0.9999999)))
    return ordered[rank - 1]


def main():
    outdir, binary, request, units = sys.argv[1:5]
    seeds = [int(v) for v in sys.argv[5].split(',')]
    rounds = int(sys.argv[6])
    spec_extra = sys.argv[7] if len(sys.argv) > 7 else ''
    os.makedirs(outdir, exist_ok=True)
    rows = []
    for rnd in range(rounds):
        for seed in seeds:
            spec = runlib.spec_for(seed, 'work', units, True, spec_extra)
            tag = f'{request}-s{seed}-r{rnd}'
            doc, wall, err = runlib.run(binary, request, seed, spec,
                                        f'{outdir}/{tag}.json')
            portfolio = doc.get('portfolio') or {}
            phases = portfolio.get('phases') or []
            zero = next((p for p in phases if p['name'] == 'm0'), None)
            if zero is None:
                rows.append({'tag': tag, 'error': err[-300:]})
                print(f'{tag}: FAILED {err[-200:]}', flush=True)
                continue
            t0 = zero['elapsedSeconds']
            w0 = zero['workUnits']
            total_s = portfolio['elapsedSeconds']
            total_w = portfolio['workUnits']
            rest_s = total_s - t0
            rest_w = total_w - w0
            row = {
                'tag': tag, 'seed': seed, 'round': rnd,
                'processWallSeconds': wall,
                'phaseZeroSeconds': t0, 'phaseZeroWorkUnits': w0,
                'coordinatorSeconds': total_s, 'coordinatorWorkUnits': total_w,
                'phaseZeroRate': w0 / t0 if t0 else None,
                'restRate': rest_w / rest_s if rest_s > 0 else None,
                'rawDepthMm': portfolio['incumbent']['rawDepthMm'],
            }
            row['bias'] = (row['phaseZeroRate'] / row['restRate']
                           if row['restRate'] else None)
            rows.append(row)
            print(f'{tag}: wall={wall:.3f} t0={t0:.3f} W0={w0} '
                  f'rate0={row["phaseZeroRate"]:.0f} b={row["bias"]:.3f} '
                  f'depth={row["rawDepthMm"]:.3f}', flush=True)

    good = [r for r in rows if 'error' not in r]
    summary = {'binary': binary, 'request': request, 'units': units,
               'seeds': seeds, 'rounds': rounds, 'specExtra': spec_extra,
               'rows': rows}
    if good:
        walls = [r['processWallSeconds'] for r in good]
        summary['wall'] = {
            'p50': percentile(walls, 0.50), 'p95': percentile(walls, 0.95),
            'max': max(walls), 'min': min(walls), 'n': len(walls)}
        summary['headroom'] = summary['wall']['p50'] / summary['wall']['p95']
        summary['bias'] = {
            'median': statistics.median(r['bias'] for r in good),
            'min': min(r['bias'] for r in good),
            'max': max(r['bias'] for r in good)}
        # Per seed, because `W0` is per seed: the spread that matters for the
        # plan is the spread of `t0` *within* one cell, not across cells whose
        # phase-0 work genuinely differs.
        per_seed = {}
        for seed in seeds:
            cell = [r for r in good if r['seed'] == seed]
            if len(cell) < 2:
                continue
            t0s = [r['phaseZeroSeconds'] for r in cell]
            w0s = sorted({r['phaseZeroWorkUnits'] for r in cell})
            per_seed[str(seed)] = {
                'n': len(cell),
                'phaseZeroSecondsMedian': statistics.median(t0s),
                'phaseZeroSecondsMin': min(t0s), 'phaseZeroSecondsMax': max(t0s),
                'phaseZeroSecondsRelSpread':
                    (max(t0s) - min(t0s)) / statistics.median(t0s),
                'phaseZeroWorkUnitsDistinct': w0s,
                'phaseZeroWorkUnitsDeterministic': len(w0s) == 1,
                'depthsDistinct': sorted({r['rawDepthMm'] for r in cell}),
            }
        summary['perSeed'] = per_seed
    json.dump(summary, open(f'{outdir}/calibrate.json', 'w'), indent=1)
    print(json.dumps({k: v for k, v in summary.items() if k != 'rows'},
                     indent=1))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
