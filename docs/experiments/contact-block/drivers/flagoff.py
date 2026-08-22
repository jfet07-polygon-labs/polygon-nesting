#!/usr/bin/env python3
"""Flag-off bit reproduction: the four gate documents, both binaries, field by
field.

    flagoff.py BASEDIR CBDIR BASELABEL CBLABEL [OUT]

`gates.py` already reports that the four pinned scalars reproduce. This asks the
stronger question the protocol wants: with the feature compiled in and no
environment variable set, is the *whole document* the same one the shipping
build produces? A pinned-scalar match can hide a changed trajectory that happens
to land on the same depth; a whole-document match cannot.

`executableSha256` is expected to differ and is reported as such rather than
stripped: the two binaries genuinely are different files, and a driver that
filtered that field would be filtering the one difference it is certain of.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import docdiff  # noqa: E402
import gatelib as lib  # noqa: E402

EXPECTED = {'/executableSha256'}


def main():
    base_dir, cb_dir, base_label, cb_label = sys.argv[1:5]
    out_path = sys.argv[5] if len(sys.argv) > 5 else None
    result = {'baseDir': base_dir, 'cbDir': cb_dir, 'gates': {}}
    for gate in lib.GATES:
        tag = gate[0]
        left = lib.strip_times(json.load(
            open(f'{base_dir}/{base_label}-{tag}.json')))
        right = lib.strip_times(json.load(
            open(f'{cb_dir}/{cb_label}-{tag}.json')))
        a, b = docdiff.flatten(left), docdiff.flatten(right)
        keys = sorted(set(a) | set(b))
        differing = [k for k in keys
                     if a.get(k, '<missing>') != b.get(k, '<missing>')]
        unexpected = [k for k in differing if k not in EXPECTED]
        result['gates'][tag] = {
            'fields': len(keys),
            'differing': differing,
            'unexpectedDiffering': unexpected,
            'bitReproducesModuloExecutableHash': not unexpected,
        }
        print(f'{tag}: {len(keys)} fields, differing={differing}, '
              f'clean={not unexpected}')
    result['ALL_CLEAN'] = all(
        g['bitReproducesModuloExecutableHash'] for g in result['gates'].values())
    print(json.dumps({'ALL_CLEAN': result['ALL_CLEAN']}, indent=1))
    if out_path:
        json.dump(result, open(out_path, 'w'), indent=1)


if __name__ == '__main__':
    main()
