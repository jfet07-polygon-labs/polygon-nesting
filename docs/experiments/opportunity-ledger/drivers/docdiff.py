#!/usr/bin/env python3
"""Whole-document comparison of two gate runs.

    python3 docdiff.py DIR LABEL_A LABEL_B

The pinned scalars are what the gates assert; this is the stronger check the
protected-legacy ledger asks for - every counter, every restart row, every
diagnostic field, with wall-clock and build-identity fields removed. Those
removed fields are listed in the output rather than hidden, because "we ignored
some fields" is only honest if the reader can see which.
"""
import json
import sys

# Fields that legitimately differ between two runs of the same binary, or
# between two builds: the clock, and the build's own identity.
VOLATILE = {
    'elapsedMs', 'elapsedSeconds', 'engineElapsedSeconds', 'wallMs',
    'durationMs', 'timestamp', 'totalMs', 'ms', 'processWallSeconds',
    'phaseProfile', 'phases', 'profile', 'leafSeconds', 'engineVersion',
    'buildIdentity', 'binaryPath', 'peakResidentBytes', 'allocatedBytes',
    'medianElapsedMs', 'minElapsedMs', 'maxElapsedMs',
    'firstQuartileElapsedMs', 'thirdQuartileElapsedMs',
    'engineWorktreeStatus', 'executableSha256', 'relevantSourceTreeSha256',
}


def flatten(node, path='', out=None):
    if out is None:
        out = {}
    if isinstance(node, dict):
        for key, value in node.items():
            if key in VOLATILE:
                continue
            flatten(value, path + '/' + key, out)
    elif isinstance(node, list):
        for index, value in enumerate(node):
            flatten(value, path + f'/{index}', out)
    else:
        out[path] = repr(node) if isinstance(node, float) else node
    return out


def main():
    directory, label_a, label_b = sys.argv[1], sys.argv[2], sys.argv[3]
    result = {'dir': directory, 'a': label_a, 'b': label_b,
              'ignoredFields': sorted(VOLATILE), 'gates': {}}
    for gate in ('g1', 'g2', 'g3', 'g4'):
        first = flatten(json.load(
            open(f'{directory}/{label_a}-{gate}.json')))
        second = flatten(json.load(
            open(f'{directory}/{label_b}-{gate}.json')))
        keys = set(first) | set(second)
        differences = [k for k in sorted(keys)
                       if first.get(k) != second.get(k)]
        result['gates'][gate] = {
            'fieldsCompared': len(keys),
            'differences': len(differences),
            'first': differences[:20],
        }
    result['ALL_EQUAL'] = all(g['differences'] == 0
                              for g in result['gates'].values())
    print(json.dumps(result, indent=1))


if __name__ == '__main__':
    main()
