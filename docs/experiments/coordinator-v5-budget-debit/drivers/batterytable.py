#!/usr/bin/env python3
"""One row per cell of a paired battery, as a markdown table.

    batterytable.py BATTERY.json

Columns are the ones the budget-debit question turns on: the depth, the
coordinator's reported `workUnits` total (which under the fix already contains
the debit), the debit itself, the *global* part (total minus debit, i.e. the
number the old accounting reported), how many schedule actions the run bought,
and why the schedule stopped.
"""
import json
import sys


def rows(doc):
    for row in doc['rows']:
        debit = sum(c['debitedUnits'] or 0 for c in row['debitCalls'])
        total = row.get('workUnits')
        actions = row.get('scheduleActions') or []
        yield {
            'arm': row['arm'], 'seed': row['seed'], 'round': row['round'],
            'depth': row['engineDepthMm'],
            'workUnits': total,
            'debited': debit,
            'globalUnits': (total - debit) if total is not None else None,
            'scheduleActions': sum(1 for a in actions if a['class'] == 'schedule'),
            'actions': len(actions),
            'iterations': row.get('scheduleIterations'),
            'exit': row.get('scheduleExit'),
            'seconds': round(row['processSeconds'], 2),
        }


def main():
    doc = json.load(open(sys.argv[1]))
    print(f"### {doc['name']} — spec `{doc['spec']}`")
    print()
    print('| arm | seed | round | depth mm | workUnits | debited | global |'
          ' m34 actions | actions | exit | process s |')
    print('|---|---|---|---|---|---|---|---|---|---|---|')
    # A wall-budget run reports no work units at all (the counter is off), so
    # every units column is formatted through `num` rather than assuming an
    # integer is there to comma-group.
    def num(value):
        return '-' if value is None else f'{value:,}'

    for r in sorted(rows(doc), key=lambda r: (r['seed'], r['round'], r['arm'])):
        print(f"| {r['arm']} | {r['seed']} | {r['round']} | {r['depth']} |"
              f" {num(r['workUnits'])} | {num(r['debited'])} |"
              f" {num(r['globalUnits'])} |"
              f" {r['scheduleActions']} | {r['actions']} | {r['exit']} |"
              f" {r['seconds']} |")


if __name__ == '__main__':
    main()
