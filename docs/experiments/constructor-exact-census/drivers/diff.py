#!/usr/bin/env python3
"""Whole-document comparison of one gate stream between two binaries.

    python3 diff.py <aLabel> <aBinary> <bLabel> <bBinary> [gate]

Prints every leaf field whose value differs, wall-clock fields excluded. The
point is to see *which* fields a flag moves, not only whether the pinned
scalars survive.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib  # noqa: E402

a_label, a_binary, b_label, b_binary = sys.argv[1:5]
tag = sys.argv[5] if len(sys.argv) > 5 else 'g1'
outdir = '/var/lib/t3/tmp/ccensus/diff'
gate = next(g for g in lib.GATES if g[0] == tag)


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


docs = {}
for label, binary in ((a_label, a_binary), (b_label, b_binary)):
    doc, _, _ = lib.run_gate(binary, gate, outdir, label=f'{label}-')
    docs[label] = flatten(doc)

left, right = docs[a_label], docs[b_label]
keys = sorted(set(left) | set(right))
diffs = [{'field': key, a_label: left.get(key), b_label: right.get(key)}
         for key in keys if left.get(key) != right.get(key)]
print(json.dumps({'gate': tag, 'fieldsCompared': len(keys),
                  'fieldsDiffering': len(diffs), 'diffs': diffs}, indent=1))
os.makedirs(outdir, exist_ok=True)
json.dump({'gate': tag, 'fieldsCompared': len(keys), 'diffs': diffs},
          open(f'{outdir}/diff-{a_label}-{b_label}-{tag}.json', 'w'), indent=1)
