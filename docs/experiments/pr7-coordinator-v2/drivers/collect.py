#!/usr/bin/env python3
"""Copies the batteries' evidence into this experiment directory.

The raw battery files carry a per-run incumbent series each, so they are
several hundred kilobytes apiece; this keeps every row and every operator call
and drops only the traces' point-by-point series, which are reproducible from
the driver and are not what any claim rests on. The milestone table the README
quotes is computed here from the full series before it is dropped.
"""
import json
import os
import shutil
import statistics
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib  # noqa: E402

HERE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
EVIDENCE = f'{HERE}/evidence'
MILESTONES = (200.0, 190.0, 185.0, 182.0, 180.0, 179.69, 179.0, 178.0, 177.0,
              175.0, 174.5, 100.0, 71.0, 70.9, 70.8, 70.75, 70.73)

BATTERIES = {
    'ten': 'battery-ten-second-arms.json',
    'curve': 'battery-anytime-curve.json',
    'thirty': 'battery-thirty-second-triggers.json',
    'final': 'battery-final-mixed61.json',
    'shapes17': 'battery-shapes17.json',
    'triangle20': 'battery-triangle20.json',
}


def first_at_or_below(series, threshold):
    for point in series:
        depth = point.get('rawDepthMm')
        if depth is None:
            depth = point['depthMm']
        if depth <= threshold:
            return point['t']
    return None


def milestones(rows):
    out = {}
    for arm in sorted({row['arm'] for row in rows}):
        out[arm] = {}
        for seed in sorted({row['seed'] for row in rows}):
            picked = [row for row in rows
                      if row['arm'] == arm and row['seed'] == seed]
            per = {}
            for threshold in MILESTONES:
                times = [first_at_or_below(row.get('incumbentSeries', []),
                                           threshold) for row in picked]
                hit = [value for value in times if value is not None]
                if not hit:
                    continue
                per[str(threshold)] = {
                    'medianSeconds': round(statistics.median(hit), 3),
                    'rounds': f'{len(hit)}/{len(times)}',
                }
            out[arm][str(seed)] = per
    return out


def main():
    os.makedirs(EVIDENCE, exist_ok=True)
    for name, target in BATTERIES.items():
        path = f'{lib.OUT}/{name}/battery.json'
        if not os.path.exists(path):
            print(f'skip {name}')
            continue
        data = json.load(open(path))
        data['milestoneSeconds'] = milestones(data['rows'])
        for row in data['rows']:
            row.pop('incumbentSeries', None)
        json.dump(data, open(f'{EVIDENCE}/{target}', 'w'), indent=1)
        print(f'{target}: {len(data["rows"])} rows')
    for units, request, target in (
            (40000000, 'mixed-61', 'determinism-40M-mixed61.json'),
            (20000000, 'mixed-61', 'determinism-20M-binding-mixed61.json'),
            (20000000, 'triangle-20', 'determinism-20M-triangle20.json')):
        path = f'{lib.OUT}/determinism-{units}-{request}/determinism.json'
        if os.path.exists(path):
            shutil.copy(path, f'{EVIDENCE}/{target}')
            print(target)
    for label in ('pristine', 'worktree'):
        path = f'{lib.OUT}/gates/{label}/gates-{label}.json'
        if os.path.exists(path):
            shutil.copy(path, f'{EVIDENCE}/gates-{label}.json')
            print(f'gates-{label}.json')


if __name__ == '__main__':
    main()
