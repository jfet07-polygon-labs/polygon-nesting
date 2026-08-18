#!/usr/bin/env python3
"""Runs one gate stream on a counting build and prints the census.

    python3 census.py <binary> <gate-tag> [outdir]

A counting build runs the prefilter ladder on every observed pair, so the
elapsed time it reports is meaningless. Only the counts here are quotable.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib  # noqa: E402

binary = sys.argv[1]
tag = sys.argv[2] if len(sys.argv) > 2 else 'g1'
outdir = sys.argv[3] if len(sys.argv) > 3 else '/var/lib/t3/tmp/cinner/census'
gate = next(g for g in lib.GATES if g[0] == tag)
doc, wall, err = lib.run_gate(binary, gate, outdir, label='census-')
if '_loadError' in doc:
    print(json.dumps({'error': doc['_loadError'][:2000]}))
    raise SystemExit(1)
census = doc.get('constructorCensus')
report = {
    'gate': tag,
    'binary': binary,
    'countingBuildWallSeconds': wall,
    'gateCheck': lib.gate_check(gate, doc),
    'census': census,
}
profile = doc.get('searchProfile')
if profile:
    report['phases'] = {
        name: {'ms': row.get('ms'), 'calls': row.get('calls')}
        for name, row in (profile.get('phases') or {}).items()
    }
print(json.dumps(report, indent=1))
os.makedirs(outdir, exist_ok=True)
json.dump(report, open(f'{outdir}/census-{tag}.json', 'w'), indent=1)
