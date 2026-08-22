#!/usr/bin/env python3
"""The instrument's cost, in the currency it is actually paid in.

    python3 workwall.py OUTDIR BINARY REQUESTS SEEDS WORK ROUNDS

Every previous measurement of the work counters priced them in **millimetres at
a fixed wall** (`docs/experiments/calibrated-plan/` §9,
`docs/experiments/work-currency/` §6), and both rounds had to write up the same
caveat: a wall-budget arm is the least reproducible configuration this campaign
has, so the magnitude does not carry between sessions even when the sign does.
§6's own numbers were 7.553 / 10.400 / 4.006 mm against §9's 2.700 / 1.527 /
1.882 mm on the same three seeds.

This driver measures the same thing without that problem. Both arms run the
**same `work=<units>` budget**, so:

  * the counters are the same counters with the same totals, so the budget is
    the same number;
  * the trajectory is therefore identical, and the driver **asserts** it - the
    whole document, with the arm's own `workMeterArming` record stripped, must
    match field for field;
  * the depth is identical by construction, so it cannot be the measurement;
  * and what is left over is **seconds**, which is exactly what the instrument
    costs.

A paired ratio per (seed, round) rather than a difference of medians, because a
ratio of two runs taken seconds apart survives a box whose load is moving and a
difference of medians taken minutes apart does not. Arm order is rotated by
round so neither arm always runs first into a cold cache.

The depth consequence follows from the ratio rather than being measured beside
it: a plan mode calibrates its budget from the rate phase 0 retires work at, so
an arm that retires the same work in `1/r` of the seconds buys `r` times the
plan - which is what `planbattery.py`'s `plan` against `plandebit` then shows
end to end.
"""
import json
import os
import statistics
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import equiv  # noqa: E402
import runlib  # noqa: E402

ARMS = ['profiler', 'debit']
EXTRA = {'profiler': '', 'debit': 'lanedebit=1'}


def main():
    outdir, binary = sys.argv[1:3]
    requests = sys.argv[3].split(',')
    seeds = [int(v) for v in sys.argv[4].split(',')]
    work = sys.argv[5]
    rounds = int(sys.argv[6])
    os.makedirs(outdir, exist_ok=True)
    rows = []
    for rnd in range(rounds):
        order = ARMS[rnd % len(ARMS):] + ARMS[:rnd % len(ARMS)]
        for arm in order:
            for request in requests:
                for seed in seeds:
                    spec = runlib.spec_for(seed, 'work', work, True,
                                           EXTRA[arm])
                    tag = f'{arm}-{request}-s{seed}-r{rnd}'
                    doc, wall, err = runlib.run(binary, request, seed, spec,
                                                f'{outdir}/{tag}.json')
                    portfolio = doc.get('portfolio') or {}
                    if not portfolio:
                        rows.append({'tag': tag, 'arm': arm, 'seed': seed,
                                     'request': request, 'round': rnd,
                                     'error': err[-300:]})
                        print(f'{tag}: FAILED {err[-200:]}', flush=True)
                        continue
                    rows.append({
                        'tag': tag, 'arm': arm, 'seed': seed,
                        'request': request, 'round': rnd, 'spec': spec,
                        'processWallSeconds': wall,
                        # The engine's own measured stream, which excludes
                        # request loading and result serialisation. Both are
                        # reported: the process wall is what a caller waits and
                        # the coordinator's is what the instrument is inside.
                        'coordinatorSeconds': portfolio['elapsedSeconds'],
                        'rawDepthMm': portfolio['incumbent']['rawDepthMm'],
                        'workUnits': portfolio['workUnits'],
                        'digest': equiv.strong_digest(doc),
                        'workMeterArming': portfolio.get('workMeterArming'),
                    })
                    print(f'{tag}: wall={wall:6.3f} '
                          f'engine={rows[-1]["coordinatorSeconds"]:6.3f} '
                          f'depth={rows[-1]["rawDepthMm"]} '
                          f'units={rows[-1]["workUnits"]}', flush=True)

    good = [r for r in rows if 'error' not in r]
    summary = {'binary': binary,
               'binarySha256': runlib.binary_sha256(binary),
               'work': work, 'requests': requests, 'seeds': seeds,
               'rounds': rounds, 'arms': ARMS, 'rows': rows, 'perCell': {}}
    all_equal = True
    ratios, engine_ratios = [], []
    for request in requests:
        for seed in seeds:
            cell = [r for r in good
                    if r['request'] == request and r['seed'] == seed]
            if not cell:
                continue
            by_arm = {arm: [r for r in cell if r['arm'] == arm]
                      for arm in ARMS}
            if not all(by_arm[arm] for arm in ARMS):
                continue
            digests = sorted({r['digest'] for r in cell})
            depths = sorted({r['rawDepthMm'] for r in cell})
            units = sorted({r['workUnits'] for r in cell})
            equal = len(digests) == 1
            all_equal = all_equal and equal
            # Paired by round: the ratio of the two arms' seconds in the same
            # round, then the median over rounds. A ratio below 1 is the debit
            # arm being faster, which is the direction the round claims.
            per_round = []
            for rnd in range(rounds):
                a = next((r for r in by_arm['profiler'] if r['round'] == rnd),
                         None)
                b = next((r for r in by_arm['debit'] if r['round'] == rnd),
                         None)
                if a and b and a['processWallSeconds'] > 0:
                    per_round.append({
                        'round': rnd,
                        'wallRatio': (b['processWallSeconds']
                                      / a['processWallSeconds']),
                        'engineRatio': (b['coordinatorSeconds']
                                        / a['coordinatorSeconds']),
                        'profilerSeconds': a['processWallSeconds'],
                        'debitSeconds': b['processWallSeconds'],
                    })
            block = {
                'documentsEqual': equal,
                'distinctDigests': digests,
                'distinctDepthsMm': depths,
                'distinctWorkUnits': units,
                'perRound': per_round,
                'profilerEngineSecondsMedian': statistics.median(
                    r['coordinatorSeconds'] for r in by_arm['profiler']),
                'debitEngineSecondsMedian': statistics.median(
                    r['coordinatorSeconds'] for r in by_arm['debit']),
            }
            if per_round:
                block['wallRatioMedian'] = statistics.median(
                    p['wallRatio'] for p in per_round)
                block['engineRatioMedian'] = statistics.median(
                    p['engineRatio'] for p in per_round)
                ratios.append(block['wallRatioMedian'])
                engine_ratios.append(block['engineRatioMedian'])
            summary['perCell'][f'{request}-s{seed}'] = block
            print(f'{request}-s{seed}: documentsEqual={equal} '
                  f'depths={depths} '
                  f'wallRatio={block.get("wallRatioMedian")} '
                  f'engineRatio={block.get("engineRatioMedian")}', flush=True)
    summary['allDocumentsEqual'] = all_equal
    if ratios:
        summary['medianWallRatio'] = statistics.median(ratios)
        summary['medianEngineRatio'] = statistics.median(engine_ratios)
        # The reciprocal, which is the number a plan mode multiplies its budget
        # by: an arm that retires the same work in 0.85 of the seconds measures
        # a rate 1/0.85 higher and plans 1.18x the units.
        summary['impliedPlanMultiplier'] = 1.0 / summary['medianEngineRatio']
    loads = [row['before'] for row in runlib.LOAD if row['before'] is not None]
    summary['boxLoad'] = {
        'n': len(loads),
        'min': min(loads) if loads else None,
        'median': statistics.median(loads) if loads else None,
        'max': max(loads) if loads else None,
    }
    json.dump(summary, open(f'{outdir}/workwall.json', 'w'), indent=1)
    print(json.dumps({k: v for k, v in summary.items() if k != 'rows'},
                     indent=1))
    return 0 if all_equal else 1


if __name__ == '__main__':
    raise SystemExit(main())
