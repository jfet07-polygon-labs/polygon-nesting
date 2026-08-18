#!/usr/bin/env python3
"""Merged-HEAD v3 against v4 at identical *work* budgets.

Work-budget mode is deterministic and load independent, so one run per
(arm, seed, budget) is the whole measurement and a shared box cannot move it.
This is the quality comparison; the wall curve is `battery.py`'s job.

    workquality.py NAME REQUEST SEEDS WORKS
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import runlib  # noqa: E402

name = sys.argv[1]
request = sys.argv[2]
seeds = [int(v) for v in sys.argv[3].split(',')]
works = [int(v) for v in sys.argv[4].split(',')]

out_dir = f'{runlib.OUT}/{name}'
result = {'name': name, 'request': request, 'seeds': seeds, 'works': works,
          'binary': runlib.BIN, 'rows': []}
for work in works:
    for seed in seeds:
        for label, extra in (('v3', 'sched=0,barren=0,divq=0'), ('v4', '')):
            spec = runlib.spec_for(seed, 'work', work, True, extra)
            tag = f'{label}-w{work}-s{seed}'
            doc, wall, err = runlib.run(runlib.BIN, request, seed, spec,
                                        f'{out_dir}/runs/{tag}.json')
            row = runlib.summarize(tag, doc, wall)
            row.update({'arm': label, 'seed': seed, 'work': work,
                        'spec': spec})
            result['rows'].append(row)
            print(f"{tag}: raw={row.get('rawDepthMm')} "
                  f"dualGate={row.get('dualGateValid')} "
                  f"spent={row.get('workUnits')} wall={wall:.2f}s", flush=True)

# The paired table, and the in-schedule question: is the best published depth
# reached by an operator phase rather than by the drain?
table = {}
for row in result['rows']:
    key = f"w{row['work']}/s{row['seed']}"
    entry = table.setdefault(key, {})
    entry[row['arm']] = {
        'rawDepthMm': row.get('rawDepthMm'),
        'dualGateValid': row.get('dualGateValid'),
        'workUnits': row.get('workUnits'),
        'processSeconds': row['processSeconds'],
        'incumbentSource': row.get('incumbentSource'),
        'finalPublicationPhase': (row.get('publications') or [{}])[-1].get(
            'phase') if row.get('publications') else None,
        'publications': len(row.get('publications') or []),
        'scheduleIterations': (row.get('schedule') or {}).get('iterations'),
        'scheduleExit': (row.get('schedule') or {}).get('exitCause'),
    }
for key, entry in table.items():
    if 'v3' in entry and 'v4' in entry and entry['v3']['rawDepthMm'] \
            and entry['v4']['rawDepthMm']:
        entry['deltaMm'] = entry['v4']['rawDepthMm'] - entry['v3']['rawDepthMm']
result['paired'] = table
os.makedirs(out_dir, exist_ok=True)
json.dump(result, open(f'{out_dir}/workquality.json', 'w'), indent=1)
print(json.dumps(table, indent=1))
