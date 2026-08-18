#!/usr/bin/env python3
"""Gate 1 alone against one binary, for the slow debug arm.

    python3 gate1.py <label> <binary> [outdir]
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib  # noqa: E402

label, binary = sys.argv[1:3]
outdir = sys.argv[3] if len(sys.argv) > 3 else f'/var/lib/t3/tmp/ccensus/gates/{label}'
gate = next(g for g in lib.GATES if g[0] == 'g1')
doc, wall, err = lib.run_gate(binary, gate, outdir, label=f'{label}-')
check = lib.gate_check(gate, doc)
check['wallSeconds'] = wall
check['stderrTail'] = err[-400:]
print(json.dumps({'label': label, 'binary': binary, 'g1': check}, indent=1))
