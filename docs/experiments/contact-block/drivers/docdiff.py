#!/usr/bin/env python3
"""Where two gate documents differ, once the wall-clock fields are stripped.

    docdiff.py LEFT.json RIGHT.json [MAXROWS]

`gates.py` reports a whole-document digest beside the four pinned scalars. When
the scalars reproduce and the digests do not, the digest is the finding and the
only honest way to report it is to say **which fields moved**. A digest
mismatch that turns out to be a build-identity string is not a trajectory
difference, and a digest mismatch that turns out to be a placement is.
"""
import json
import sys

sys.path.insert(0, __file__.rsplit('/', 1)[0])
import gatelib as lib  # noqa: E402


def flatten(node, path='', out=None):
    if out is None:
        out = {}
    if isinstance(node, dict):
        for key, value in node.items():
            flatten(value, f'{path}/{key}', out)
    elif isinstance(node, list):
        for index, value in enumerate(node):
            flatten(value, f'{path}[{index}]', out)
    else:
        out[path] = node
    return out


def main():
    left = lib.strip_times(json.load(open(sys.argv[1])))
    right = lib.strip_times(json.load(open(sys.argv[2])))
    limit = int(sys.argv[3]) if len(sys.argv) > 3 else 40
    a, b = flatten(left), flatten(right)
    keys = sorted(set(a) | set(b))
    diffs = [(k, a.get(k, '<missing>'), b.get(k, '<missing>'))
             for k in keys if a.get(k, '<missing>') != b.get(k, '<missing>')]
    print(json.dumps({'fields': len(keys), 'differing': len(diffs)}, indent=1))
    for key, x, y in diffs[:limit]:
        print(f'  {key}\n    left  = {json.dumps(x)[:160]}\n'
              f'    right = {json.dumps(y)[:160]}')
    if len(diffs) > limit:
        print(f'  ... and {len(diffs) - limit} more')


if __name__ == '__main__':
    main()
