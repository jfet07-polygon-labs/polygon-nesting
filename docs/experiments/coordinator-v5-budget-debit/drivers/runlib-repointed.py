#!/usr/bin/env python3
"""Shared invocation helpers for the coordinator-v4 measurement.

A deliberate copy of `coordinator-v3/drivers/runlib.py`, itself a copy of the
opportunity ledger's invocation half
plus `pr7-coordinator-v2/drivers/lib.py`'s request table, rather than an import
of either: this stage has to run *the same runs those stages ran* - same
request, same pinned positional tail, same salt sets, same `0.002`
search-offset allowance - and a copy that can be diffed is a better guarantee
of that than a path that can drift.

Every run is from the bare request: argument 43 (the pinned parent fixture) and
argument 46 (the warm start) are always empty.

Every arm of every A/B below is the *same binary*, selected by portfolio spec
keys (`v3=`, `sched=`, `barren=`, `divq=`), so a paired comparison is two
processes of one build rather than two builds compared across days.

The reference arm - "merged-HEAD v3" - is `v3=1,sched=0,barren=0,divq=0`.
"""
import json
import os
import subprocess
import time

ROOT = '/tmp/topo-work-wf48'
# The measurement binary. Overridable so one driver can be pointed at the
# gate build, the schedule build and the pristine base-commit build without
# three copies of the driver.
BIN = os.environ.get(
    'V4_BIN', f'{ROOT}/target/release/examples/general_request_benchmark')
OUT = '/var/lib/t3/tmp/v4'

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

# The work anchors, from coordinator v2's own: 40M is the ten-second class
# (32.3M actually spent on mixed-61 seed 1), 120M the thirty-second class.
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
