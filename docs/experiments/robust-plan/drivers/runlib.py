#!/usr/bin/env python3
"""Shared invocation helpers, repointed at this worktree.

A diffable copy of `docs/experiments/coordinator-v4/drivers/runlib.py` with
`ROOT`, `BIN` and `OUT` repointed and nothing else changed: the same request
table, the same pinned positional tail, the same salt sets, the same `0.002`
search-offset allowance, and the bare request every time (argument 43, the
pinned parent fixture, and argument 46, the warm start, are always empty).
"""
import json
import os
import subprocess
import time

ROOT = '/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_f03cd94d-c01-2'
# The measurement binary. Overridable so one driver can be pointed at the
# baseline build and the patched build without two copies of the driver.
BIN = os.environ.get(
    'PLAN_BIN', '/var/lib/t3/tmp/robust/bin/ship-meas')
OUT = os.environ.get('PLAN_OUT', '/var/lib/t3/tmp/robust/out')

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

# The void-grid cell divisor salt sets the diversify class cycles over its
# slots, unchanged from PR7, coordinator v2 and the ledger.
SALT_SETS = {
    0: '13:15:17:19',
    1: '11:15:21:27',
    2: '15:23:31:39',
}

# Appended to by `run`, drained by whichever driver wants it. A list rather
# than a running mean because the interesting statistic is the *maximum* - one
# battery run under a load spike is one number a reader has to be able to find.
LOAD = []

WORK_10S = 40_000_000
WORK_30S = 120_000_000
WORK_3S = 12_000_000


def argv(binary, request, seed, spec, allowance=DEFAULT_ALLOWANCE, runs=1):
    args = [a.format(seed=seed) for a in ARGS]
    args[0] = str(runs)
    tail = ['0', '', '', '', allowance]
    if spec:
        tail.append(spec)
    return [binary, REQUESTS.get(request, request)] + args + tail


# Every wall number in this round was taken on a box that was **not quiet**: a
# second measurement campaign (`docs/experiments/basin-race/`) was running in
# parallel for part of the window. That is not a caveat to be discovered later
# from the numbers, so it is recorded per run: `os.getloadavg()` immediately
# before and after each process, carried into every driver's rows.
#
# It is the reason `LOAD` exists rather than a note at the end of a README.
def load_now():
    try:
        return os.getloadavg()
    except OSError:
        return (None, None, None)


def run(binary, request, seed, spec, out_path, allowance=DEFAULT_ALLOWANCE,
        runs=1, trace_path=None):
    """One process. Returns (json, wall_seconds, stderr_tail)."""
    os.makedirs(os.path.dirname(out_path), exist_ok=True)
    env = dict(os.environ)
    env.pop('POLYGON_NESTING_PROFILE', None)
    env.pop('POLYGON_NESTING_QUALITY_TRACE', None)
    env.pop('POLYGON_NESTING_QUALITY_TRACE_COUNTERS', None)
    if trace_path:
        os.makedirs(os.path.dirname(trace_path), exist_ok=True)
        env['POLYGON_NESTING_QUALITY_TRACE'] = trace_path
        env['POLYGON_NESTING_QUALITY_TRACE_COUNTERS'] = '0'
    command = argv(binary, request, seed, spec, allowance, runs)
    load_before = load_now()
    started = time.monotonic()
    with open(out_path, 'w') as handle:
        result = subprocess.run(command, stdout=handle, stderr=subprocess.PIPE,
                                check=False, env=env)
    wall = time.monotonic() - started
    LOAD.append({'out': out_path, 'wall': wall,
                 'before': load_before[0], 'after': load_now()[0]})
    stderr = (result.stderr or b'').decode()[-1200:]
    try:
        with open(out_path) as handle:
            return json.load(handle), wall, stderr
    except json.JSONDecodeError:
        return {'_loadError': stderr, '_exitCode': result.returncode}, wall, \
            stderr


def spec_for(seed, budget_key, budget_value, v3, extra=''):
    """`budget_key` is 'work' or 'wall'."""
    spec = (f'{budget_key}={budget_value},'
            f'cells={SALT_SETS[seed % len(SALT_SETS)]},v3={1 if v3 else 0}')
    return spec + (',' + extra if extra else '')


def summarize(tag, doc, seconds):
    row = {
        'tag': tag,
        'processSeconds': seconds,
        'engineDepthMm': doc.get('independentUsedLongAxisDepthMm'),
    }
    portfolio = doc.get('portfolio')
    if portfolio:
        row['rawDepthMm'] = portfolio['incumbent']['rawDepthMm']
        row['dualGateValid'] = portfolio['incumbent']['dualGateValid']
        row['publishedSeconds'] = portfolio['incumbent']['publishedSeconds']
        row['publishedWorkUnits'] = \
            portfolio['incumbent']['publishedWorkUnits']
        row['incumbentSource'] = portfolio['incumbent']['source']
        row['coordinatorSeconds'] = portfolio['elapsedSeconds']
        row['workUnits'] = portfolio['workUnits']
        row['phases'] = portfolio['phases']
        row['publications'] = portfolio['publications']
        row['operatorCalls'] = portfolio['operatorCalls']
        row['archive'] = portfolio['archive']
        row['schedule'] = portfolio.get('schedule')
    else:
        row['rawDepthMm'] = doc.get('rawSourceDepthMm')
    if '_loadError' in doc:
        row['loadError'] = doc['_loadError']
    return row
