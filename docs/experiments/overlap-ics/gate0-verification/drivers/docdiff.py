#!/usr/bin/env python3
"""Two cell documents, compared field by field rather than by one digest.

    python3 docdiff.py <left.json> <right.json>

The round's own `lib.digest` compares whole documents after stripping a named
`wall` object, which is the right comparison for two processes of one binary in
one worktree. It is the *wrong* comparison for a reproduction in a **second**
worktree: the documents embed absolute paths (`request.path`, `poses.path`,
`binary`) and the binary's own `executableSha256`, so a digest comparison would
report "different" for two runs that agree on every number.

This walks both documents to their scalar leaves and reports the leaves that
disagree, having neutralised exactly three things and named them:

  * the `wall` object - the round's own wall confinement;
  * `executableSha256` - the binary being varied;
  * any leaf whose key ends in `path`, `binary` or `root` - the worktree.

Exit status is 0 when nothing else differs. That is the claim a cross-worktree
reproduction can honestly make: **identical modulo paths and the binary hash**.
"""
import json
import sys

NEUTRAL = {'wall', 'executableSha256'}
PATHISH = ('path', 'binary', 'root')


def walk(node, prefix, out):
    if isinstance(node, dict):
        for key, value in node.items():
            if key in NEUTRAL:
                continue
            walk(value, f'{prefix}.{key}', out)
    elif isinstance(node, list):
        for index, value in enumerate(node):
            walk(value, f'{prefix}[{index}]', out)
    else:
        out[prefix] = node


def compare(left_path, right_path):
    left = json.load(open(left_path))
    right = json.load(open(right_path))
    a, b = {}, {}
    walk(left, '', a)
    walk(right, '', b)
    keys = sorted(set(a) | set(b))
    diffs = []
    path_diffs = 0
    for key in keys:
        if a.get(key) == b.get(key):
            continue
        if any(key.lower().endswith(suffix) for suffix in PATHISH):
            path_diffs += 1
            continue
        diffs.append({'field': key, 'left': a.get(key), 'right': b.get(key)})
    return {
        'left': left_path,
        'right': right_path,
        'scalarFieldsCompared': len(keys),
        'pathFieldsIgnored': path_diffs,
        'differingFields': diffs,
        'IDENTICAL_MODULO_PATHS_AND_BINARY': not diffs,
    }


def main():
    document = compare(sys.argv[1], sys.argv[2])
    print(json.dumps(document, indent=1))
    return 0 if document['IDENTICAL_MODULO_PATHS_AND_BINARY'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
