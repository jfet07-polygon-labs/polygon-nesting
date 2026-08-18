#!/usr/bin/env python3
"""Shared invocation helpers for the v2 coordinator measurement.

Two differences from the PR7 driver library, both deliberate:

  * every run is FROM THE BARE REQUEST - argument 43 (the pinned parent
    fixture) and argument 46 (the warm start) are always empty, on every
    request, because a curve drawn from a pinned parent measures a replay;
  * the request is a *parameter*, not a constant, because the owner's standing
    generality mandate is that the coordinator has to run on shapes-17 and
    triangle-20 from the same code path with the same derived budgets.

Both coordinator binaries are driven from here: the v1 binary is built from
`8d9f7e5` and the v2 binary from this worktree, so every A/B between the two
schedules is a paired interleaved comparison of two processes rather than of
two numbers taken on different days.
"""

import json
import os
import subprocess
import time

ROOT = '/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_8f665a53-b7b-1'
V1_BIN = '/var/lib/t3/tmp/pr7v2-measure-v1'
V2_BIN = '/var/lib/t3/tmp/pr7v2-measure-v2'
OUT = '/var/lib/t3/tmp/pr7v2'

REQUESTS = {
    'mixed-61': f'{ROOT}/tests/fixtures/mixed-61/'
                'mixed61-request-exact-clearance.json',
    'shapes-17': f'{ROOT}/tests/fixtures/shapes-17/2000x2700-compact/'
                 'request.json',
    'triangle-20': f'{ROOT}/tests/fixtures/triangle-20/2000x2700-compact/'
                   'request.json',
}

# The pinned CLI tail, byte for byte the PR7 one. Slot 26 is the relaxed seed.
ARGS = ('1 4 0 0 0 0 1 0 0 1 1 0 16 4 8 0 0 5 5 24 8 40 10 10 5 {seed} '
        '0.005 0.001 1 6 0 0 0 structured 0 10 1 0 0 0 0').split()
DEFAULT_ALLOWANCE = '0.002'

# The salt sets, unchanged from PR7: void-grid cell divisors the diversify
# phase cycles over its slots. They are tickets, not tunings.
SALT_SETS = {
    0: '13:15:17:19',
    1: '11:15:21:27',
    2: '15:23:31:39',
}


def argv(binary, request, seed, spec, allowance=DEFAULT_ALLOWANCE, runs=1):
    args = [a.format(seed=seed) for a in ARGS]
    args[0] = str(runs)
    tail = ['0', '', '', '', allowance]
    if spec:
        tail.append(spec)
    return [binary, REQUESTS.get(request, request)] + args + tail


def run(binary, request, seed, spec, out_path, trace_path=None,
        allowance=DEFAULT_ALLOWANCE, runs=1, counters=False):
    """One process. Returns (json, wall_seconds, stderr_tail)."""
    os.makedirs(os.path.dirname(out_path), exist_ok=True)
    env = dict(os.environ)
    env.pop('POLYGON_NESTING_PROFILE', None)
    env.pop('POLYGON_NESTING_QUALITY_TRACE', None)
    env.pop('POLYGON_NESTING_QUALITY_TRACE_COUNTERS', None)
    if trace_path:
        os.makedirs(os.path.dirname(trace_path), exist_ok=True)
        env['POLYGON_NESTING_QUALITY_TRACE'] = trace_path
        env['POLYGON_NESTING_QUALITY_TRACE_COUNTERS'] = \
            '1' if counters else '0'
    command = argv(binary, request, seed, spec, allowance, runs)
    started = time.monotonic()
    with open(out_path, 'w') as handle:
        result = subprocess.run(command, stdout=handle, stderr=subprocess.PIPE,
                                check=False, env=env)
    wall = time.monotonic() - started
    stderr = (result.stderr or b'').decode()[-800:]
    try:
        with open(out_path) as handle:
            return json.load(handle), wall, stderr
    except json.JSONDecodeError:
        return {'_loadError': stderr, '_exitCode': result.returncode}, wall, \
            stderr


def incumbent_series(trace_path):
    """The depth-versus-time curve, joined to raw source depth.

    `incumbent` events carry the grid-snapped depth; `exactCandidate` events
    carry the raw f64 reading for the same fingerprint. Identical to the PR7
    driver so the two stages' curves are the same measurement.
    """
    series = []
    raw_by_fingerprint = {}
    try:
        handle = open(trace_path)
    except OSError:
        return series
    with handle:
        for line in handle:
            try:
                event = json.loads(line)
            except json.JSONDecodeError:
                continue
            kind = event.get('event')
            if kind == 'exactCandidate':
                raw_by_fingerprint.setdefault(event.get('fingerprint'),
                                              event.get('rawDepthMm'))
            elif kind == 'incumbent':
                fingerprint = event.get('fingerprint')
                series.append({
                    't': event['t'],
                    'depthMm': event['depthMm'],
                    'rawDepthMm': raw_by_fingerprint.get(fingerprint),
                    'source': event.get('source'),
                    'operator': event.get('operator'),
                    'fingerprint': fingerprint,
                })
    return series


def summarize(tag, doc, seconds, trace_path=None):
    row = {
        'tag': tag,
        'processSeconds': seconds,
        'engineDepthMm': doc.get('independentUsedLongAxisDepthMm'),
    }
    if trace_path:
        row['incumbentSeries'] = incumbent_series(trace_path)
    portfolio = doc.get('portfolio')
    if portfolio:
        row['rawDepthMm'] = portfolio['incumbent']['rawDepthMm']
        row['dualGateValid'] = portfolio['incumbent']['dualGateValid']
        row['publishedSeconds'] = portfolio['incumbent']['publishedSeconds']
        row['coordinatorSeconds'] = portfolio['elapsedSeconds']
        row['descentStalled'] = portfolio.get('descentStalled')
        row['phases'] = portfolio['phases']
        row['publications'] = portfolio['publications']
        row['operatorCalls'] = portfolio['operatorCalls']
        row['archive'] = portfolio['archive']
        row['areaLowerBoundDepthMm'] = portfolio['areaLowerBoundDepthMm']
        row['constructedDepthMm'] = portfolio['constructedDepthMm']
    else:
        row['rawDepthMm'] = doc.get('rawSourceDepthMm')
    if '_loadError' in doc:
        row['loadError'] = doc['_loadError']
    return row
