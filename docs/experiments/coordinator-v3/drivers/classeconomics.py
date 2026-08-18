#!/usr/bin/env python3
"""The per-class cost-and-yield table, pooled over a work-budget battery.

The ledger's Part 1 §5 table for the v3 queue: calls, publications, work units
and Δraw per million evaluations, per action class, measured in the runs the
queue actually made rather than assumed from the prior.

    classeconomics.py WORKQUALITY.json WORK [OUT.json]
"""
import json
import sys

document = json.load(open(sys.argv[1]))
work = int(sys.argv[2])
pooled = {}
per_seed = {}
for row in document['rows']:
    if row['arm'] != 'v3' or row['work'] != work:
        continue
    schedule = row.get('schedule')
    if not schedule:
        continue
    for entry in schedule['classes']:
        agg = pooled.setdefault(entry['class'], {
            'actions': 0, 'publications': 0, 'workUnits': 0, 'seconds': 0.0,
            'deltaRawMm': 0.0, 'firstEstimated': [], 'firstActual': []})
        agg['actions'] += entry['actions']
        agg['publications'] += entry['publications']
        agg['workUnits'] += entry['workUnits']
        agg['seconds'] += entry['seconds']
        agg['deltaRawMm'] += entry['deltaRawMm']
        agg['firstEstimated'].append(entry['firstEstimatedCost'])
        agg['firstActual'].append(entry['firstActualCost'])
        per_seed.setdefault(str(row['seed']), {})[entry['class']] = {
            'actions': entry['actions'],
            'publications': entry['publications'],
            'workUnits': entry['workUnits'],
            'deltaRawMm': entry['deltaRawMm'],
            'firstEstimatedCost': entry['firstEstimatedCost'],
            'firstActualCost': entry['firstActualCost'],
            'firstActualOverEstimate':
                entry['firstActualCost'] / entry['firstEstimatedCost']
                if entry['firstEstimatedCost'] else None,
        }
for name, agg in pooled.items():
    agg['deltaRawPerMegaUnit'] = (agg['deltaRawMm'] / (agg['workUnits'] / 1e6)
                                  if agg['workUnits'] else None)
    agg['workUnitsPerAction'] = (agg['workUnits'] / agg['actions']
                                 if agg['actions'] else None)
    agg['deltaRawPerAction'] = (agg['deltaRawMm'] / agg['actions']
                                if agg['actions'] else None)
result = {'work': work, 'pooled': pooled, 'perSeed': per_seed}
print(json.dumps(result, indent=1))
if len(sys.argv) > 3:
    json.dump(result, open(sys.argv[3], 'w'), indent=1)
