#!/usr/bin/env python3
"""Compares two labelled gate runs as whole documents.

    docdiff.py GATEDIR LEFTLABEL RIGHTLABEL [OUT.json]

`gates.py` writes `<label>-<gate>.json` per gate. This flattens both documents,
removes the fields that legitimately differ between two builds - wall clock and
build identity - and reports how many of the rest differ. A search-visible
change anywhere would show up here even if the pinned scalars happened to
match.
"""
import json
import os
import sys

# Wall clock and build identity. Everything else is compared.
VOLATILE = {
    'elapsedMs', 'medianElapsedMs', 'minElapsedMs', 'maxElapsedMs',
    'firstQuartileElapsedMs', 'thirdQuartileElapsedMs', 'meanElapsedMs',
    'p50ElapsedMs', 'p95ElapsedMs', 'elapsedSeconds', 'engineElapsedSeconds',
    'wallMs', 'durationMs', 'timestamp', 'totalMs', 'ms',
    'processWallSeconds', 'executableSha256', 'relevantSourceTreeSha256',
    'engineWorktreeStatus', 'engineVersion', 'buildIdentity', 'binaryPath',
    'peakResidentBytes', 'allocatedBytes', 'seconds', 'startedSeconds',
    'enteredSeconds', 'publishedSeconds', 'birthSeconds', 'secondsSpent',
    'phaseProfile', 'profile', 'leafSeconds',
}


def flatten(node, path='', out=None):
    if out is None:
        out = {}
    if isinstance(node, dict):
        for key, value in node.items():
            if key in VOLATILE:
                continue
            flatten(value, f'{path}/{key}', out)
    elif isinstance(node, list):
        for index, value in enumerate(node):
            flatten(value, f'{path}/{index}', out)
    else:
        out[path] = repr(node)
    return out


gatedir = sys.argv[1]
left_label, right_label = sys.argv[2], sys.argv[3]
result = {'gateDir': gatedir, 'left': left_label, 'right': right_label,
          'volatileFields': sorted(VOLATILE), 'gates': {}}
for gate in ('g1', 'g2', 'g3', 'g4'):
    left_path = f'{gatedir}/{left_label}/{left_label}-{gate}.json'
    right_path = f'{gatedir}/{right_label}/{right_label}-{gate}.json'
    if not (os.path.exists(left_path) and os.path.exists(right_path)):
        result['gates'][gate] = {'error': 'missing document'}
        continue
    left = flatten(json.load(open(left_path)))
    right = flatten(json.load(open(right_path)))
    keys = sorted(set(left) | set(right))
    differing = [k for k in keys if left.get(k) != right.get(k)]
    result['gates'][gate] = {
        'fieldsCompared': len(keys),
        'differingFields': len(differing),
        'differing': differing[:40],
    }
result['ALL_IDENTICAL'] = all(
    entry.get('differingFields') == 0 for entry in result['gates'].values())
print(json.dumps(result, indent=1))
if len(sys.argv) > 4:
    json.dump(result, open(sys.argv[4], 'w'), indent=1)
