#!/usr/bin/env python3
"""The four pinned regression gates, run against one binary.

    python3 gates.py <label> <binary> [outdir]

Prints one JSON document: per gate the pinned fields, whether they reproduce,
and a whole-document digest with the wall-clock fields removed, so two binaries
can be compared as documents rather than only on the pinned scalars.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import gatelib as lib  # noqa: E402

label = sys.argv[1]
binary = sys.argv[2]
outdir = sys.argv[3] if len(sys.argv) > 3 \
    else f'/var/lib/t3/tmp/overlapics/gates/{label}'

result = {'label': label, 'binary': binary, 'gates': {}}
for gate in lib.GATES:
    doc, wall, _ = lib.run_gate(binary, gate, outdir, label=label + '-')
    check = lib.gate_check(gate, doc)
    check['wallSeconds'] = wall
    check['docDigest'] = lib.doc_digest(doc)
    result['gates'][gate[0]] = check
result['ALL_PASS'] = all(g.get('hit') for g in result['gates'].values())
print(json.dumps(result, indent=1))
os.makedirs(outdir, exist_ok=True)
json.dump(result, open(f'{outdir}/gates-{label}.json', 'w'), indent=1)
