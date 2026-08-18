#!/usr/bin/env python3
"""Whole-document comparison of two gate runs, field by field.

    python3 docdiff.py GATESDIR_A GATESDIR_B LABEL_A LABEL_B

The pinned scalars are the gate; this is the stronger check the
compression-schedule round introduced - every field of the benchmark document
compared, with only the wall-clock and build-identity fields removed, so a
change that moved something the gate does not name still shows up.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import gatelib  # noqa: E402

# On top of gatelib.VOLATILE: the fields that identify the *build* rather than
# its behaviour. Two binaries of two different source trees necessarily differ
# here and a difference here is not a behavioural difference.
BUILD_IDENTITY = {'executableSha256', 'relevantSourceTreeSha256',
                  'engineWorktreeStatus', 'engineCommit', 'minElapsedMs',
                  'medianElapsedMs', 'maxElapsedMs', 'firstQuartileElapsedMs',
                  'thirdQuartileElapsedMs', 'meanElapsedMs',
                  'standardDeviationMs'}

DIR_A, DIR_B, LABEL_A, LABEL_B = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4]


def flatten(node, path=''):
    if isinstance(node, dict):
        for key, value in node.items():
            yield from flatten(value, f'{path}/{key}')
    elif isinstance(node, list):
        for index, value in enumerate(node):
            yield from flatten(value, f'{path}/{index}')
    else:
        yield path, node


def load(directory, label, gate):
    doc = json.load(open(f'{directory}/{label}-{gate}.json'))
    return {k: v for k, v in flatten(gatelib.strip_volatile(doc))
            if k.rsplit('/', 1)[-1] not in BUILD_IDENTITY}


result = {'a': LABEL_A, 'b': LABEL_B, 'gates': {}}
for gate in ('g1', 'g2', 'g3', 'g4'):
    a, b = load(DIR_A, LABEL_A, gate), load(DIR_B, LABEL_B, gate)
    keys = set(a) | set(b)
    diffs = [k for k in sorted(keys) if a.get(k, '<missing>') != b.get(k, '<missing>')]
    result['gates'][gate] = {'fieldsCompared': len(keys),
                             'differences': len(diffs),
                             'differingFields': diffs[:40]}
result['identical'] = all(g['differences'] == 0 for g in result['gates'].values())
print(json.dumps(result, indent=1))
