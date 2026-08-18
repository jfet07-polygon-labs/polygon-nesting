#!/usr/bin/env python3
"""The schedule's depth-versus-work curve, and the schedule read at the
control's *actual* spend.

    python3 curve.py GATE.json GATEDIR ARM [OUT.json]

The gate pairs the arms on an equal *allowance*. That is the right primary
statistic - it is what the opportunity ledger's A/B/C used, and an operator that
stops early should be rewarded for it rather than padded out - but it leaves one
question open that the schedule, unlike a ladder, can answer: what had it
reached at the moment the control stopped?

The schedule records the candidate queries every step cost and the raw depth
every accepted confirmation measured, so the curve is a reconstruction of
something the run really did rather than an interpolation. Each row reports the
best confirmed depth at or before the control's own spend, in the same work
unit, from the same parent.
"""
import json
import os
import sys


def schedule_of(path):
    doc = json.load(open(path))
    pop = ((doc.get('relaxedDiagnostics') or {})
           .get('coupledDynamicSeparator') or {}).get(
               'persistentVacancyPopulation') or {}
    return pop.get('compressionSchedule')


def curve(schedule, pairs_per_confirmation):
    """(cumulative work units, best raw depth so far) after every step."""
    points = []
    queries = 0
    confirmations = 0
    best = None
    for row in schedule.get('steps') or []:
        queries += row.get('candidateQueries', 0)
        if row.get('rawDepthMm') is not None:
            confirmations += 1
            best = row['rawDepthMm'] if best is None else min(
                best, row['rawDepthMm'])
        if row.get('confirmationRefused'):
            confirmations += 1
        work = queries + 5 * pairs_per_confirmation * confirmations
        points.append((work, best))
    return points


def at_spend(points, budget):
    best = None
    for work, depth in points:
        if work > budget:
            break
        if depth is not None:
            best = depth if best is None else min(best, depth)
    return best


def main():
    gate = json.load(open(sys.argv[1]))
    gatedir = sys.argv[2]
    arm = sys.argv[3] if len(sys.argv) > 3 else 'sched'
    pairs = 61 * 60 // 2
    rows = []
    for cell in gate['cells']:
        seed = cell['seed']
        parent = cell['parentRawDepthMm']
        control = cell['arms'].get('m26') or {}
        preamble = (cell['arms'].get('preamble') or {}).get('processWorkUnits')
        control_work = None
        if preamble is not None and control.get('processWorkUnits') is not None:
            control_work = max(control['processWorkUnits'] - preamble, 0)
        row = {'seed': seed, 'parentRawDepthMm': parent,
               'controlRawDepthMm': control.get('rawSourceDepthMm'),
               'controlWorkUnits': control_work,
               'controlDeltaMm': max(
                   parent - (control.get('rawSourceDepthMm') or parent), 0.0)}
        path = f'{gatedir}/seed{seed}-{arm}.json'
        if not os.path.exists(path):
            rows.append(row)
            continue
        schedule = schedule_of(path)
        if not schedule:
            rows.append(row)
            continue
        points = curve(schedule, pairs)
        # The first depth an exact validator accepted after the lane's entry
        # transform. `initialize_complete_state` snaps every rotation onto the
        # structured surrogate's 2.5-degree grid, so the layout the schedule
        # starts from is not the layout in the fixture; this is the first
        # measurement of what that costs, and it is an upper bound on the
        # snap alone because the steps before it also ran repair.
        first = next((s for s in (schedule.get('steps') or [])
                      if s.get('rawDepthMm') is not None), None)
        if first is not None:
            row['firstConfirmedStep'] = first['step']
            row['firstConfirmedRawDepthMm'] = first['rawDepthMm']
            row['entryLossMm'] = first['rawDepthMm'] - parent
        row['parentProxyCollisionPairs'] = schedule.get('parentCollisionPairs')
        row['parentProxyBoundaryViolations'] = schedule.get(
            'parentBoundaryViolations')
        row['scheduleSteps'] = len(points)
        row['scheduleFinalWorkUnits'] = points[-1][0] if points else 0
        row['scheduleBestRawDepthMm'] = at_spend(points, float('inf'))
        if control_work:
            matched = at_spend(points, control_work)
            row['scheduleAtControlSpendRawDepthMm'] = matched
            row['scheduleAtControlSpendDeltaMm'] = max(
                parent - (matched if matched is not None else parent), 0.0)
            row['matchedSpendAdvantageMm'] = (
                row['scheduleAtControlSpendDeltaMm'] - row['controlDeltaMm'])
        # A decimated curve, so the evidence carries the shape without carrying
        # every step of every cell.
        stride = max(1, len(points) // 60)
        row['curve'] = [{'workUnits': w, 'bestRawDepthMm': d}
                        for w, d in points[::stride]]
        rows.append(row)
    advantages = [r['matchedSpendAdvantageMm'] for r in rows
                  if 'matchedSpendAdvantageMm' in r]
    advantages.sort()
    summary = {
        'cells': len(advantages),
        'medianMatchedSpendAdvantageMm':
            (advantages[len(advantages) // 2] if advantages else None),
        'cellsWhereScheduleWins': sum(1 for v in advantages if v > 0),
        'cellsTied': sum(1 for v in advantages if v == 0),
        'cellsWhereLadderWins': sum(1 for v in advantages if v < 0),
    }
    out = {'arm': arm, 'summary': summary, 'rows': rows}
    print(json.dumps(out, indent=1))
    if len(sys.argv) > 4:
        json.dump(out, open(sys.argv[4], 'w'), indent=1)


if __name__ == '__main__':
    main()
