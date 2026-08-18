#!/usr/bin/env python3
"""Two processes, one work budget, compared as whole documents.

A work budget is a function of the evaluation counters, so a v3 run must be a
deterministic function of (request, seed, budget) and nothing else - not of the
box's load, not of the clock. This compares the two documents field by field
after removing the fields that legitimately differ between two processes (wall
clock and build identity), which is a stronger check than comparing the depth.

    determinism.py NAME REQUEST SEEDS WORK
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import runlib  # noqa: E402

# Fields that are wall-clock or build identity and therefore legitimately
# differ between two processes. Everything else is compared.
VOLATILE_KEYS = {
    'elapsedMs', 'elapsedSeconds', 'engineElapsedSeconds', 'wallMs',
    'durationMs', 'timestamp', 'totalMs', 'ms', 'processWallSeconds',
    'medianElapsedMs', 'minElapsedMs', 'maxElapsedMs', 'p95ElapsedMs',
    'p50ElapsedMs', 'meanElapsedMs', 'firstQuartileElapsedMs',
    'thirdQuartileElapsedMs', 'executableSha256',
    'relevantSourceTreeSha256', 'engineWorktreeStatus', 'engineVersion',
    'buildIdentity', 'binaryPath', 'peakResidentBytes', 'allocatedBytes',
    'seconds', 'secondsSpent', 'startedSeconds', 'enteredSeconds',
    'publishedSeconds', 'birthSeconds', 'occupancyOverTime', 'secondsP50',
    'secondsP95', 'secondsTotal', 'phaseProfile', 'profile', 'leafSeconds',
}


def flatten(node, path='', out=None):
    if out is None:
        out = {}
    if isinstance(node, dict):
        for key, value in node.items():
            if key in VOLATILE_KEYS:
                continue
            flatten(value, f'{path}/{key}', out)
    elif isinstance(node, list):
        for index, value in enumerate(node):
            flatten(value, f'{path}/{index}', out)
    else:
        out[path] = repr(node)
    return out


def main():
    name = sys.argv[1]
    request = sys.argv[2]
    seeds = [int(v) for v in sys.argv[3].split(',')]
    work = int(sys.argv[4])

    out_dir = f'{runlib.OUT}/{name}'
    result = {'name': name, 'request': request, 'work': work,
              'binary': runlib.BIN, 'rows': []}
    for seed in seeds:
        for v3, label in ((True, 'v3'), (False, 'v2')):
            docs = []
            for attempt in (0, 1):
                spec = runlib.spec_for(seed, 'work', work, v3)
                doc, wall, err = runlib.run(
                    runlib.BIN, request, seed, spec,
                    f'{out_dir}/runs/{label}-s{seed}-p{attempt}.json')
                docs.append(doc)
            left, right = (flatten(d) for d in docs)
            keys = sorted(set(left) | set(right))
            differing = [k for k in keys if left.get(k) != right.get(k)]
            row = {
                'arm': label, 'seed': seed,
                'fieldsCompared': len(keys),
                'differingFields': len(differing),
                'differingSample': differing[:20],
                'rawDepthMm': [d.get('portfolio', {}).get('incumbent', {})
                               .get('rawDepthMm') for d in docs],
                'workUnits': [d.get('portfolio', {}).get('workUnits')
                              for d in docs],
            }
            result['rows'].append(row)
            print(json.dumps(row)[:600], flush=True)
    os.makedirs(out_dir, exist_ok=True)
    json.dump(result, open(f'{out_dir}/determinism.json', 'w'), indent=1)
    print(json.dumps([{k: v for k, v in r.items() if k != 'differingSample'}
                      for r in result['rows']], indent=1))


if __name__ == '__main__':
    main()
