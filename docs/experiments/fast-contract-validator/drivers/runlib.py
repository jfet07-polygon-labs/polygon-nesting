#!/usr/bin/env python3
"""Shared invocation helpers for the opportunity ledger and the A/B/C probe.

Deliberately a copy of `pr7-coordinator-v2/drivers/lib.py`'s invocation half
rather than an import of it: this stage has to run *the same runs that stage
ran* - same request, same pinned positional tail, same salt sets, same
`0.002` search-offset allowance - and a copy that can be diffed is a better
guarantee of that than a path that can drift.

Every run is from the bare request: argument 43 (the pinned parent fixture) and
argument 46 (the warm start) are always empty.
"""
import json
import os
import subprocess
import time

ROOT = '/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_4111958b-3b3-1'
OUT = '/var/lib/t3/tmp/ledger'

REQUESTS = {
    'mixed-61': f'{ROOT}/tests/fixtures/mixed-61/'
                'mixed61-request-exact-clearance.json',
}

# The pinned CLI tail, byte for byte the PR7 / coordinator-v2 one.
# Slot 26 is the relaxed seed.
ARGS = ('1 4 0 0 0 0 1 0 0 1 1 0 16 4 8 0 0 5 5 24 8 40 10 10 5 {seed} '
        '0.005 0.001 1 6 0 0 0 structured 0 10 1 0 0 0 0').split()
DEFAULT_ALLOWANCE = '0.002'

# The void-grid cell divisor salt sets the diversify phase cycles over its
# slots, unchanged from PR7 and from coordinator v2.
SALT_SETS = {
    0: '13:15:17:19',
    1: '11:15:21:27',
    2: '15:23:31:39',
}

# The work-unit budgets, from `pr7-coordinator-v2`'s own anchors: 40M is the
# ten-second class (32.3M actually spent, mixed-61 seed 1), so the
# thirty-second class is three times it.
WORK_10S = 40_000_000
WORK_30S = 120_000_000


def argv(binary, request, seed, spec, allowance=DEFAULT_ALLOWANCE, runs=1):
    args = [a.format(seed=seed) for a in ARGS]
    args[0] = str(runs)
    tail = ['0', '', '', '', allowance]
    if spec:
        tail.append(spec)
    return [binary, REQUESTS.get(request, request)] + args + tail


def run(binary, request, seed, spec, out_path, allowance=DEFAULT_ALLOWANCE,
        runs=1):
    """One process. Returns (json, wall_seconds, stderr_tail)."""
    os.makedirs(os.path.dirname(out_path), exist_ok=True)
    env = dict(os.environ)
    env.pop('POLYGON_NESTING_PROFILE', None)
    env.pop('POLYGON_NESTING_QUALITY_TRACE', None)
    command = argv(binary, request, seed, spec, allowance, runs)
    started = time.monotonic()
    with open(out_path, 'w') as handle:
        result = subprocess.run(command, stdout=handle, stderr=subprocess.PIPE,
                                check=False, env=env)
    wall = time.monotonic() - started
    stderr = (result.stderr or b'').decode()[-1200:]
    try:
        with open(out_path) as handle:
            return json.load(handle), wall, stderr
    except json.JSONDecodeError:
        return {'_loadError': stderr, '_exitCode': result.returncode}, wall, \
            stderr


def spec_for(seed, work, extra=''):
    spec = f'work={work},cells={SALT_SETS[seed % len(SALT_SETS)]}'
    return spec + (',' + extra if extra else '')
