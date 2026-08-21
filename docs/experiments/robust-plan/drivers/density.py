#!/usr/bin/env python3
"""The confirmation-density sweep on the first m34 slice.

    python3 density.py OUTDIR BINARY MODE VALUE ROUNDS [SEEDS] [FIXTURE] [EXTRA]

`MODE` is `plan` or `work`, and the two are the two halves of the gate:

* **`plan`** at the ten-second target is the *decision* run - the budget the
  user priority names, with everything the coordinator does downstream of the
  first slice free to move. It answers "does the run publish deeper?".
* **`work`** at a pinned budget is Grok review 3 §item 1's gate - *"equal-work
  sullo stesso parent, depth per query non peggiore"*. Equal work makes the
  arms comparable at all; depth-per-query is the ratio that says whether the
  extra confirmations bought their price or merely spent it.

The grid is the product and not the diagonal, because the two knobs raise
confirmations per micron by different mechanisms and drag different other things
with them: `step_grid` shrinks the clamp increment, so it also multiplies the
repair sweeps spent per micron of descent; `confirm_every` shortens the cadence,
so it asks more often at the same clamp. A diagonal cannot tell which of the two
paid.

Reported per cell: published depth, the first slice's own confirmation counts and
step counts, how many m34 slices the whole run bought, and the two ratios -
millimetres per thousand of the slice's own work units, and millimetres per
accepted confirmation.
"""
import json
import os
import statistics
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import runlib  # noqa: E402

GRIDS = [1.0, 0.5, 0.25, 0.125]
CONFIRMS = [4, 2, 1]


def slices_of(portfolio):
    """Every m34 call's slice report, in dispatch order."""
    out = []
    for call in portfolio.get('operatorCalls') or []:
        # `operator` is the mode's name and `scheduleSlice` is present only on
        # an m34 call that actually armed a schedule, so the second test is the
        # load-bearing one: a mode-34 call refused at the entry gate carries no
        # slice and must not be counted as one.
        if call.get('operator') == 'mode34' and call.get('scheduleSlice'):
            out.append(call['scheduleSlice'])
    return out


def main():
    outdir, binary, mode, value = sys.argv[1:5]
    rounds = int(sys.argv[5])
    seeds = ([int(v) for v in sys.argv[6].split(',')] if len(sys.argv) > 6
             else [0, 1, 2])
    fixture = sys.argv[7] if len(sys.argv) > 7 else 'mixed-61'
    extra = sys.argv[8] if len(sys.argv) > 8 else ''
    os.makedirs(outdir, exist_ok=True)
    cells = [(grid, confirm) for grid in GRIDS for confirm in CONFIRMS]
    rows = []
    for rnd in range(rounds):
        # Rotated by round, so no configuration always runs first into a cold
        # page cache.
        order = cells[rnd % len(cells):] + cells[:rnd % len(cells)]
        for grid, confirm in order:
            for seed in seeds:
                pieces = [p for p in (extra,) if p]
                # `1.0` and `4` are the module's own defaults, so the baseline
                # cell names **no key at all** - which is what makes it the same
                # document the base binary produces rather than one that only
                # agrees numerically.
                if grid != 1.0:
                    pieces.append(f'm34grid1={grid}')
                if confirm != 4:
                    pieces.append(f'm34confirm1={confirm}')
                spec = runlib.spec_for(seed, mode, value, True,
                                       ','.join(pieces))
                tag = f'g{grid}-c{confirm}-s{seed}-r{rnd}'
                doc, wall, err = runlib.run(binary, fixture, seed, spec,
                                            f'{outdir}/{tag}.json')
                portfolio = doc.get('portfolio') or {}
                if not portfolio:
                    print(f'{tag}: FAILED {err[-200:]}', flush=True)
                    continue
                found = slices_of(portfolio)
                first = found[0] if found else {}
                start = first.get('startDepthMm')
                final = first.get('finalDepthMm')
                units = first.get('workUnits') or 0
                accepted = first.get('confirmationsAccepted') or 0
                drop = (start - final) if (start is not None
                                           and final is not None) else None
                row = {
                    'tag': tag, 'grid': grid, 'confirm': confirm, 'seed': seed,
                    'round': rnd, 'spec': spec,
                    'processWallSeconds': wall,
                    'rawDepthMm': portfolio['incumbent']['rawDepthMm'],
                    'coordinatorWorkUnits': portfolio['workUnits'],
                    'planUnits': (portfolio.get('plan') or {}).get('units'),
                    'slices': len(found),
                    'firstSliceStartMm': start,
                    'firstSliceFinalMm': final,
                    'firstSliceDropMm': drop,
                    'firstSliceSteps': first.get('stepsTaken'),
                    'firstSliceWorkUnits': units,
                    'firstSliceConfirmAttempted':
                        first.get('confirmationsAttempted'),
                    'firstSliceConfirmAccepted': accepted,
                    'firstSliceConfirmRefused':
                        first.get('confirmationsRefused'),
                    'firstSliceConfirmSkipped':
                        first.get('confirmationsSkippedInfeasible'),
                    'firstSliceExit': first.get('exitCause'),
                    # Grok's ratio, both denominators. `None` rather than zero
                    # when the slice bought nothing, so a barren cell cannot be
                    # averaged into a win.
                    'mmPerKiloUnit': (drop / (units / 1000.0)
                                      if drop is not None and units else None),
                    'mmPerAcceptedConfirmation': (drop / accepted
                                                  if drop is not None
                                                  and accepted else None),
                    'load1': runlib.LOAD[-1]['before'],
                }
                rows.append(row)
                print(f'{tag}: wall={wall:6.3f} depth={row["rawDepthMm"]:.4f} '
                      f'slices={row["slices"]} drop='
                      f'{-1 if drop is None else drop:.4f} '
                      f'conf={row["firstSliceConfirmAccepted"]}/'
                      f'{row["firstSliceConfirmAttempted"]} '
                      f'u={units}', flush=True)

    def median_of(cell, key):
        values = [r[key] for r in cell if r.get(key) is not None]
        return statistics.median(values) if values else None

    summary = {'binary': binary, 'mode': mode, 'value': value,
               'fixture': fixture, 'seeds': seeds, 'rounds': rounds,
               'extra': extra, 'grids': GRIDS, 'confirms': CONFIRMS,
               'rows': rows, 'byCell': {}}
    for grid, confirm in cells:
        cell = [r for r in rows if r['grid'] == grid and r['confirm'] == confirm]
        if not cell:
            continue
        per_seed = {}
        for seed in seeds:
            block = [r for r in cell if r['seed'] == seed]
            if block:
                per_seed[str(seed)] = {
                    'depthMedianMm': statistics.median(
                        r['rawDepthMm'] for r in block),
                    'distinctDepthsMm': sorted({r['rawDepthMm']
                                                for r in block}),
                    'mmPerKiloUnit': median_of(block, 'mmPerKiloUnit'),
                    'slices': sorted({r['slices'] for r in block}),
                }
        summary['byCell'][f'{grid}/{confirm}'] = {
            'n': len(cell),
            'grid': grid, 'confirm': confirm,
            'depthMedianMm': statistics.median(r['rawDepthMm'] for r in cell),
            'seedMedianOfMedians': statistics.median(
                v['depthMedianMm'] for v in per_seed.values()),
            'wallP50': statistics.median(r['processWallSeconds'] for r in cell),
            'wallMax': max(r['processWallSeconds'] for r in cell),
            'slicesMedian': statistics.median(r['slices'] for r in cell),
            'firstSliceDropMm': median_of(cell, 'firstSliceDropMm'),
            'firstSliceStepsMedian': median_of(cell, 'firstSliceSteps'),
            'firstSliceWorkUnitsMedian': median_of(cell, 'firstSliceWorkUnits'),
            'confirmAcceptedMedian': median_of(cell,
                                               'firstSliceConfirmAccepted'),
            'confirmAttemptedMedian': median_of(cell,
                                                'firstSliceConfirmAttempted'),
            'mmPerKiloUnit': median_of(cell, 'mmPerKiloUnit'),
            'mmPerAcceptedConfirmation': median_of(
                cell, 'mmPerAcceptedConfirmation'),
            # Why the first slice stopped, counted. This is the column that
            # decides whether the lever has anything to buy at all: a slice that
            # exits on `bound` reached the drop it was asked for, so a finer grid
            # walks the same distance in more steps and the extra confirmations
            # are cost with no possible yield. A slice that exits on `budget` or
            # `barren` stopped short, and only there can density change what it
            # reaches.
            'exitCauses': {
                cause: sum(1 for r in cell if r.get('firstSliceExit') == cause)
                for cause in sorted({r.get('firstSliceExit') for r in cell}
                                    - {None})},
            'perSeed': per_seed,
        }
    baseline = summary['byCell'].get('1.0/4')
    if baseline:
        for key, block in summary['byCell'].items():
            block['deltaVsBaselineMm'] = (
                block['seedMedianOfMedians'] - baseline['seedMedianOfMedians'])
            if baseline['mmPerKiloUnit'] and block['mmPerKiloUnit']:
                block['mmPerKiloUnitRatio'] = (
                    block['mmPerKiloUnit'] / baseline['mmPerKiloUnit'])
    loads = [row['before'] for row in runlib.LOAD if row['before'] is not None]
    summary['boxLoad'] = {
        'n': len(loads), 'min': min(loads) if loads else None,
        'median': statistics.median(loads) if loads else None,
        'max': max(loads) if loads else None,
    }
    json.dump(summary, open(f'{outdir}/density.json', 'w'), indent=1)
    print(json.dumps(summary['byCell'], indent=1))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
