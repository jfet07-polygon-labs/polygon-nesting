#!/usr/bin/env python3
"""The gate's tables: the paired delta per cell, and the cost per published mm.

    python3 summarize.py GATE.json [OUT.json]

The statistic is the paired delta of the best exact-valid raw source depth
against the parent, per cell, with the parent as the floor for every arm - so a
publication is a strictly negative delta and an arm that found nothing is a
zero, never a missing row.

Cost is reported in the portfolio's own work units, twice over, because the two
readings answer different questions and disagree by a knowable amount: the
`operator` reading is the whole-process counter minus the identical mode-0
preamble the same cell measured, and is what an operator costs a coordinator;
the `self` reading is the schedule's own deterministic meter, and is what a
work-capped arm is capped on. Wall seconds are reported and never compared:
the schedule is one lane and the mode-26 pipeline is eight, so equal work is
emphatically not equal wall here, and that is the port's largest open cost.
"""
import json
import statistics
import sys


def median(values):
    return statistics.median(values) if values else None


def cell_delta(arm, parent_depth):
    """The arm's improvement on its parent, floored at zero."""
    if not arm or arm.get('exactValid') is not True:
        return 0.0
    depth = arm.get('rawSourceDepthMm')
    if depth is None:
        return 0.0
    return max(parent_depth - depth, 0.0)


def main():
    gate = json.load(open(sys.argv[1]))
    arms = []
    for cell in gate['cells']:
        arms.extend(k for k in cell['arms'] if k not in arms)
    measured = [a for a in arms if a != 'preamble']
    rows = []
    for cell in gate['cells']:
        parent = cell['parentRawDepthMm']
        preamble = (cell['arms'].get('preamble') or {}).get('processWorkUnits')
        row = {'seed': cell['seed'], 'parentRawDepthMm': parent,
               'inBand': 174.0 <= parent <= 179.5,
               'preambleWorkUnits': preamble, 'arms': {}}
        for arm in measured:
            found = cell['arms'].get(arm) or {}
            if 'error' in found:
                row['arms'][arm] = {'error': found['error'][:200]}
                continue
            delta = cell_delta(found, parent)
            operator_work = None
            if preamble is not None and found.get('processWorkUnits') is not None:
                operator_work = max(found['processWorkUnits'] - preamble, 0)
            schedule = found.get('schedule') or {}
            entry = {
                'rawSourceDepthMm': found.get('rawSourceDepthMm'),
                'deltaMm': delta,
                'published': delta > 0.0,
                'exactValid': found.get('exactValid'),
                'contractValid': found.get('contractValid'),
                'operatorWorkUnits': operator_work,
                'selfWorkUnits': schedule.get('work_units')
                or schedule.get('workUnits'),
                'processWallSeconds': found.get('processWallSeconds'),
                'mmPerMillionUnits': (delta / (operator_work / 1e6))
                if operator_work else None,
            }
            if schedule:
                entry['scheduleExit'] = schedule.get('exitCause')
                entry['steps'] = schedule.get('stepsTaken')
                entry['confirmationsAttempted'] = schedule.get(
                    'confirmationsAttempted')
                entry['confirmationsRefused'] = schedule.get(
                    'confirmationsRefused')
                entry['confirmationsSkippedInfeasible'] = schedule.get(
                    'confirmationsSkippedInfeasible')
                entry['microLegalizationsAttempted'] = schedule.get(
                    'microLegalizationsAttempted')
                entry['microLegalizationsAccepted'] = schedule.get(
                    'microLegalizationsAccepted')
                entry['rollbacks'] = schedule.get('rollbacks')
                entry['confirmationMs'] = schedule.get('confirmationMs')
                entry['repairMs'] = schedule.get('repairMs')
                entry['parentProxyFeasible'] = schedule.get(
                    'parentProxyFeasible')
                entry['parentCollisionPairs'] = schedule.get(
                    'parentCollisionPairs')
                entry['parentBoundaryViolations'] = schedule.get(
                    'parentBoundaryViolations')
            row['arms'][arm] = entry
        rows.append(row)

    summary = {'arms': measured, 'cells': len(rows), 'perArm': {}, 'paired': {}}
    for arm in measured:
        deltas = [r['arms'][arm]['deltaMm'] for r in rows
                  if 'deltaMm' in r['arms'].get(arm, {})]
        works = [r['arms'][arm]['operatorWorkUnits'] for r in rows
                 if r['arms'].get(arm, {}).get('operatorWorkUnits')]
        walls = [r['arms'][arm]['processWallSeconds'] for r in rows
                 if r['arms'].get(arm, {}).get('processWallSeconds')]
        published = [d for d in deltas if d > 0.0]
        summary['perArm'][arm] = {
            'cells': len(deltas),
            'publishes': len(published),
            'medianDeltaMm': median(deltas),
            'meanDeltaMm': (sum(deltas) / len(deltas)) if deltas else None,
            'bestDeltaMm': max(deltas) if deltas else None,
            'medianOperatorWorkUnits': median(works),
            'totalDeltaMm': sum(deltas),
            'totalOperatorWorkUnits': sum(works) if works else None,
            'mmPerMillionUnitsPooled': (sum(deltas) / (sum(works) / 1e6))
            if works and sum(works) else None,
            'medianProcessWallSeconds': median(walls),
        }
    # Every schedule arm against the legacy control, paired per cell.
    for arm in measured:
        if arm == 'm26':
            continue
        pairs = [(r['seed'],
                  r['arms'][arm]['deltaMm'] - r['arms']['m26']['deltaMm'])
                 for r in rows
                 if 'deltaMm' in r['arms'].get(arm, {})
                 and 'deltaMm' in r['arms'].get('m26', {})]
        values = [p[1] for p in pairs]
        summary['paired'][f'{arm}-minus-m26'] = {
            'cells': len(values),
            'medianDepthAdvantageMm': median(values),
            'cellsWhereScheduleWins': sum(1 for v in values if v > 0),
            'cellsTied': sum(1 for v in values if v == 0),
            'cellsWhereLadderWins': sum(1 for v in values if v < 0),
            'perCell': pairs,
        }
    out = {'source': sys.argv[1], 'summary': summary, 'rows': rows}
    print(json.dumps(out, indent=1))
    if len(sys.argv) > 2:
        json.dump(out, open(sys.argv[2], 'w'), indent=1)


if __name__ == '__main__':
    main()
