#!/usr/bin/env python3
"""Full leaf-phase decomposition of one gate stream, per binary.

    python3 decompose.py <gate> <label> <binary> [<label> <binary> ...]

Reports every phase, every counter and the thread count, so the enclosing
phases can be reconciled against the leaves they contain. Phase milliseconds
are summed across the lane threads; the stream's wall clock is not their sum.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib  # noqa: E402

tag = sys.argv[1]
gate = next(g for g in lib.GATES if g[0] == tag)
outdir = '/tmp/rl/decompose'
report = {}
for index in range(2, len(sys.argv), 2):
    label, binary = sys.argv[index], sys.argv[index + 1]
    doc, wall, _ = lib.run_gate(binary, gate, outdir,
                                env={'POLYGON_NESTING_PROFILE': '1'},
                                label=f'{label}-{tag}-')
    profile = doc.get('searchProfile') or {}
    report[label] = {
        'gate': tag,
        'engineElapsedSeconds': lib.engine_seconds(doc),
        'processWallSeconds': wall,
        'threads': profile.get('threads'),
        'leafMilliseconds': profile.get('leafMilliseconds'),
        'counters': profile.get('counters'),
        'gateHit': lib.gate_check(gate, doc)['hit'],
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
os.makedirs(outdir, exist_ok=True)
json.dump(report, open(f'{outdir}/decompose-{tag}.json', 'w'), indent=1)
