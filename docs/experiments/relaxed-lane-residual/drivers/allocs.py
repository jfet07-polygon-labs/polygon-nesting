#!/usr/bin/env python3
"""Gross allocation demand of one gate stream, per binary.

    python3 allocs.py <gate> <label> <binary> [<label> <binary> ...]

Requires binaries built with `profiling-allocator`, which installs the counting
global allocator, and runs them with `POLYGON_NESTING_PROFILE=1`, which is what
arms the counters. What it reports is *gross* demand, not residency: `dealloc`
is not subtracted, so this is the number of trips to the allocator, which is the
quantity a buffer-reuse lever is supposed to move.

The allocator build is itself slower than the arm it describes, so these are
counts, never a wall-clock claim.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib  # noqa: E402

tag = sys.argv[1]
arms = [(sys.argv[i], sys.argv[i + 1]) for i in range(2, len(sys.argv), 2)]
gate = next(g for g in lib.GATES if g[0] == tag)
outdir = '/var/lib/t3/tmp/relaxb/allocs'

report = {'gate': tag, 'arms': {}}
for label, binary in arms:
    doc, wall, _ = lib.run_gate(binary, gate, outdir,
                                env={'POLYGON_NESTING_PROFILE': '1'},
                                label=f'alloc-{label}-')
    counters = ((doc.get('searchProfile') or {}).get('counters') or {})
    check = lib.gate_check(gate, doc)
    report['arms'][label] = {
        'binary': binary,
        'allocationCount': counters.get('allocationCount'),
        'allocationBytes': counters.get('allocationBytes'),
        'candidateQueries': counters.get('candidateQueries'),
        'surrogateEvaluations': (doc.get('relaxedDiagnostics') or {})
        .get('surrogateEvaluations'),
        'gateHit': check.get('hit'),
        'raw': check.get('raw') or check.get('depths'),
        'wallSeconds': wall,
    }
    print(json.dumps({label: report['arms'][label]}), flush=True)

base = arms[0][0]
for label, _ in arms[1:]:
    left, right = report['arms'][base], report['arms'][label]
    if left['allocationCount'] and right['allocationCount']:
        report[f'{label}Ratio'] = {
            'allocationCount': right['allocationCount'] / left['allocationCount'],
            'allocationBytes': right['allocationBytes'] / left['allocationBytes'],
            'allocationsRemoved': left['allocationCount'] - right['allocationCount'],
            'bytesRemoved': left['allocationBytes'] - right['allocationBytes'],
        }
print(json.dumps(report, indent=1))
os.makedirs(outdir, exist_ok=True)
json.dump(report, open(f'{outdir}/allocs-{tag}.json', 'w'), indent=1)
