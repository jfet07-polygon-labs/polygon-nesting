#!/usr/bin/env python3
"""Which leaf paths differ between two result documents, after `strip_volatile`.

    python3 leafdiff.py A.json B.json [MAX]

`gatelib.doc_digest` answers "are these the same document" with one bit, which
is the right instrument when the answer is yes and a useless one when it is no.
This prints the paths, so a digest mismatch can be classified rather than
argued about - which is what the m34-wall-price round's `docdiff.py` is for and
this is the single-pair form of.

Written for this round because the inherited `VOLATILE` set was assembled
against the *gate* documents and does not cover the compression schedule's own
per-slice millisecond fields, so two runs of the SAME binary on a mode-34
document mismatch for reasons that have nothing to do with any feature.
"""
import json
import sys

sys.path.insert(0, __file__.rsplit('/', 1)[0])
import gatelib  # noqa: E402


def leaves(node, path='', out=None):
    if out is None:
        out = {}
    if isinstance(node, dict):
        for key, value in node.items():
            leaves(value, f'{path}/{key}', out)
    elif isinstance(node, list):
        for index, value in enumerate(node):
            leaves(value, f'{path}/{index}', out)
    else:
        out[path] = node
    return out


a = leaves(gatelib.strip_volatile(json.load(open(sys.argv[1]))))
b = leaves(gatelib.strip_volatile(json.load(open(sys.argv[2]))))
limit = int(sys.argv[3]) if len(sys.argv) > 3 else 40
keys = sorted(set(a) | set(b))
differ = [k for k in keys if a.get(k) != b.get(k)]
print(json.dumps({
    'leaves': len(keys),
    'differing': len(differ),
    'differingFieldNames': sorted({k.rsplit('/', 1)[-1] for k in differ}),
    'examples': [{'path': k, 'a': a.get(k), 'b': b.get(k)}
                 for k in differ[:limit]],
}, indent=1))
