#!/usr/bin/env python3
"""Shared invocation helpers for the quality frontier trace.

This is the finer-ladder driver's ARGS contract with two differences, both
deliberate:

  * `ROOT`/`BIN` point at this worktree, and `BIN` is chosen per call so the
    same driver can drive the base (trace compiled out) and trace binaries;
  * every run here is FROM REQUEST ONLY - argument 43 (the pinned parent
    fixture) is always the empty string, and argument 46 (the warm start) is
    always empty. A curve drawn from a pinned parent measures a replay, not a
    search, which is exactly the substitution the review rejected.
"""

import json
import os
import subprocess
import time

ROOT = '/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_8df8fa5c-4d3-1'
BIN_DIR = '/var/lib/t3/tmp/qft/bin'
BASE_BIN = f'{BIN_DIR}/base'
TRACE_BIN = f'{BIN_DIR}/trace'
REQ = f'{ROOT}/tests/fixtures/mixed-61/mixed61-request-exact-clearance.json'
TRUE = (f'{ROOT}/docs/experiments/persistent-vacancy-descent/exact-contract/'
        'true-contract')
LADDER = f'{TRUE}/finer-ladder'

# The pinned CLI tail, byte for byte the finer-ladder one. Slot 16
# (sheet-long-axis-override) stays 0; slot 26 is the relaxed seed.
ARGS = ('1 4 0 0 0 0 1 0 0 1 1 0 16 4 8 {clamp} 0 5 5 24 8 40 10 10 5 {seed} '
        '0.005 0.001 1 6 0 0 0 structured 0 10 1 0 0 0 0').split()
ALLOWANCE = '0.0005'


def argv(binary, mode, parent, target, seed, warm='', clamp='0',
         allowance=ALLOWANCE):
    tail = [a.format(clamp=clamp, seed=seed) for a in ARGS]
    return ([binary, REQ] + tail
            + [str(mode), parent, '' if target is None else str(target), warm,
               allowance])


def run(binary, mode, parent, target, seed, out_path, trace_path=None,
        warm='', clamp='0', allowance=ALLOWANCE, profile=False):
    """One process. Returns (json, wall_seconds)."""
    os.makedirs(os.path.dirname(out_path), exist_ok=True)
    env = dict(os.environ)
    env.pop('POLYGON_NESTING_QUALITY_TRACE', None)
    env.pop('POLYGON_NESTING_PROFILE', None)
    if trace_path:
        os.makedirs(os.path.dirname(trace_path), exist_ok=True)
        env['POLYGON_NESTING_QUALITY_TRACE'] = trace_path
    if profile:
        env['POLYGON_NESTING_PROFILE'] = '1'
    command = argv(binary, mode, parent, target, seed, warm, clamp, allowance)
    started = time.monotonic()
    with open(out_path, 'w') as out:
        result = subprocess.run(command, stdout=out, stderr=subprocess.PIPE,
                                check=False, env=env)
    wall = time.monotonic() - started
    try:
        with open(out_path) as handle:
            return json.load(handle), wall
    except json.JSONDecodeError:
        return {'_loadError': (result.stderr or b'').decode()[-600:]}, wall


def population(run_json):
    if '_loadError' in run_json:
        return None
    coupled = (run_json.get('relaxedDiagnostics') or {}).get(
        'coupledDynamicSeparator')
    return (coupled or {}).get('persistentVacancyPopulation')


def gate_row(run_json):
    """The four fields every pinned regression gate is quoted in."""
    pop = population(run_json) or {}
    return {
        'independentDepthMm': run_json.get('independentUsedLongAxisDepthMm'),
        'placementFingerprint': run_json.get('finalPlacementFingerprint'),
        'modeRaw': pop.get('rawSourceDepthMm'),
        'modeDepth': pop.get('independentDepthMm'),
        'modeFingerprint': pop.get('finalPlacementFingerprint'),
        'exactValid': pop.get('exactValid'),
        'contractValid': pop.get('contractValid'),
    }


def read_trace(path):
    events = []
    with open(path) as handle:
        for line in handle:
            line = line.strip()
            if line:
                events.append(json.loads(line))
    return events
