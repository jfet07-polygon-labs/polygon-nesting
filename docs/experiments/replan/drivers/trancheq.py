#!/usr/bin/env python3
"""What should the first tranche aim at?

    python3 trancheq.py OUTDIR BINARY REQUEST TARGETS SEEDS FRACTIONS ROUNDS

`PLAN_FIRST_TRANCHE` is the one constant this round introduces, and it is a
trade with two ends (see the constant's own doc comment). This driver measures
both ends on the same cells at the same time:

  * **the overrun end** - process wall against the target, per fraction. A
    fraction of 1.0 is the single-plan mode and is the arm that ran 36.39 s
    against a 30 s target in `docs/experiments/calibrated-plan/` §10.
  * **the depth end** - the raw depth each fraction produced, and how many
    tranches it took to get there.

Reported per (target, fraction) so the answer can be different at three seconds
and at thirty - which, given that the whole reason this constant exists is that
the bias grows with the budget, it may well be.

**This driver cannot produce `evidence/cap-30s.json`, and that is a finding
rather than a limitation.** Sol review 9 §P0 opened on it: the committed file
carries rows whose `spec` field reads `...,m34cap=0` and `...,m34cap=1`, and the
only branch below turns a `fraction` other than `off` into `planfirst=<value>`,
so `capoff`/`capon` would have emitted `replan=1,planfirst=capon`. There is no
path here that writes `m34cap` at all. The file was produced by a driver that is
not this one, or by this one modified during the collection - either way the
source, the driver and the measured binary did not agree, which is the provenance
break that round was retracted for.

The claim itself is retracted for a *separate and larger* reason - `m34cap`
could not stop a committed slice at that HEAD, so the two arms were one
trajectory - and `docs/experiments/real-interruption/` §2 is the replay that
established it. See the correction at the head of `../README.md` and the
`SUPERSEDED` block in `../evidence/cap-30s.json`.

Nothing here is changed to make the retracted rows reproducible: the point of
leaving the driver as it was is that a reader can check the claim above.
"""
import json
import os
import statistics
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import planbattery  # noqa: E402
import runlib  # noqa: E402


def main():
    outdir, binary, request = sys.argv[1:4]
    targets = [int(v) for v in sys.argv[4].split(',')]
    seeds = [int(v) for v in sys.argv[5].split(',')]
    fractions = [v for v in sys.argv[6].split(',')]
    rounds = int(sys.argv[7])
    os.makedirs(outdir, exist_ok=True)
    rows = []
    for rnd in range(rounds):
        # Rotated by round, so no fraction always runs first into a cold cache.
        order = fractions[rnd % len(fractions):] + fractions[:rnd % len(fractions)]
        for fraction in order:
            for target in targets:
                for seed in seeds:
                    extra = ('replan=0' if fraction == 'off'
                             else f'replan=1,planfirst={fraction}')
                    spec = runlib.spec_for(seed, 'plan', str(target), True,
                                           extra)
                    tag = f'f{fraction}-t{target}-s{seed}-r{rnd}'
                    doc, wall, err = runlib.run(binary, request, seed, spec,
                                                f'{outdir}/{tag}.json')
                    portfolio = doc.get('portfolio') or {}
                    if not portfolio:
                        rows.append({'tag': tag, 'error': err[-300:]})
                        print(f'{tag}: FAILED {err[-200:]}', flush=True)
                        continue
                    plan = portfolio.get('plan') or {}
                    tranches = portfolio.get('tranches') or []
                    row = {
                        'tag': tag, 'fraction': fraction, 'target': target,
                        'seed': seed, 'round': rnd, 'spec': spec,
                        'processWallSeconds': wall,
                        'overran': wall > target / 1000.0,
                        'overrunRatio': wall / (target / 1000.0),
                        'rawDepthMm': portfolio['incumbent']['rawDepthMm'],
                        'dualGateValid':
                            portfolio['incumbent']['dualGateValid'],
                        'planUnits': plan.get('units'),
                        'tranches': len(tranches),
                        'trancheUnits': [t['units'] for t in tranches],
                        'finalUnits': (tranches[-1]['units'] if tranches
                                       else plan.get('units')),
                        'digest': planbattery.digest(doc),
                    }
                    rows.append(row)
                    print(f'{tag}: wall={wall:6.3f} '
                          f'depth={row["rawDepthMm"]:.3f} '
                          f'tr={row["tranches"]} '
                          f'units={row["planUnits"]}->{row["finalUnits"]}',
                          flush=True)
    summary = {'binary': binary, 'request': request, 'targets': targets,
               'seeds': seeds, 'fractions': fractions, 'rounds': rounds,
               'rows': rows, 'cells': {}}
    for target in targets:
        for fraction in fractions:
            cell = [r for r in rows if r.get('fraction') == fraction
                    and r.get('target') == target]
            if not cell:
                continue
            walls = [r['processWallSeconds'] for r in cell]
            summary['cells'][f'{target}/{fraction}'] = {
                'n': len(cell),
                'wallP50': planbattery.percentile(walls, 0.50),
                'wallP95': planbattery.percentile(walls, 0.95),
                'wallMax': max(walls),
                'overruns': sum(1 for r in cell if r['overran']),
                'worstOverrunRatio': max(r['overrunRatio'] for r in cell),
                'depthMedianMm': statistics.median(
                    r['rawDepthMm'] for r in cell),
                'perSeedDepthMedianMm': {
                    str(s): statistics.median(
                        r['rawDepthMm'] for r in cell if r['seed'] == s)
                    for s in seeds
                    if any(r['seed'] == s for r in cell)},
                'trancheCounts': sorted({r['tranches'] for r in cell}),
                'allDualGateValid': all(r['dualGateValid'] for r in cell),
                # Per seed, how many distinct documents the fraction produced.
                # A fraction that lands mid-rung on the re-plan is a fraction
                # that costs reproducibility, and this is where it shows.
                'distinctDigestsPerSeed': {
                    str(s): len({r['digest'] for r in cell if r['seed'] == s})
                    for s in seeds
                    if any(r['seed'] == s for r in cell)},
            }
    loads = [row['before'] for row in runlib.LOAD
             if row['before'] is not None]
    summary['boxLoad'] = {
        'n': len(loads),
        'min': min(loads) if loads else None,
        'median': statistics.median(loads) if loads else None,
        'max': max(loads) if loads else None,
    }
    json.dump(summary, open(f'{outdir}/trancheq.json', 'w'), indent=1)
    print(json.dumps(summary['cells'], indent=1))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
