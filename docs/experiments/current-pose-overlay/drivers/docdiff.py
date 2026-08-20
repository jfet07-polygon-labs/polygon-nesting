#!/usr/bin/env python3
"""Whole-document diff between two gate runs.

    python3 docdiff.py DIR_A LABEL_A DIR_B LABEL_B OUT.json

The four pinned gates check four scalars each. This checks *everything else*:
every leaf of the two benchmark documents, with only the fields that
legitimately differ between two runs of the same code removed
(`lib.VOLATILE` — clocks, profiles, allocation counters). Anything that
survives and still differs is either a build artefact (executable hash, source
tree hash, worktree status) or a semantic change, and the point of running it
is that the two are then distinguishable by name rather than by assertion.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib  # noqa: E402


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


def main():
    dir_a, label_a, dir_b, label_b, out_path = sys.argv[1:6]
    result = {'a': {'dir': dir_a, 'label': label_a},
              'b': {'dir': dir_b, 'label': label_b}, 'gates': {}}
    for gate in lib.GATES:
        tag = gate[0]
        try:
            doc_a = json.load(open(f'{dir_a}/{label_a}-{tag}.json'))
            doc_b = json.load(open(f'{dir_b}/{label_b}-{tag}.json'))
        except (OSError, json.JSONDecodeError) as error:
            result['gates'][tag] = {'error': str(error)}
            continue
        flat_a = leaves(lib.strip_volatile(doc_a))
        flat_b = leaves(lib.strip_volatile(doc_b))
        keys = sorted(set(flat_a) | set(flat_b))
        differing = {k: {label_a: flat_a.get(k), label_b: flat_b.get(k)}
                     for k in keys if flat_a.get(k) != flat_b.get(k)}
        result['gates'][tag] = {
            'leavesCompared': len(keys),
            'differingLeafCount': len(differing),
            'differingLeaves': differing,
        }
    result['totalDifferingLeaves'] = sum(
        g.get('differingLeafCount', 0) for g in result['gates'].values())
    json.dump(result, open(out_path, 'w'), indent=1)
    print(json.dumps(
        {'totalDifferingLeaves': result['totalDifferingLeaves'],
         'perGate': {t: {'leavesCompared': g.get('leavesCompared'),
                         'differing': sorted(g.get('differingLeaves', {}))}
                     for t, g in result['gates'].items()}}, indent=1))


if __name__ == '__main__':
    main()
