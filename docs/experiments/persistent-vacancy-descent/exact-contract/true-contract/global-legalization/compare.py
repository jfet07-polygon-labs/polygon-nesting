#!/usr/bin/env python3
"""Compare already-written base/treat parity pairs, ignoring provenance only."""

import json
import os
import sys

# Fields that cannot possibly be equal across two different binaries or two
# different wall clocks. Everything else - every layout, depth, fingerprint,
# counter and diagnostic - must match bit for bit.
IGNORE_EXACT = {'executableSha256'}


def volatile(key):
    return (key in IGNORE_EXACT
            or 'ElapsedMs' in key or 'elapsedMs' in key
            or 'Millis' in key or 'Nanos' in key)


def walk(base, treat, path=''):
    if type(base) is not type(treat):
        return [(path, 'TYPE', base, treat)]
    if isinstance(base, dict):
        out = []
        for key in sorted(set(base) | set(treat)):
            if volatile(key):
                continue
            if key not in base:
                out.append((f'{path}/{key}', 'ONLY-TREAT', None, treat[key]))
            elif key not in treat:
                out.append((f'{path}/{key}', 'ONLY-BASE', base[key], None))
            else:
                out += walk(base[key], treat[key], f'{path}/{key}')
        return out
    if isinstance(base, list):
        if len(base) != len(treat):
            return [(path, 'LEN', len(base), len(treat))]
        out = []
        for index, (left, right) in enumerate(zip(base, treat)):
            out += walk(left, right, f'{path}[{index}]')
        return out
    return [] if base == treat else [(path, 'VAL', base, treat)]


def main():
    directory = sys.argv[1]
    tags = sorted({name[:-len('-base.json')] for name in os.listdir(directory)
                   if name.endswith('-base.json')})
    failures = 0
    for tag in tags:
        with open(f'{directory}/{tag}-base.json') as handle:
            base = json.load(handle)
        with open(f'{directory}/{tag}-treat.json') as handle:
            treat = json.load(handle)
        differences = walk(base, treat)
        if differences:
            failures += 1
            print(f'{tag}: *** {len(differences)} DIFFERENCES ***')
            for row in differences[:8]:
                print(f'    {row}')
        else:
            print(f'{tag}: IDENTICAL')
    print(f'differing cases: {failures}/{len(tags)}')


if __name__ == '__main__':
    main()
