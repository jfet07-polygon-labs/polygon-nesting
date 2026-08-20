#!/usr/bin/env python3
"""Whole-document equality of two binaries' gate runs, identity fields aside.

    gatedocdiff.py DIR LEFT RIGHT [OUT]

`gates.py` already prints a `docDigest` per gate, but that digest is not
comparable across two *builds*: the benchmark stamps `engineCommit`,
`engineWorktreeStatus`, `engineWorktreeDirty` and `executableSha256` into every
document, and those differ by construction between a fixed and an unfixed
binary even when every measured number is identical. Strip them - and the
host-identity fields, which differ between boxes - and the remaining document
is the arm's whole answer, compared field by field rather than only on the four
pinned scalars.
"""
import hashlib
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import gatelib  # noqa: E402

IDENTITY = {'engineCommit', 'engineWorktreeStatus', 'engineWorktreeDirty',
            'executableSha256', 'executablePath', 'binaryPath', 'cpuModel',
            'actualThreads', 'hostname', 'buildProfile'}

# The wall-clock summary `gatelib.VOLATILE` misses: the same five quartiles of
# the same measured stream, which are a clock reading on a shared box and
# nothing else.
CLOCK = {'medianElapsedMs', 'minElapsedMs', 'maxElapsedMs',
         'firstQuartileElapsedMs', 'thirdQuartileElapsedMs',
         'interquartileRangeElapsedMs'}
IDENTITY |= CLOCK


def strip(node):
    if isinstance(node, dict):
        return {k: strip(v) for k, v in sorted(node.items())
                if k not in gatelib.VOLATILE and k not in IDENTITY}
    if isinstance(node, list):
        return [strip(v) for v in node]
    if isinstance(node, float):
        return repr(node)
    return node


def digest(doc):
    return hashlib.sha256(
        json.dumps(strip(doc), sort_keys=True).encode()).hexdigest()


def main():
    directory, left, right = sys.argv[1], sys.argv[2], sys.argv[3]
    out = {'dir': directory, 'left': left, 'right': right, 'gates': {}}
    for gate in (g[0] for g in gatelib.GATES):
        a = json.load(open(f'{directory}/{left}-{gate}.json'))
        b = json.load(open(f'{directory}/{right}-{gate}.json'))
        da, db = digest(a), digest(b)
        row = {'leftDigest': da[:16], 'rightDigest': db[:16],
               'identical': da == db}
        if da != db:
            ka, kb = strip(a), strip(b)
            row['differingKeys'] = sorted(
                k for k in set(list(ka) + list(kb)) if ka.get(k) != kb.get(k))
        out['gates'][gate] = row
    out['ALL_IDENTICAL'] = all(g['identical'] for g in out['gates'].values())
    print(json.dumps(out, indent=1))
    if len(sys.argv) > 4:
        json.dump(out, open(sys.argv[4], 'w'), indent=1)


if __name__ == '__main__':
    main()
