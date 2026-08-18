#!/usr/bin/env python3
"""Part 1: the opportunity-and-delayed-credit ledger on the saturated state.

    python3 ledger.py [WORK_UNITS] [SEEDS] [BINARY]

Runs the coordinator from the bare request at a work budget large enough that
no phase is stopped by it, and writes the five ledger tables the review asks
for. Work-budget mode is deterministic and load-independent, so one run per
seed is the whole measurement; `--twice` re-runs each seed in a second process
and reports whether the two documents agree.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import runlib  # noqa: E402

VOLATILE = {
    'elapsedMs', 'elapsedSeconds', 'engineElapsedSeconds', 'wallMs',
    'durationMs', 'timestamp', 'totalMs', 'ms', 'processWallSeconds',
    'phaseProfile', 'phases', 'profile', 'leafSeconds', 'engineVersion',
    'buildIdentity', 'binaryPath', 'peakResidentBytes', 'allocatedBytes',
    'medianElapsedMs', 'minElapsedMs', 'maxElapsedMs',
    'firstQuartileElapsedMs', 'thirdQuartileElapsedMs',
    'engineWorktreeStatus', 'executableSha256', 'relevantSourceTreeSha256',
    'startedSeconds', 'secondsSpent', 'secondsP50', 'secondsP95',
    'secondsTotal', 'birthSeconds', 'publishedSeconds', 'seconds',
    'occupancyOverTime',
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
    work = int(sys.argv[1]) if len(sys.argv) > 1 else runlib.WORK_30S
    seeds = [int(s) for s in (sys.argv[2] if len(sys.argv) > 2
                              else '0,1,2').split(',')]
    binary = sys.argv[3] if len(sys.argv) > 3 else '/var/lib/t3/tmp/ledger-bin'
    twice = '--twice' in sys.argv
    result = {'binary': binary, 'workBudget': work, 'seeds': seeds,
              'allowance': runlib.DEFAULT_ALLOWANCE, 'runs': {}}
    for seed in seeds:
        spec = runlib.spec_for(seed, work)
        doc, wall, err = runlib.run(
            binary, 'mixed-61', seed, spec,
            f'{runlib.OUT}/ledger/seed{seed}-{work}.json')
        if '_loadError' in doc:
            result['runs'][seed] = {'error': err[-600:]}
            continue
        row = {'spec': spec, 'processWallSeconds': wall,
               'portfolio': doc['portfolio']}
        if twice:
            again, _, _ = runlib.run(
                binary, 'mixed-61', seed, spec,
                f'{runlib.OUT}/ledger/seed{seed}-{work}-b.json')
            first, second = flatten(doc), flatten(again)
            keys = set(first) | set(second)
            row['determinism'] = {
                'fieldsCompared': len(keys),
                'differences': sum(1 for k in keys
                                   if first.get(k) != second.get(k)),
            }
        result['runs'][seed] = row
    print(json.dumps(result, indent=1))


if __name__ == '__main__':
    main()
