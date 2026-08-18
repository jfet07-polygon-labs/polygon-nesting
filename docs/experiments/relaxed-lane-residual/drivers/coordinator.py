#!/usr/bin/env python3
"""One coordinator run from the bare request, decomposed.

    python3 coordinator.py <wallMs> <label> <binary> [<label> <binary> ...]

The coordinator schedules its own operators, so this is the "what does the
relaxed lane cost at a budget" measurement rather than a replay of a pinned
parent. Profiling is on, so the milliseconds are a decomposition and the
counters are exact; neither is a wall-clock claim.
"""
import json
import os
import subprocess
import sys
import time

ROOT = '/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_028f78e1-e59-2'
REQ = f'{ROOT}/tests/fixtures/mixed-61/mixed61-request-exact-clearance.json'
ARGS = ('1 4 0 0 0 0 1 0 0 1 1 0 16 4 8 0 0 5 5 24 8 40 10 10 5 5 '
        '0.005 0.001 1 6 0 0 0 structured 0 10 1 0 0 0 0').split()

wall_ms = sys.argv[1]
spec = f'wall={wall_ms},cells=13:15:17:19'
outdir = '/tmp/rl/coordinator'
os.makedirs(outdir, exist_ok=True)
report = {}
for index in range(2, len(sys.argv), 2):
    label, binary = sys.argv[index], sys.argv[index + 1]
    command = ([binary, REQ] + ARGS + ['0', '', '', '', '0.002', spec])
    env = dict(os.environ)
    env['POLYGON_NESTING_PROFILE'] = '1'
    path = f'{outdir}/{label}.json'
    started = time.monotonic()
    with open(path, 'w') as handle:
        proc = subprocess.run(command, stdout=handle, stderr=subprocess.PIPE,
                              check=False, env=env)
    elapsed = time.monotonic() - started
    try:
        doc = json.load(open(path))
    except json.JSONDecodeError:
        print(json.dumps({label: {'error': (proc.stderr or b'').decode()[-1500:]}}))
        continue
    profile = doc.get('searchProfile') or {}
    portfolio = doc.get('portfolio') or {}
    report[label] = {
        'spec': spec,
        'processWallSeconds': elapsed,
        'coordinatorSeconds': portfolio.get('elapsedSeconds'),
        'rawDepthMm': (portfolio.get('incumbent') or {}).get('rawDepthMm'),
        'dualGateValid': (portfolio.get('incumbent') or {}).get('dualGateValid'),
        'threads': profile.get('threads'),
        'leafMilliseconds': profile.get('leafMilliseconds'),
        'counters': profile.get('counters'),
        'phases': {
            row['phase']: {'ms': row.get('milliseconds'),
                           'calls': row.get('calls'),
                           'enclosing': row.get('enclosing')}
            for row in sorted(profile.get('phases') or [],
                              key=lambda r: -(r.get('milliseconds') or 0))
        },
    }
    print(json.dumps({label: {k: v for k, v in report[label].items()
                              if k not in ('phases', 'counters')}}), flush=True)
json.dump(report, open(f'{outdir}/coordinator-{wall_ms}.json', 'w'), indent=1)
