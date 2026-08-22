#!/usr/bin/env python3
"""Shared invocation helpers, repointed at this worktree.

A diffable copy of `docs/experiments/sparse-rotation/drivers/runlib.py` with
`ROOT`, `BIN` and `OUT` repointed and nothing else changed: the same request
table, the same pinned positional tail, the same salt sets, the same `0.002`
search-offset allowance.
"""
import json
import os
import subprocess
import time

ROOT = '/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_f674db6b-1e0-2'
BIN = os.environ.get('MB_BIN', '/var/lib/t3/tmp/m26band/bin/meas')
OUT = os.environ.get('MB_OUT', '/var/lib/t3/tmp/m26band/out')

REQUESTS = {
    'mixed-61': f'{ROOT}/tests/fixtures/mixed-61/'
                'mixed61-request-exact-clearance.json',
    'shapes-17': f'{ROOT}/tests/fixtures/shapes-17/2000x2700-compact/'
                 'request.json',
    'triangle-20': f'{ROOT}/tests/fixtures/triangle-20/2000x2700-compact/'
                   'request.json',
}

# The pinned CLI tail, byte for byte the PR7 / coordinator-v2 / ledger one.
# Slot 26 is the relaxed seed.
ARGS = ('1 4 0 0 0 0 1 0 0 1 1 0 16 4 8 0 0 5 5 24 8 40 10 10 5 {seed} '
        '0.005 0.001 1 6 0 0 0 structured 0 10 1 0 0 0 0').split()
DEFAULT_ALLOWANCE = '0.002'

SALT_SETS = {
    0: '13:15:17:19',
    1: '11:15:21:27',
    2: '15:23:31:39',
}


def probe(binary, request, seed, fixture, env_extra, out_path,
          allowance=DEFAULT_ALLOWANCE, timeout=1800):
    """One diagnostic process against a pinned parent. Mode 0, no target.

    The diagnostic door returns before any search runs, so the mode and the
    target are inert - but they are still passed exactly as every other replay
    driver in this repository passes them, because the positional tail is a
    pinned contract and a driver that abbreviates it is measuring a different
    command.
    """
    args = [a.format(seed=seed) for a in ARGS]
    command = ([binary, REQUESTS.get(request, request)] + args
               + ['0', fixture, '0', '', allowance])
    env = dict(os.environ)
    env.pop('POLYGON_NESTING_PROFILE', None)
    env.update(env_extra)
    os.makedirs(os.path.dirname(out_path), exist_ok=True)
    started = time.monotonic()
    with open(out_path, 'w') as handle:
        proc = subprocess.run(command, stdout=handle, stderr=subprocess.PIPE,
                              check=False, env=env, timeout=timeout)
    wall = time.monotonic() - started
    stderr = (proc.stderr or b'').decode()[-1500:]
    try:
        return json.load(open(out_path)), wall, stderr, proc.returncode
    except json.JSONDecodeError:
        return None, wall, stderr, proc.returncode
