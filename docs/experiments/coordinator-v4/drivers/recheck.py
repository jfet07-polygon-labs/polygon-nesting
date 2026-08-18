#!/usr/bin/env python3
"""Re-compares two already-written processes' documents, without re-running.

    recheck.py RUNDIR OUT.json

`RUNDIR` holds `<arm>-s<seed>-p0.json` and `-p1.json` pairs as `determinism.py`
wrote them. Used when the volatile-field list has to be corrected after the
runs were made, so the correction is applied to the *same* two processes rather
than to a new pair.
"""
import json
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import determinism  # noqa: E402

rundir = sys.argv[1]
rows = []
pattern = re.compile(r'^(?P<arm>v[34])-s(?P<seed>\d+)-p0\.json$')
for name in sorted(os.listdir(rundir)):
    match = pattern.match(name)
    if not match:
        continue
    left = json.load(open(f'{rundir}/{name}'))
    right = json.load(open(f'{rundir}/{name[:-7]}p1.json'))
    flat_left = determinism.flatten(left)
    flat_right = determinism.flatten(right)
    keys = sorted(set(flat_left) | set(flat_right))
    differing = [k for k in keys if flat_left.get(k) != flat_right.get(k)]
    rows.append({
        'arm': match['arm'], 'seed': int(match['seed']),
        'fieldsCompared': len(keys),
        'differingFields': len(differing),
        'differing': differing[:20],
        'rawDepthMm': [d.get('portfolio', {}).get('incumbent', {})
                       .get('rawDepthMm') for d in (left, right)],
        'workUnits': [d.get('portfolio', {}).get('workUnits')
                      for d in (left, right)],
    })
print(json.dumps(rows, indent=1))
if len(sys.argv) > 2:
    json.dump(rows, open(sys.argv[2], 'w'), indent=1)
