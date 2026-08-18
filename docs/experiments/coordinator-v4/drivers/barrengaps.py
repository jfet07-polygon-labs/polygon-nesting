#!/usr/bin/env python3
"""The distribution of *barren runs* in the v3 action loop.

A barren run is a maximal streak of consecutive actions that published nothing.
It is the statistic a global patience rule would have to be sized from, and
this stage measures it rather than guessing it: the longest barren run that was
followed by a publication is a floor on any patience that must not destroy this
stage's own headline gain.

    barrengaps.py BATTERY.json [BATTERY.json ...] [--out OUT.json]
"""
import json
import sys

out_path = None
paths = list(sys.argv[1:])
if '--out' in paths:
    index = paths.index('--out')
    out_path = paths[index + 1]
    paths = paths[:index] + paths[index + 2:]

rows = []
for path in paths:
    document = json.load(open(path))
    request = document['request']
    for row in document['rows']:
        schedule = row.get('schedule')
        if not schedule:
            continue
        streak = 0
        productive = []          # barren runs that ended in a publication
        for action in schedule['actions']:
            if action['publications'] > 0:
                if streak:
                    productive.append(streak)
                streak = 0
            else:
                streak += 1
        rows.append({
            'request': request, 'arm': row['arm'], 'seed': row['seed'],
            'round': row['round'],
            'actions': len(schedule['actions']),
            'trailingBarren': streak,
            'productiveBarrenRuns': productive,
            'longestProductiveBarrenRun': max(productive) if productive else 0,
        })

by_arm = {}
for row in rows:
    key = f"{row['request']}/{row['arm']}"
    entry = by_arm.setdefault(key, {'runs': 0, 'actions': 0,
                                    'longestProductiveBarrenRun': 0,
                                    'maxTrailingBarren': 0})
    entry['runs'] += 1
    entry['actions'] += row['actions']
    entry['longestProductiveBarrenRun'] = max(
        entry['longestProductiveBarrenRun'], row['longestProductiveBarrenRun'])
    entry['maxTrailingBarren'] = max(entry['maxTrailingBarren'],
                                     row['trailingBarren'])
document = {'byArm': by_arm, 'rows': rows}
print(json.dumps(document, indent=1))
if out_path:
    json.dump(document, open(out_path, 'w'), indent=1)
