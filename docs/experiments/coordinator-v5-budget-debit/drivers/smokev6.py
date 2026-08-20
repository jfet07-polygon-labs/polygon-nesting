#!/usr/bin/env python3
"""One cell of the corrected battery, printed in full, before spending hours.

    smokev6.py BINARY SPEC [SEED] [REQUEST]

The point is to answer, cheaply and before the paired battery is launched, the
one question the previous round got wrong by not asking it: *does the schedule
class fire at all under this spec, and does it carry a non-zero debit?* A
battery whose arms never reach mode 34 measures nothing about a mode-34 fix,
which is exactly what `evidence/battery-fixed-sched.json` did with `v3=0`.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import runlibv6 as runlib  # noqa: E402

binary = sys.argv[1]
spec = sys.argv[2]
seed = int(sys.argv[3]) if len(sys.argv) > 3 else 1
request = sys.argv[4] if len(sys.argv) > 4 else 'mixed-61'
out = f'{runlib.OUT}/smoke/{os.path.basename(binary)}-s{seed}.json'

doc, wall, err = runlib.run(binary, request, seed, spec, out)
row = runlib.summarize('smoke', doc, wall)
portfolio = doc.get('portfolio') or {}
calls = portfolio.get('operatorCalls') or []
self_metered = [c for c in calls if c.get('selfMeteredUnits') is not None]
schedule = portfolio.get('schedule') or {}
actions = schedule.get('actions') or []
print(json.dumps({
    'spec': spec,
    'seed': seed,
    'binary': binary,
    'processSeconds': wall,
    'engineDepthMm': row.get('engineDepthMm'),
    'rawDepthMm': row.get('rawDepthMm'),
    'workUnits': row.get('workUnits'),
    'operatorCalls': len(calls),
    'mode34Calls': sum(1 for c in calls if c.get('operator') == 'mode34'),
    'selfMeteredCalls': len(self_metered),
    'selfMeteredDetail': self_metered[:6],
    'scheduleActions': [
        {k: a.get(k) for k in ('iteration', 'class', 'actualCost',
                               'meteredCost', 'selfMeteredUnits',
                               'debitedUnits', 'workUnits')}
        for a in actions if a.get('class') == 'schedule'][:6],
    'actionClasses': sorted({a.get('class') for a in actions}),
    'scheduleIterations': schedule.get('iterations'),
    'exitCause': schedule.get('exitCause'),
    'stderrTail': err[-400:] if err else '',
    'loadError': row.get('loadError'),
}, indent=1))
