#!/usr/bin/env python3
"""Whole-document comparison of two gate output directories.

Wall-clock and build-identity fields are removed before comparing: they are the
only fields that legitimately differ between two binaries built from different
trees. Everything else - every counter, every restart row, every diagnostic
field, every failure-reason string - must agree, which is a much stronger claim
than "the published number reproduces".

Usage: docdiff.py DIR_A DIR_B TAG [TAG ...]
"""
import json
import sys

VOLATILE = {
    'elapsedMs', 'elapsedSeconds', 'firstQuartileElapsedMs',
    'thirdQuartileElapsedMs', 'medianElapsedMs', 'minElapsedMs',
    'maxElapsedMs',
    'engineCommit', 'engineWorktreeDirty', 'engineWorktreeStatus',
    'executableSha256', 'relevantSourceTreeSha256', 'request',
    'startedSeconds', 'enteredSeconds', 'birthSeconds', 'seconds',
    'publishedSeconds', 'occupancyOverTime',
}


def strip(node):
    if isinstance(node, dict):
        return {key: strip(value) for key, value in node.items()
                if key not in VOLATILE}
    if isinstance(node, list):
        return [strip(value) for value in node]
    return node


def paths(node, prefix=''):
    if isinstance(node, dict):
        for key, value in node.items():
            yield from paths(value, f'{prefix}/{key}')
    elif isinstance(node, list):
        for index, value in enumerate(node):
            yield from paths(value, f'{prefix}/{index}')
    else:
        yield prefix, node


def main():
    dir_a, dir_b, tags = sys.argv[1], sys.argv[2], sys.argv[3:]
    result = {'a': dir_a, 'b': dir_b, 'documents': {}}
    for tag in tags:
        try:
            left = strip(json.load(open(f'{dir_a}/{tag}.json')))
            right = strip(json.load(open(f'{dir_b}/{tag}.json')))
        except (OSError, json.JSONDecodeError) as error:
            result['documents'][tag] = {'identical': False,
                                        'error': str(error)}
            continue
        if left == right:
            result['documents'][tag] = {'identical': True, 'differences': 0}
            continue
        left_paths = dict(paths(left))
        right_paths = dict(paths(right))
        differing = sorted(
            key for key in set(left_paths) | set(right_paths)
            if left_paths.get(key, '<absent>') != right_paths.get(key, '<absent>'))
        result['documents'][tag] = {
            'identical': False,
            'differences': len(differing),
            'first': [
                {'path': key,
                 'a': left_paths.get(key, '<absent>'),
                 'b': right_paths.get(key, '<absent>')}
                for key in differing[:12]
            ],
        }
    result['ALL_IDENTICAL'] = all(row['identical']
                                  for row in result['documents'].values())
    print(json.dumps(result, indent=1))


if __name__ == '__main__':
    main()
