#!/usr/bin/env python3
"""The four pinned gate documents, compared field by field across two runs.

    docdiff.py OUTFILE PREDIR PRELABEL POSTDIR POSTLABEL

A diffable copy of `docs/experiments/contact-block/drivers/docdiff.py`'s job:
`gates.py` reports whether the pinned scalars hit and a whole-document digest,
and a digest that moves says only *that* something moved. This says *what*.

`gatelib.strip_times` is applied first, so the comparison is over every
search-visible field and nothing that is a clock. This round changes no engine
code, so the only differences a reader should accept are build identity -
`engineCommit`, `engineWorktreeDirty`, `executableSha256`,
`relevantSourceTreeSha256`. Anything else is a finding.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import gatelib as lib  # noqa: E402

GATES = ('g1', 'g2', 'g3', 'g4')


def walk(pre, post, diffs, path=''):
    if isinstance(pre, dict) and isinstance(post, dict):
        for key in sorted(set(pre) | set(post)):
            if key not in pre or key not in post:
                diffs.append({'path': f'{path}/{key}', 'kind': 'keyOnlyInOne'})
                continue
            walk(pre[key], post[key], diffs, f'{path}/{key}')
    elif isinstance(pre, list) and isinstance(post, list):
        if len(pre) != len(post):
            diffs.append({'path': path, 'kind': 'length',
                          'pre': len(pre), 'post': len(post)})
            return
        for index, (a, b) in enumerate(zip(pre, post)):
            walk(a, b, diffs, f'{path}[{index}]')
    elif pre != post:
        diffs.append({'path': path, 'pre': pre, 'post': post})


def scalars(node):
    if isinstance(node, dict):
        return sum(scalars(v) for v in node.values())
    if isinstance(node, list):
        return sum(scalars(v) for v in node)
    return 1


def main():
    outfile, predir, prelabel, postdir, postlabel = sys.argv[1:6]
    result = {
        'note': ('Field-by-field diff of the four pinned gate documents, run '
                 'on the same binary before and after the audition commit, '
                 'with gatelib.strip_times applied. This round changes no '
                 'engine code, so the only permitted differences are build '
                 'identity.'),
        'pre': prelabel, 'post': postlabel, 'gates': {},
    }
    for gate in GATES:
        pre = lib.strip_times(json.load(
            open(f'{predir}/{prelabel}-{gate}.json')))
        post = lib.strip_times(json.load(
            open(f'{postdir}/{postlabel}-{gate}.json')))
        diffs = []
        walk(pre, post, diffs)
        result['gates'][gate] = {
            'scalarFieldsCompared': scalars(pre),
            'differenceCount': len(diffs),
            'differences': diffs,
        }
        print(f'{gate}: {scalars(pre)} scalar fields compared, '
              f'{len(diffs)} differences: '
              f'{[d["path"] for d in diffs]}', flush=True)
    result['ONLY_BUILD_IDENTITY_DIFFERS'] = all(
        all(d['path'].split('/')[-1] in {
            'engineCommit', 'engineWorktreeDirty', 'executableSha256',
            'relevantSourceTreeSha256'} for d in g['differences'])
        for g in result['gates'].values())
    json.dump(result, open(outfile, 'w'), indent=1)
    print(json.dumps({'ONLY_BUILD_IDENTITY_DIFFERS':
                      result['ONLY_BUILD_IDENTITY_DIFFERS']}, indent=1))


if __name__ == '__main__':
    main()
