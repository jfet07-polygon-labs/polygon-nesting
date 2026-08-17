#!/usr/bin/env python3
"""Baseline-vs-treatment parity for every mode the change must not touch."""

import json
import os
import subprocess
import sys

sys.path.insert(0, '/var/lib/t3/tmp/mode31')
from run import ARGS, REQ, RECORD, SCRATCH  # noqa: E402

BASE = '/var/lib/t3/tmp/mode31-baseline'
TREAT = '/var/lib/t3/tmp/mode31-bench'
OUT = '/var/lib/t3/tmp/mode31/parity'

# Fields that legitimately differ run to run (wall clock, environment).
VOLATILE = {'elapsedMs', 'wallClockMs', 'durationMs', 'timestamp', 'hostname',
            'runElapsedMs', 'elapsedMillis', 'totalElapsedMs'}


def strip(node):
    if isinstance(node, dict):
        return {k: strip(v) for k, v in node.items()
                if k not in VOLATILE and 'ElapsedMs' not in k
                and 'Millis' not in k and 'Nanos' not in k
                and 'DurationMs' not in k}
    if isinstance(node, list):
        return [strip(v) for v in node]
    return node


def run(binary, tag, mode, parent, target, seed, request=REQ, allowance='0.0005'):
    os.makedirs(OUT, exist_ok=True)
    argv = [binary, request] + [a.format(clamp='0', seed=seed) for a in ARGS] + [
        str(mode), parent, str(target), '', allowance]
    path = f'{OUT}/{tag}.json'
    with open(path, 'w') as out:
        subprocess.run(argv, stdout=out, stderr=subprocess.DEVNULL, check=False)
    with open(path) as handle:
        return strip(json.load(handle))


CASES = [
    # Mode 0 never reaches the vacancy population, so the pinned parent is
    # loaded and unused; it is here only to keep the positional tail valid.
    ('mode0', 0, RECORD, '0', 0),
    ('mode11-record', 11, RECORD, '163.5', 0),
    ('mode17-record', 17, RECORD, '163.5', 0),
    ('mode20-salt320', 20, '/var/lib/t3/tmp/ex5-seed-native.json', '320.000', 0),
    ('mode22-record', 22, RECORD, '164.842', 0),
    ('mode23-record', 23, RECORD, '0.5', 0),
    ('mode24-record', 24, RECORD, '163.8', 0),
    ('mode27-record', 27, RECORD, '0', 0),
    ('mode28-record', 28, RECORD, '163.8', 0),
    ('mode29-record', 29, RECORD, '163.8', 0),
    ('mode27-scratch', 27, SCRATCH, '0', 0),
    ('mode28-scratch', 28, SCRATCH, '164.4', 0),
    ('mode29-scratch', 29, SCRATCH, '164.4', 0),
]

if __name__ == '__main__':
    failures = 0
    for tag, mode, parent, target, seed in CASES:
        base = run(BASE, f'{tag}-base', mode, parent, target, seed)
        treat = run(TREAT, f'{tag}-treat', mode, parent, target, seed)
        same = base == treat
        failures += 0 if same else 1
        print(f'{tag}: {"IDENTICAL" if same else "*** DIFFERS ***"}')
    print(f'differing cases: {failures}/{len(CASES)}')
