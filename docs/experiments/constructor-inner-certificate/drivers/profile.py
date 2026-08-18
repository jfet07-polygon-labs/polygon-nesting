#!/usr/bin/env python3
"""One profiled gate-1 run per binary, reported as a leaf-phase table.

    python3 profile.py <label> <binary> [<label> <binary> ...]

`search-profiling` costs about 4.5% of a mode-20 stream, so these numbers are a
*decomposition*, never a wall-clock claim; the paired A/B is the wall-clock
claim.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib  # noqa: E402

outdir = '/var/lib/t3/tmp/cinner/profile'
gate = next(g for g in lib.GATES if g[0] == 'g1')
report = {}
for index in range(1, len(sys.argv), 2):
    label, binary = sys.argv[index], sys.argv[index + 1]
    doc, wall, _ = lib.run_gate(binary, gate, outdir,
                                env={'POLYGON_NESTING_PROFILE': '1'},
                                label=f'{label}-')
    profile = doc.get('searchProfile') or {}
    phases = profile.get('phases') or []
    report[label] = {
        'engineElapsedSeconds': lib.engine_seconds(doc),
        'processWallSeconds': wall,
        'leafMilliseconds': profile.get('leafMilliseconds'),
        'counters': profile.get('counters'),
        'gateHit': lib.gate_check(gate, doc)['hit'],
        'phases': {
            row['phase']: {
                'ms': row.get('milliseconds'),
                'calls': row.get('calls'),
                'leafSharePercent': row.get('leafSharePercent'),
            }
            for row in sorted(phases, key=lambda r: -(r.get('milliseconds') or 0))
        },
    }
    print(json.dumps({label: {k: v for k, v in report[label].items()
                              if k != 'phases'}}), flush=True)
print(json.dumps(report, indent=1))
os.makedirs(outdir, exist_ok=True)
json.dump(report, open(f'{outdir}/profile.json', 'w'), indent=1)
