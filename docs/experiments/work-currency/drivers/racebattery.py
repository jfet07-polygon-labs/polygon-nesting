#!/usr/bin/env python3
"""§4: does the currency bound the draws? The race's own equal-work gate,
re-run with three arms instead of two.

`docs/experiments/basin-race/` §4.5 is the question this answers. The race
loses its equal-work gate on mixed-61 by **+2.366** and **+2.934 mm**, it moves
the run off the incumbent in **0 of 21** cells, and §4.4 names the cause: the
share ceiling is enforced in the work currency, mode 20 costs 92.7 units per
second in that currency, so the ceiling cannot bound the draws' wall. The race
exits on `deadline` having spent 8.2-9.6 s of a ten-second target.

If that pricing was the whole story, arming the parallel currency should bound
the draws and the two equal-work cells should move toward parity. If the loss
survives comparable pricing, the race is losing for a second reason and the
currency has ruled it out rather than fixed it. Either answer is worth the
runs; **the race stays off either way**, because §4.3's 0-of-21 pick rate is a
property of the criteria and no currency touches it.

Three arms per cell, one binary, rotated so no arm always runs first:

    off    `race=0`                  the un-raced run
    on     `race=3:1:3`              the race, priced by the shipped meter
    on2    `race=3:1:3,cur2=1`       the race, priced by the parallel currency

The instrument is plan mode, and `plan=<ms>` is installed at the end of phase 0
- before the race phase - so all three arms of a cell run the same phase 0 and
should land on the same rung. Should, not must: the ladder straddles under
load, so this driver **checks** `portfolio.plan.units` per cell and refuses to
call a row equal-work when it is not.

One thing the third arm does *not* control for and this driver reports rather
than hides: `cur2=1` changes what the queue can afford *after* the race too, so
`on2 - off` is the race-plus-currency delta. `§4`'s companion table - the plan
battery with the race off - is what separates them.

    python3 racebattery.py OUTDIR BINARY REQUESTS SEEDS TARGET_MS [RACE_SPEC]
"""
import json
import os
import statistics
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import runlib  # noqa: E402

DEFAULT_RACE = '3:1:3'


def cell(doc, wall):
    portfolio = doc.get('portfolio') or {}
    incumbent = portfolio.get('incumbent') or {}
    plan = portfolio.get('plan') or {}
    schedule = portfolio.get('schedule') or {}
    race = portfolio.get('basinRace') or {}
    calls = portfolio.get('operatorCalls') or []
    row = {
        'processSeconds': wall,
        'depthMm': incumbent.get('rawDepthMm'),
        'dualGateValid': incumbent.get('dualGateValid'),
        'planUnits': plan.get('units'),
        'workUnits': portfolio.get('workUnits'),
        'coordinatorSeconds': portfolio.get('elapsedSeconds'),
        'publications': len(portfolio.get('publications') or []),
        'operatorCalls': len(calls),
        'scheduleIterations': schedule.get('iterations'),
        'scheduleExitCause': schedule.get('exitCause'),
        'workCurrency': portfolio.get('workCurrency'),
        'digest': runlib.doc_digest(doc),
    }
    # What the draws cost, in both currencies and in seconds. This is the row
    # the whole question turns on: the shipped meter prices these at a few
    # hundred units and the clock at three seconds each.
    draws = [c for c in calls if c['operator'] == 'mode20']
    row['draws'] = {
        'calls': len(draws),
        'seconds': sum(c['elapsedSeconds'] for c in draws),
        'globalUnits': sum(c['globalUnits'] for c in draws),
        'chargedUnits': sum(c['workUnits'] for c in draws),
        'classUnits': sum((c.get('workCurrency') or {}).get('classUnits', 0)
                          for c in draws),
    }
    if race:
        row['race'] = {
            'armsStarted': race.get('armsStarted'),
            'rounds': race.get('rounds'),
            'winnerSlot': race.get('winnerSlot'),
            'movedOffIncumbent': race.get('movedOffIncumbent'),
            'workUnits': race.get('workUnits'),
            'seconds': race.get('seconds'),
            'exitCause': race.get('exitCause'),
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
    os.makedirs(outdir, exist_ok=True)

    arms = {
        'off': 'race=0',
        'on': f'race={race_spec}',
        'on2': f'race={race_spec},cur2=1',
    }
    order_base = ['off', 'on', 'on2']
    result = {
        'binary': binary, 'binarySha256': runlib.sha256_of(binary),
        'targetMillis': target, 'raceSpec': race_spec,
        'requests': requests, 'seeds': seeds, 'arms': arms, 'rows': [],
    }
    for index, request in enumerate(requests):
        for seed in seeds:
            shift = (index + seed) % 3
            order = order_base[shift:] + order_base[:shift]
            row = {'request': request, 'seed': seed, 'armOrder': order}
            for arm in order:
                spec = runlib.spec_for(seed, 'plan', target, True, arms[arm])
                tag = f'race-{request}-s{seed}-{arm}'
                doc, wall, err = runlib.run(
                    binary, request, seed, spec, f'{outdir}/{tag}.json')
                row[arm] = cell(doc, wall)
                row[arm]['spec'] = spec
                if err:
                    row[arm]['stderrTail'] = err[-300:]
            off = row['off']
            for arm in ('on', 'on2'):
                if (off['depthMm'] is not None
                        and row[arm]['depthMm'] is not None):
                    row[f'delta{arm.capitalize()}Mm'] = (
                        row[arm]['depthMm'] - off['depthMm'])
                row[f'equalWork{arm.capitalize()}'] = (
                    off['planUnits'] is not None
                    and off['planUnits'] == row[arm]['planUnits'])
            result['rows'].append(row)
            print(f'{request} s{seed}: off={off["depthMm"]} '
                  f'on={row["on"]["depthMm"]} on2={row["on2"]["depthMm"]} '
                  f'dOn={row.get("deltaOnMm")} dOn2={row.get("deltaOn2Mm")} '
                  f'eqOn={row["equalWorkOn"]} eqOn2={row["equalWorkOn2"]} '
                  f'raceS={row["on"].get("race", {}).get("seconds")}/'
                  f'{row["on2"].get("race", {}).get("seconds")}', flush=True)

    summary = {'cells': len(result['rows'])}
    for arm in ('On', 'On2'):
        paired = [r for r in result['rows']
                  if r.get(f'equalWork{arm}') and r.get(f'delta{arm}Mm')
                  is not None]
        deltas = [r[f'delta{arm}Mm'] for r in paired]
        summary[arm] = {
            'equalWorkCells': len(paired),
            'medianDeltaMm': statistics.median(deltas) if deltas else None,
            'meanDeltaMm': statistics.fmean(deltas) if deltas else None,
            'better': sum(1 for d in deltas if d < 0),
            'worse': sum(1 for d in deltas if d > 0),
            'tied': sum(1 for d in deltas if d == 0),
        }
    for key in ('on', 'on2'):
        rows = [r[key] for r in result['rows'] if r[key].get('race')]
        summary[f'{key}RaceSeconds'] = [r['race'].get('seconds') for r in rows]
        summary[f'{key}RaceWorkUnits'] = [r['race'].get('workUnits')
                                          for r in rows]
        summary[f'{key}DrawSeconds'] = [r['draws']['seconds'] for r in rows]
        summary[f'{key}MovedOffIncumbent'] = sum(
            1 for r in rows if r['race'].get('movedOffIncumbent'))
    result['summary'] = summary
    result['boxLoad'] = runlib.LOAD
    json.dump(result, open(f'{outdir}/racebattery-{target}.json', 'w'),
              indent=1, sort_keys=True)
    print(json.dumps(summary, indent=1))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
