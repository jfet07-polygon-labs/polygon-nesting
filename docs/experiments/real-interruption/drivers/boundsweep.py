#!/usr/bin/env python3
"""The bound lever, paired against the base arm on the same cells.

    python3 boundsweep.py OUTDIR BINARY REQUEST TARGET_MS SEEDS ROUNDS ARMS

`docs/experiments/robust-plan/` §13.1 is the reason this driver exists. The
confirmation-density sweep was flat-to-negative at twelve cells in two budget
modes, and the cause was not the knob it swept:

> Every cell of both sweeps exits on `bound`, and every cell's first slice drops
> **exactly 1.6160 mm**. [...] So the lever that matters here is **the bound,
> not the grid**, and this round does not touch it.

So this measures the bound. Every arm runs the same seeds in the same rounds,
interleaved by round so no arm always runs first into a cold cache, and every
row carries the columns that section says to read: the **first slice's drop**,
its **exit cause**, its **steps**, its **batches**, and the slice work - beside
the run's depth and wall.

The arms are named on the command line, comma separated, from `ARM_SPECS`.
"""
import json
import os
import statistics
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import planbattery  # noqa: E402
import runlib  # noqa: E402

ARM_SPECS = {
    # The shipped coordinator: nine rungs, no checkpoint policy at all.
    'base': '',
    # The bound unlocked, at three shares of what the coordinator has left for
    # the action. `robust-plan` §13.1's claim is that the queue spends the
    # handed-back budget better than a longer slice would, and a share is the
    # only way to put a number on "better".
    'past25': 'm34past=1,m34pastshare=0.25',
    'past50': 'm34past=1,m34pastshare=0.5',
    'past100': 'm34past=1',
    # The bound unlocked *and* the wall stop armed, which is the pair: past the
    # bound a slice has no natural end, so the thing that bounds its wall is the
    # checkpoint policy and not the operator.
    'pastwall': 'm34past=1,m34wallstop=1',
    'past50wall': 'm34past=1,m34pastshare=0.5,m34wallstop=1',
    # The wall stop alone, on the shipped nine-rung slice: how much of the
    # overrun is the *bounded* slice being in flight at the deadline.
    'wallstop': 'm34wallstop=1',
    # The interleave: the slice hands its turn back every two batches and the
    # queue runs another action before resuming it.
    'yield2': 'm34yield=2',
    'pastyield': 'm34past=1,m34yield=2',
    # The density point `record-line-cascade` bought its millimetre on, now that
    # the bound is unlocked: `step=0.25` at `past=1`.
    'past100grid25': 'm34past=1,m34grid1=0.25',
    'past50grid25': 'm34past=1,m34pastshare=0.5,m34grid1=0.25',
    # The same density point on the *bounded* slice, which is the cell
    # `robust-plan` §14 measured at +2.622 mm against the baseline. Here as the
    # control that says whether anything about the box or the binary moved.
    'grid25': 'm34grid1=0.25',
}


def slices_of(doc):
    calls = ((doc.get('portfolio') or {}).get('operatorCalls')) or []
    return [c['scheduleSlice'] for c in calls if c.get('scheduleSlice')]


def row_for(doc, wall):
    portfolio = doc.get('portfolio') or {}
    reports = slices_of(doc)
    first = reports[0] if reports else {}
    return {
        'processWallSeconds': wall,
        'rawDepthMm': portfolio['incumbent']['rawDepthMm'],
        'dualGateValid': portfolio['incumbent']['dualGateValid'],
        'planUnits': (portfolio.get('plan') or {}).get('units'),
        'workUnits': portfolio.get('workUnits'),
        'actions': len((portfolio.get('schedule') or {}).get('actions') or []),
        'slices': len(reports),
        'firstSliceDropMm': (
            round(first['startDepthMm'] - first['finalDepthMm'], 4)
            if first else None),
        'firstSliceExit': first.get('exitCause'),
        'firstSliceSteps': first.get('stepsTaken'),
        'firstSliceBatches': first.get('batches') or 1,
        'firstSliceWorkUnits': first.get('workUnits'),
        'firstSliceConfirmations': first.get('confirmationsAttempted'),
        'firstSliceAccepted': first.get('confirmationsAccepted'),
        'sliceWorkTotal': sum(r.get('workUnits') or 0 for r in reports),
        'sliceStepsTotal': sum(r.get('stepsTaken') or 0 for r in reports),
        'batchesTotal': sum((r.get('batches') or 1) for r in reports),
        'resumptionsTotal': sum((r.get('resumptions') or 0) for r in reports),
        'interruptedSlices': sum(1 for r in reports if r.get('interrupted')),
        'exits': [r.get('exitCause') for r in reports],
        'digest': planbattery.digest(doc),
    }


def main():
    outdir, binary, request, target = sys.argv[1:5]
    seeds = [int(v) for v in sys.argv[5].split(',')]
    rounds = int(sys.argv[6])
    arms = sys.argv[7].split(',')
    target_s = int(target) / 1000.0
    os.makedirs(outdir, exist_ok=True)
    rows = []
    for rnd in range(rounds):
        order = arms[rnd % len(arms):] + arms[:rnd % len(arms)]
        for arm in order:
            for seed in seeds:
                # Every arm carries the calibration file, because the
                # canonical instrument this campaign measures on is the
                # calibrated plan (`docs/experiments/robust-plan/` §14: without
                # it "two cells of the grid can be handed different budgets by
                # the box, and a cell that drew a bigger plan looks like a cell
                # that bought depth").
                extra = ','.join(part for part in
                                 (f'plancal={planbattery.CAL_LIVE}',
                                  ARM_SPECS[arm]) if part)
                spec = runlib.spec_for(seed, 'plan', target, True, extra)
                tag = f'{arm}-t{target}-s{seed}-r{rnd}'
                doc, wall, err = runlib.run(binary, request, seed, spec,
                                            f'{outdir}/{tag}.json')
                if not (doc.get('portfolio') or {}):
                    rows.append({'tag': tag, 'arm': arm, 'seed': seed,
                                 'round': rnd, 'error': err[-300:]})
                    print(f'{tag}: FAILED {err[-200:]}', flush=True)
                    continue
                row = {'tag': tag, 'arm': arm, 'seed': seed, 'round': rnd,
                       'spec': spec, 'target': int(target)}
                row.update(row_for(doc, wall))
                row['overran'] = wall > target_s
                row['overrunRatio'] = wall / target_s
                rows.append(row)
                print(f"{tag}: wall={wall:6.3f} depth={row['rawDepthMm']:.3f} "
                      f"drop0={row['firstSliceDropMm']} "
                      f"exit0={row['firstSliceExit']} "
                      f"b={row['batchesTotal']} r={row['resumptionsTotal']} "
                      f"intr={row['interruptedSlices']} "
                      f"acts={row['actions']}", flush=True)
    summary = {
        'binary': binary, 'binarySha256': runlib.binary_sha256(binary),
        'request': request, 'target': int(target), 'seeds': seeds,
        'rounds': rounds, 'arms': arms, 'rows': rows,
        'cells': cells(rows, arms, seeds), 'boxLoad': loadstats(),
    }
    with open(f'{outdir}/summary.json', 'w') as handle:
        json.dump(summary, handle, indent=2)
    print(json.dumps(summary['cells'], indent=1))


def cells(rows, arms, seeds):
    """Per (arm, seed) the median over rounds, then the median over seeds."""
    out = {}
    for arm in arms:
        per_seed = {}
        for seed in seeds:
            got = [r for r in rows
                   if r.get('arm') == arm and r.get('seed') == seed
                   and 'rawDepthMm' in r]
            if not got:
                continue
            per_seed[seed] = {
                'depth': statistics.median(r['rawDepthMm'] for r in got),
                'wall': statistics.median(r['processWallSeconds']
                                          for r in got),
                'drop0': statistics.median(r['firstSliceDropMm'] for r in got
                                           if r['firstSliceDropMm'] is not None
                                           ) if any(
                    r['firstSliceDropMm'] is not None for r in got) else None,
                'distinctDepths': len({r['rawDepthMm'] for r in got}),
                'distinctDigests': len({r['digest'] for r in got}),
                'n': len(got),
            }
        if not per_seed:
            continue
        depths = [v['depth'] for v in per_seed.values()]
        walls = [r['processWallSeconds'] for r in rows
                 if r.get('arm') == arm and 'processWallSeconds' in r]
        out[arm] = {
            'perSeed': per_seed,
            'medianOfSeedMedians': statistics.median(depths),
            'wallP50': planbattery.percentile(walls, 0.5) if walls else None,
            'wallMax': max(walls) if walls else None,
            'overran': sum(1 for r in rows if r.get('arm') == arm
                           and r.get('overran')),
            'runs': sum(1 for r in rows if r.get('arm') == arm
                        and 'rawDepthMm' in r),
        }
    return out


def loadstats():
    loads = [row['before'] for row in runlib.LOAD if row['before'] is not None]
    return {'n': len(loads), 'min': min(loads) if loads else None,
            'median': statistics.median(loads) if loads else None,
            'max': max(loads) if loads else None}


if __name__ == '__main__':
    main()
