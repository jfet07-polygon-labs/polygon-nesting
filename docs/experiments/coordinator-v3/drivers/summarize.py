#!/usr/bin/env python3
"""The tables this stage reports, from one battery document.

    summarize.py BATTERY.json [OUT.json]

Prints the per-seed depth table, the paired v3-minus-v2 deltas per budget, the
process-wall table, the per-class economics, and - for every v3 arm - whether
its best published depth was reached *in schedule* or by the drain.
"""
import json
import statistics
import sys

document = json.load(open(sys.argv[1]))
rows = document['rows']
labels = [arm['label'] for arm in document['arms']]
seeds = sorted({row['seed'] for row in rows})
rounds = sorted({row['round'] for row in rows})
by_key = {(row['arm'], row['seed'], row['round']): row for row in rows}

out = {'request': document['request'], 'rounds': len(rounds),
       'seeds': seeds, 'perSeed': {}, 'paired': {}, 'wall': {},
       'classes': {}, 'inSchedule': {}}

for label in labels:
    out['perSeed'][label] = {}
    for seed in seeds:
        values = [by_key[(label, seed, r)].get('rawDepthMm')
                  for r in rounds if (label, seed, r) in by_key]
        values = [v for v in values if v is not None]
        out['perSeed'][label][str(seed)] = {
            'best': min(values) if values else None,
            'median': statistics.median(values) if values else None,
            'worst': max(values) if values else None,
            'all': values,
        }

# Paired v3-minus-v2 per budget tier, matched by the numeric suffix.
tiers = sorted({label[len('v2at'):] for label in labels
                if label.startswith('v2at')}, key=int)
for tier in tiers:
    left, right = f'v2at{tier}', f'v3at{tier}'
    if left not in labels or right not in labels:
        continue
    deltas = []
    for seed in seeds:
        for r in rounds:
            a, b = by_key.get((left, seed, r)), by_key.get((right, seed, r))
            if not a or not b:
                continue
            if a.get('rawDepthMm') is None or b.get('rawDepthMm') is None:
                continue
            deltas.append(b['rawDepthMm'] - a['rawDepthMm'])
    if deltas:
        out['paired'][tier] = {
            'medianMm': statistics.median(deltas),
            'minMm': min(deltas), 'maxMm': max(deltas),
            'v3Better': sum(1 for d in deltas if d < -1e-9),
            'v3Worse': sum(1 for d in deltas if d > 1e-9),
            'equal': sum(1 for d in deltas if abs(d) <= 1e-9),
            'rounds': len(deltas),
        }

for label in labels:
    walls = [row['processSeconds'] for row in rows if row['arm'] == label]
    coordinator = [row['coordinatorSeconds'] for row in rows
                   if row['arm'] == label and 'coordinatorSeconds' in row]
    out['wall'][label] = {
        'processMedian': statistics.median(walls),
        'processMax': max(walls),
        'coordinatorMedian': statistics.median(coordinator)
        if coordinator else None,
    }

for row in rows:
    schedule = row.get('schedule')
    if not schedule:
        continue
    for entry in schedule['classes']:
        key = f"{row['arm']}/{entry['class']}"
        agg = out['classes'].setdefault(
            key, {'actions': 0, 'publications': 0, 'workUnits': 0,
                  'seconds': 0.0, 'deltaRawMm': 0.0, 'runs': 0,
                  'firstEstimateRatio': []})
        agg['actions'] += entry['actions']
        agg['publications'] += entry['publications']
        agg['workUnits'] += entry['workUnits']
        agg['seconds'] += entry['seconds']
        agg['deltaRawMm'] += entry['deltaRawMm']
        agg['runs'] += 1
        est, act = entry['firstEstimatedCost'], entry['firstActualCost']
        if est and act:
            agg['firstEstimateRatio'].append(act / est)
for key, agg in out['classes'].items():
    ratios = agg.pop('firstEstimateRatio')
    agg['firstActualOverEstimateMedian'] = \
        statistics.median(ratios) if ratios else None
    agg['firstActualOverEstimateMax'] = max(ratios) if ratios else None
    agg['mmPerMegaUnit'] = (agg['deltaRawMm'] / (agg['workUnits'] / 1e6)
                            if agg['workUnits'] else None)

# In schedule, or by the drain? The last publication's phase answers it.
for row in rows:
    publications = row.get('publications')
    if not publications:
        continue
    key = row['arm']
    entry = out['inSchedule'].setdefault(
        key, {'runs': 0, 'lastPublicationPhase': {}})
    entry['runs'] += 1
    phase = publications[-1]['phase']
    entry['lastPublicationPhase'][phase] = \
        entry['lastPublicationPhase'].get(phase, 0) + 1

print(json.dumps(out, indent=1))
if len(sys.argv) > 2:
    json.dump(out, open(sys.argv[2], 'w'), indent=1)
