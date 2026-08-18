#!/usr/bin/env python3
"""The work-budget mode's reproducibility gate.

A wall-clock schedule branches on a clock, so two of its runs are two different
searches on a shared box. The work-budget schedule branches only on the
engine's own counters, so two of its runs must be one search - and that is
checkable rather than assertable:

  1. `runs=2` inside one process. The benchmark already fails closed when a
     replay produces a different result or different relaxed diagnostics, so
     this is the in-process half for free.
  2. Two independent processes, compared as whole documents with only the
     wall-clock and build-identity fields removed. Every phase boundary, every
     operator call, every archive member and every work-unit reading must
     agree.

Usage: determinism.py BINARY WORK_UNITS [SEED]
"""
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import docdiff  # noqa: E402
import lib  # noqa: E402

OUT = '/var/lib/t3/tmp/pr7/determinism'
DEFAULT_ALLOWANCE = '0.002'
SPEC = 'work={units},slots=4,states=3,cycles=1,epochs=4,cells=13:15:17:19'


def invoke(binary, tag, seed, spec, runs):
    argv = [binary, lib.REQ]
    args = [a.format(clamp='0', seed=seed) for a in lib.ARGS]
    args[0] = str(runs)
    argv += args + ['0', '', '', '', DEFAULT_ALLOWANCE, spec]
    env = dict(os.environ)
    env.pop('POLYGON_NESTING_QUALITY_TRACE', None)
    env.pop('POLYGON_NESTING_PROFILE', None)
    os.makedirs(OUT, exist_ok=True)
    path = f'{OUT}/{tag}.json'
    with open(path, 'w') as handle:
        completed = subprocess.run(argv, stdout=handle, stderr=subprocess.PIPE,
                                   check=False, env=env)
    try:
        return json.load(open(path)), completed.returncode, ''
    except json.JSONDecodeError:
        return None, completed.returncode, (completed.stderr or b'').decode()[-400:]


def main():
    binary, units = sys.argv[1], int(sys.argv[2])
    seed = int(sys.argv[3]) if len(sys.argv) > 3 else 1
    spec = SPEC.format(units=units)
    result = {'binary': binary, 'workUnits': units, 'seed': seed, 'spec': spec}

    replay, code, error = invoke(binary, 'replay', seed, spec, 2)
    result['inProcessReplay'] = {
        'runs': 2,
        'exitCode': code,
        'passed': code == 0 and replay is not None,
        'error': error,
    }
    if replay:
        result['inProcessReplay']['engineDepthMm'] = \
            replay.get('independentUsedLongAxisDepthMm')

    first, code_a, _ = invoke(binary, 'process-a', seed, spec, 1)
    second, code_b, _ = invoke(binary, 'process-b', seed, spec, 1)
    if first is None or second is None:
        result['crossProcess'] = {'passed': False, 'exitCodes': [code_a, code_b]}
    else:
        left = docdiff.strip(first)
        right = docdiff.strip(second)
        left_paths = dict(docdiff.paths(left))
        right_paths = dict(docdiff.paths(right))
        differing = sorted(
            key for key in set(left_paths) | set(right_paths)
            if left_paths.get(key, '<absent>') != right_paths.get(key, '<absent>'))
        result['crossProcess'] = {
            'passed': not differing,
            'differences': len(differing),
            'first': [
                {'path': key, 'a': left_paths.get(key, '<absent>'),
                 'b': right_paths.get(key, '<absent>')}
                for key in differing[:12]
            ],
            'engineDepthMm': first.get('independentUsedLongAxisDepthMm'),
            'workUnitsSpent': first['portfolio']['workUnits'],
            'phases': [
                {'name': phase['name'], 'workUnits': phase['workUnits'],
                 'skipped': phase['skipped'],
                 'operatorCalls': phase['operatorCalls']}
                for phase in first['portfolio']['phases']
            ],
        }
    result['ALL_PASS'] = (result['inProcessReplay']['passed']
                          and result['crossProcess']['passed'])
    print(json.dumps(result, indent=1))
    json.dump(result, open(f'{OUT}/determinism.json', 'w'), indent=1)


if __name__ == '__main__':
    main()
