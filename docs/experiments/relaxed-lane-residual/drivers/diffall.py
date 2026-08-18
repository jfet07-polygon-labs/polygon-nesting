#!/usr/bin/env python3
"""Whole-document diff of every gate between two binaries.

    python3 diffall.py <aLabel> <aBinary> <bLabel> <bBinary>
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib  # noqa: E402

a_label, a_binary, b_label, b_binary = sys.argv[1:5]
outdir = '/tmp/rl/diff'


def flatten(node, path='', out=None):
    if out is None:
        out = {}
    if isinstance(node, dict):
        for key, value in node.items():
            if key in lib.VOLATILE:
                continue
            flatten(value, f'{path}/{key}', out)
    elif isinstance(node, list):
        for index, value in enumerate(node):
            flatten(value, f'{path}/{index}', out)
    else:
        out[path] = node
    return out


report = {}
for gate in lib.GATES:
    tag = gate[0]
    docs = {}
    for label, binary in ((a_label, a_binary), (b_label, b_binary)):
        doc, _, _ = lib.run_gate(binary, gate, outdir, label=f'{label}-{tag}-')
        docs[label] = flatten(doc)
    left, right = docs[a_label], docs[b_label]
    keys = sorted(set(left) | set(right))
    diffs = [{'field': k, a_label: left.get(k), b_label: right.get(k)}
             for k in keys if left.get(k) != right.get(k)]
    report[tag] = {'fieldsCompared': len(keys), 'fieldsDiffering': len(diffs),
                   'diffs': diffs}
    print(json.dumps({tag: {'fieldsCompared': len(keys),
                            'fieldsDiffering': len(diffs),
                            'diffs': diffs[:20]}}, indent=1), flush=True)
os.makedirs(outdir, exist_ok=True)
json.dump(report, open(f'{outdir}/diffall-{a_label}-{b_label}.json', 'w'), indent=1)
