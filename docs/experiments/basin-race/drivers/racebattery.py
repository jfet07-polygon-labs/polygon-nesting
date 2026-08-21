#!/usr/bin/env python3
"""The paired race battery: race on against race off, at equal plan.

    python3 racebattery.py OUTDIR BINARY REQUESTS SEEDS TARGET_MS \\
        [RACE_SPEC] [EXTRA]

Two arms per cell, on one binary, interleaved so neither arm always runs first
into a cold cache:

    off   `race=0`        the incumbent: one basin, chosen by phase 0
    on    `race=<spec>`   the multi-basin race

The instrument is **plan mode**, which is what makes the equal-work gate a gate
rather than a hope. `plan=<ms>` is installed at the end of phase 0 - before the
race phase and before the v3 queue - so both arms of a cell run the *same*
phase 0, read the same probe counter, and land on the same rung of the same
ladder. Their `portfolio.plan.units` must therefore be identical, and this
driver checks it per cell and refuses to call a row equal-work when it is not.
That is a stronger statement than "both arms had a ten-second target": it is
"both arms were given the same integer number of work units".

Reported per cell:

  * depth, both arms, and the signed delta (`on - off`; negative is better,
    because depth is a strip depth);
  * whether the race moved the run off the basin the un-raced run used -
    `portfolio.basinRace.movedOffIncumbent`, which is `winnerSlot != 0`;
  * what the race cost, as its own phase's work units against the run's;
  * the arms' three criteria, so a pick can be read rather than only counted.

Nothing here averages over cells with different plans. A median over rows that
were not equal-work is the failure mode this driver exists to prevent.
"""
import json
import os
import statistics
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import planbattery  # noqa: E402
import runlib  # noqa: E402

DEFAULT_RACE = '3:1:3'


def cell(doc, wall):
    """The fields one run contributes, race or no race."""
    portfolio = doc.get('portfolio') or {}
    incumbent = portfolio.get('incumbent') or {}
    plan = portfolio.get('plan') or {}
    schedule = portfolio.get('schedule') or {}
    race = portfolio.get('basinRace') or {}
    row = {
        'processSeconds': wall,
        'depthMm': incumbent.get('rawDepthMm'),
        'engineDepthMm': doc.get('independentUsedLongAxisDepthMm'),
        'dualGateValid': incumbent.get('dualGateValid'),
        'planUnits': plan.get('units'),
        'workUnits': portfolio.get('workUnits'),
        'coordinatorSeconds': portfolio.get('elapsedSeconds'),
        'publications': portfolio.get('publications'),
        'operatorCalls': portfolio.get('operatorCalls'),
        'scheduleIterations': schedule.get('iterations'),
        'scheduleExitCause': schedule.get('exitCause'),
        'digest': planbattery.digest(doc),
    }
    if race:
        row['race'] = {
            'armsStarted': race.get('armsStarted'),
            'rounds': race.get('rounds'),
            'kept': race.get('kept'),
            'retired': race.get('retired'),
            'winnerSlot': race.get('winnerSlot'),
            'movedOffIncumbent': race.get('movedOffIncumbent'),
            'winnerDepthMm': race.get('winnerDepthMm'),
            'incumbentArmDepthMm': race.get('incumbentArmDepthMm'),
            'workUnits': race.get('workUnits'),
            'seconds': race.get('seconds'),
            'exitCause': race.get('exitCause'),
            'arms': race.get('arms'),
        }
    if '_loadError' in doc:
        row['loadError'] = doc['_loadError'][-400:]
    return row


def main():
    outdir, binary = sys.argv[1:3]
    requests = sys.argv[3].split(',')
    seeds = [int(v) for v in sys.argv[4].split(',')]
    target = int(sys.argv[5])
    race_spec = sys.argv[6] if len(sys.argv) > 6 else DEFAULT_RACE
    extra = sys.argv[7] if len(sys.argv) > 7 else ''
    os.makedirs(outdir, exist_ok=True)

    arms = {'off': 'race=0', 'on': f'race={race_spec}'}
    result = {
        'binary': binary, 'targetMillis': target, 'raceSpec': race_spec,
        'requests': requests, 'seeds': seeds, 'extra': extra,
        'arms': arms, 'rows': [],
    }
    for index, request in enumerate(requests):
        for seed in seeds:
            # Arm order rotated per cell so neither arm always runs first.
            order = ['off', 'on'] if (index + seed) % 2 == 0 else ['on', 'off']
            row = {'request': request, 'seed': seed, 'armOrder': order}
            for arm in order:
                pieces = [arms[arm]] + ([extra] if extra else [])
                spec = runlib.spec_for(seed, 'plan', target, True,
                                       ','.join(pieces))
                tag = f'{request}-s{seed}-{arm}'
                doc, wall, err = runlib.run(
                    binary, request, seed, spec, f'{outdir}/{tag}.json')
                row[arm] = cell(doc, wall)
                row[arm]['spec'] = spec
                if err:
                    row[arm]['stderrTail'] = err[-300:]
            off, on = row['off'], row['on']
            row['equalWork'] = (
                off['planUnits'] is not None
                and off['planUnits'] == on['planUnits'])
            if off['depthMm'] is not None and on['depthMm'] is not None:
                row['deltaMm'] = on['depthMm'] - off['depthMm']
            row['movedOffIncumbent'] = (on.get('race') or {}).get(
                'movedOffIncumbent')
            result['rows'].append(row)
            print(f'{request} s{seed}: off={off["depthMm"]} on={on["depthMm"]} '
                  f'delta={row.get("deltaMm")} equalWork={row["equalWork"]} '
                  f'moved={row["movedOffIncumbent"]}', flush=True)

    paired = [r for r in result['rows']
              if r.get('equalWork') and r.get('deltaMm') is not None]
    deltas = [r['deltaMm'] for r in paired]
    result['summary'] = {
        'cells': len(result['rows']),
        'equalWorkCells': len(paired),
        'medianDeltaMm': statistics.median(deltas) if deltas else None,
        'meanDeltaMm': statistics.fmean(deltas) if deltas else None,
        'raceBetter': sum(1 for d in deltas if d < 0),
        'raceWorse': sum(1 for d in deltas if d > 0),
        'tied': sum(1 for d in deltas if d == 0),
        'movedOffIncumbent': sum(
            1 for r in result['rows'] if r.get('movedOffIncumbent')),
        'raceRan': sum(1 for r in result['rows']
                       if (r['on'].get('race') or {}).get('armsStarted')),
    }
    json.dump(result, open(f'{outdir}/racebattery-{target}.json', 'w'),
              indent=1)
    print(json.dumps(result['summary'], indent=1))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
