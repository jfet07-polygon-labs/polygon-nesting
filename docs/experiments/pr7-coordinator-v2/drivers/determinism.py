#!/usr/bin/env python3
"""The work-budget mode's reproducibility gate, re-run for the v2 schedule.

v2 adds one new branch point - the affordability guard, which compares a
*measured operator cost* against what is left of the budget - and that guard is
exactly the kind of thing that can quietly turn a reproducible schedule into a
clock-dependent one. It does not, because the cost it reads is quoted in the
budget's own currency: work units under a work budget, seconds only under a
wall budget. This driver is the check rather than the claim.

  1. `runs=2` inside one process. The benchmark fails closed when a replay
     produces a different result or different relaxed diagnostics.
  2. Two independent processes, compared as whole documents with the
     wall-clock and build-identity fields removed. Every phase boundary, every
     operator call, every archive member and every work-unit reading must
     agree.

Usage: determinism.py WORK_UNITS [SEED] [REQUEST] [BINARY]
"""
import json
import os
import sys

# The PR7 directory goes on the path *second*, because it carries a `lib` of
# its own and this stage's `lib` is the one that must win; only `docdiff` is
# borrowed, and it is borrowed rather than copied so that "whole document" means
# the same set of stripped fields it meant in PR7.
sys.path.insert(0, os.path.join(
    os.path.dirname(os.path.dirname(os.path.dirname(
        os.path.abspath(__file__)))),
    'pr7-portfolio-coordinator', 'drivers'))
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import docdiff  # noqa: E402
import lib  # noqa: E402

SPEC = 'work={units},cells=13:15:17:19'


def main():
    units = int(sys.argv[1])
    seed = int(sys.argv[2]) if len(sys.argv) > 2 else 1
    request = sys.argv[3] if len(sys.argv) > 3 else 'mixed-61'
    binary = sys.argv[4] if len(sys.argv) > 4 else lib.V2_BIN
    spec = SPEC.format(units=units)
    out = f'{lib.OUT}/determinism-{units}-{request}'
    result = {'binary': binary, 'request': request, 'workUnits': units,
              'seed': seed, 'spec': spec}

    replay, _, error = lib.run(binary, request, seed, spec,
                               f'{out}/replay.json', runs=2)
    result['inProcessReplay'] = {
        'runs': 2,
        'passed': '_loadError' not in replay,
        'error': error[-300:] if '_loadError' in replay else '',
        'engineDepthMm': replay.get('independentUsedLongAxisDepthMm'),
    }

    first, _, _ = lib.run(binary, request, seed, spec, f'{out}/process-a.json')
    second, _, _ = lib.run(binary, request, seed, spec, f'{out}/process-b.json')
    if '_loadError' in first or '_loadError' in second:
        result['crossProcess'] = {'passed': False,
                                  'error': first.get('_loadError', '')[-300:]}
    else:
        left = dict(docdiff.paths(docdiff.strip(first)))
        right = dict(docdiff.paths(docdiff.strip(second)))
        differing = sorted(
            key for key in set(left) | set(right)
            if left.get(key, '<absent>') != right.get(key, '<absent>'))
        result['crossProcess'] = {
            'passed': not differing,
            'differences': len(differing),
            'first': [{'path': key, 'a': left.get(key, '<absent>'),
                       'b': right.get(key, '<absent>')}
                      for key in differing[:12]],
            'engineDepthMm': first.get('independentUsedLongAxisDepthMm'),
            'workUnitsSpent': first['portfolio']['workUnits'],
            'phases': [{'name': phase['name'],
                        'workUnits': phase['workUnits'],
                        'skipped': phase['skipped'],
                        'operatorCalls': phase['operatorCalls']}
                       for phase in first['portfolio']['phases']],
        }
    result['ALL_PASS'] = (result['inProcessReplay']['passed']
                          and result['crossProcess']['passed'])
    print(json.dumps(result, indent=1))
    os.makedirs(out, exist_ok=True)
    json.dump(result, open(f'{out}/determinism.json', 'w'), indent=1)


if __name__ == '__main__':
    main()
