#!/usr/bin/env python3
"""Does the wall stop fire at all, and which classes does it bind?

    python3 forcedoverrun.py OUTDIR BINARY REQUEST TARGET_MS SEEDS ROUNDS \\
        [HEADROOM]

The calibrated thirty-second battery is the *shipping* measurement and it has a
weakness as a test of the mechanism: a calibrated plan under-buys, so on most
cells the run finishes before the deadline and the wall stop has nothing to do.
A battery in which the policy never fires cannot tell "the policy works" from
"the policy is unreachable", which is exactly the trap
`docs/experiments/replan/` §12.3 fell into with `m34cap` - a key that was armed
in the spec and did nothing.

So this drives the run *into* an overrun on purpose. `planhead=<h>` with `h`
well above 1 buys a plan the wall cannot pay for, and then the four arms
separate cleanly:

    off        no wall policy at all - the overrun, at full size
    checkpoint `m34wallstop=1`: the shipped policy, which binds the mode-34
               checkpoint and nothing else
    all        `m34wallstopall=1`: the same deadline in front of the queue
    reserve    and `m34wallreserve=1`, which additionally refuses a class the
               queue does not expect to finish

This is a **mechanism** test and not a quality one, and the distinction matters
for how the numbers are read: an over-bought plan stopped at the wall returns a
shallower layout than the same plan allowed to overrun, by construction, and
that difference is not a price the shipping configuration pays. What the arms
are compared on here is *seconds against the target* and *whether the output is
exact-valid*, which is the whole of what a wall stop is for.
"""
import json
import os
import statistics
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import runlib  # noqa: E402

ARMS = {
    'off': '',
    'checkpoint': 'm34wallstop=1',
    'all': 'm34wallstopall=1',
    'reserve': 'm34wallstopall=1,m34wallreserve=1',
}
ORDER = ['off', 'checkpoint', 'all', 'reserve']


def main():
    outdir, binary, request, target_ms = sys.argv[1:5]
    seeds = [int(v) for v in sys.argv[5].split(',')]
    rounds = int(sys.argv[6])
    headroom = sys.argv[7] if len(sys.argv) > 7 else '3.0'
    target_s = int(target_ms) / 1000.0
    os.makedirs(outdir, exist_ok=True)
    rows = []
    for rnd in range(rounds):
        order = ORDER[rnd % len(ORDER):] + ORDER[:rnd % len(ORDER)]
        for arm in order:
            for seed in seeds:
                extra = ','.join(
                    p for p in (f'planhead={headroom}', ARMS[arm]) if p)
                spec = runlib.spec_for(seed, 'plan', target_ms, True, extra)
                tag = f'{arm}-s{seed}-r{rnd}'
                doc, wall, err = runlib.run(binary, request, seed, spec,
                                            f'{outdir}/{tag}.json')
                portfolio = doc.get('portfolio') or {}
                if not portfolio:
                    rows.append({'tag': tag, 'arm': arm, 'seed': seed,
                                 'round': rnd, 'error': err[-300:]})
                    print(f'{tag}: FAILED {err[-200:]}', flush=True)
                    continue
                schedule = portfolio.get('schedule') or {}
                rows.append({
                    'tag': tag, 'arm': arm, 'seed': seed, 'round': rnd,
                    'spec': spec,
                    'processWallSeconds': wall,
                    'coordinatorSeconds': portfolio['elapsedSeconds'],
                    'overrunSeconds': wall - target_s,
                    'overran': wall > target_s,
                    'rawDepthMm': portfolio['incumbent']['rawDepthMm'],
                    'dualGateValid': portfolio['incumbent']['dualGateValid'],
                    'exitCause': schedule.get('exitCause'),
                    'actions': schedule.get('iterations'),
                    # The plan is the same number in every arm of a cell only
                    # if the box was steady while the cell ran; a live plan
                    # reads a clock. Recorded so a reader can see when two arms
                    # of one cell are not the same budget - which is a caveat
                    # about this battery and not about the mechanism.
                    'planUnits': (portfolio.get('plan') or {}).get('units'),
                    'workUnits': portfolio['workUnits'],
                })
                print(f'{tag}: wall={wall:6.2f} over={rows[-1]["overrunSeconds"]:+6.2f} '
                      f'exit={rows[-1]["exitCause"]} '
                      f'depth={rows[-1]["rawDepthMm"]:.4f} '
                      f'valid={rows[-1]["dualGateValid"]} '
                      f'plan={rows[-1]["planUnits"]}', flush=True)

    good = [r for r in rows if 'error' not in r]
    summary = {'binary': binary,
               'binarySha256': runlib.binary_sha256(binary),
               'request': request, 'targetMs': target_ms,
               'targetSeconds': target_s, 'headroom': headroom,
               'seeds': seeds, 'rounds': rounds, 'arms': ORDER,
               'rows': rows, 'byArm': {}}
    for arm in ORDER:
        cell = [r for r in good if r['arm'] == arm]
        if not cell:
            continue
        walls = sorted(r['processWallSeconds'] for r in cell)
        summary['byArm'][arm] = {
            'n': len(cell),
            'overruns': sum(1 for r in cell if r['overran']),
            'exactValid': sum(1 for r in cell if r['dualGateValid']),
            'wallMedian': statistics.median(walls),
            'wallMax': max(walls),
            'worstOverrunSeconds': max(r['overrunSeconds'] for r in cell),
            'depthMedianMm': statistics.median(r['rawDepthMm'] for r in cell),
            'exitCauses': {
                cause: sum(1 for r in cell if str(r['exitCause']) == cause)
                for cause in sorted({str(r['exitCause']) for r in cell})
            },
            'distinctPlanUnits': sorted({r['planUnits'] for r in cell}),
        }
    loads = [row['before'] for row in runlib.LOAD if row['before'] is not None]
    summary['boxLoad'] = {
        'n': len(loads),
        'min': min(loads) if loads else None,
        'median': statistics.median(loads) if loads else None,
        'max': max(loads) if loads else None,
    }
    json.dump(summary, open(f'{outdir}/forcedoverrun.json', 'w'), indent=1)
    print(json.dumps(summary['byArm'], indent=1))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
