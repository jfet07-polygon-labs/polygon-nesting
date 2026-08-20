#!/usr/bin/env python3
"""What each arm of a paired battery *really* spent, in one currency.

    truecost.py BATTERY.json [BATTERY.json ...]

The fixed and unfixed arms of this battery cannot be compared on their reported
`workUnits` totals, because that is precisely the number the fix changes. They
can be compared on *true* work, and both documents carry enough to recover it:

* fixed - `portfolio.workUnits` already folds the debit in, so the reported
  total is the true total;
* unfixed - the schedule action rows carry `actualCost` (which is
  `max(metered, selfMetered)`, the price the ranking already used) and
  `meteredCost` (the coordinator's own counter delta). Their difference,
  summed over the run's schedule actions, is exactly the debit the unfixed
  binary computed and threw away. Adding it back gives what that run would
  have been charged had the meter been honest.

This is Sol review 6 §1 finding 2's counterfactual - "il totale corretto
sarebbe circa 41.19M, 41.81M e 51.33M" - recomputed on this round's own runs
rather than on the pinned v4 trace, and it is what makes the depth comparison
between the arms readable: an unfixed run that overran its nominal budget
bought its depth with work the fixed run was not given.
"""
import json
import statistics
import sys


def true_cost(row):
    """(reported, counterfactual debit, true total) for one run row."""
    reported = row.get('workUnits')
    if reported is None:
        return None, None, None
    charged = sum(c['debitedUnits'] or 0 for c in row.get('debitCalls') or [])
    if charged:
        # A fixed run: the debit is already inside `reported`.
        return reported, 0, reported
    counterfactual = 0
    for action in row.get('scheduleActions') or []:
        actual, metered = action.get('actualCost'), action.get('meteredCost')
        if actual is None or metered is None:
            continue
        counterfactual += max(0, round(actual - metered))
    return reported, counterfactual, reported + counterfactual


def main():
    out = {}
    for path in sys.argv[1:]:
        doc = json.load(open(path))
        name = doc['name']
        budget = None
        if 'work=' in doc['spec']:
            budget = int(doc['spec'].split('work=')[1].split(',')[0])
        rows = []
        for row in doc['rows']:
            reported, counterfactual, total = true_cost(row)
            rows.append({
                'arm': row['arm'], 'seed': row['seed'], 'round': row['round'],
                'depthMm': row['engineDepthMm'],
                'reportedWorkUnits': reported,
                'debitAlreadyCharged':
                    sum(c['debitedUnits'] or 0
                        for c in row.get('debitCalls') or []),
                'counterfactualDebit': counterfactual,
                'trueWorkUnits': total,
                'overrunUnits':
                    None if (total is None or budget is None)
                    else total - budget,
                'overrunFraction':
                    None if (total is None or budget is None)
                    else round(total / budget - 1.0, 6),
            })
        summary = {}
        for arm in sorted({r['arm'] for r in rows}):
            arm_rows = [r for r in rows if r['arm'] == arm
                        and r['trueWorkUnits'] is not None]
            if not arm_rows:
                continue
            summary[arm] = {
                'medianDepthMm':
                    statistics.median(r['depthMm'] for r in arm_rows),
                'medianReported':
                    statistics.median(r['reportedWorkUnits']
                                      for r in arm_rows),
                'medianTrue':
                    statistics.median(r['trueWorkUnits'] for r in arm_rows),
                'maxTrue': max(r['trueWorkUnits'] for r in arm_rows),
                'maxOverrunFraction':
                    max(r['overrunFraction'] for r in arm_rows)
                    if budget else None,
                'runsOverBudget':
                    sum(1 for r in arm_rows if r['overrunUnits']
                        and r['overrunUnits'] > 0) if budget else None,
                'runs': len(arm_rows),
            }
        out[name] = {'budget': budget, 'spec': doc['spec'],
                     'rows': rows, 'summary': summary}
    print(json.dumps(out, indent=1))


if __name__ == '__main__':
    main()
