#!/usr/bin/env python3
"""Shared runner for the four pinned regression gates.

A diffable copy of `docs/experiments/calibrated-plan/drivers/gatelib.py` with
`ROOT` repointed at this worktree and nothing else changed: the same four gates,
the same pinned CLI tail, the same `doc_digest` with the elapsed-derived summary
statistics and `engineWorktreeStatus` stripped.
"""
import hashlib
import json
import os
import subprocess
import time

ROOT = '/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_f674db6b-1e0-2'
REQ = f'{ROOT}/tests/fixtures/mixed-61/mixed61-request-exact-clearance.json'
TRUE = (f'{ROOT}/docs/experiments/persistent-vacancy-descent/exact-contract/'
        'true-contract')

ARGS = ('1 4 0 0 0 0 1 0 0 1 1 0 16 4 8 {clamp} 0 5 5 24 8 40 10 10 5 {seed} '
        '0.005 0.001 1 6 0 0 0 structured 0 10 1 0 0 0 0').split()

# tag, mode, parent, target, trailing allowance, expected raw depth,
# expected fingerprint prefix.
GATES = [
    ('g1', 20, '/var/lib/t3/tmp/ex5-seed-native.json', '320.000', None,
     206.869, '8a7737381238fa4d'),
    ('g2', 22, f'{TRUE}/record-159.092/pinned-parent-159.092.json',
     '159.892624', '0.0005', 159.09233022733062, 'fa01012af1d559ae09c'),
    ('g3', 22, f'{TRUE}/finer-ladder/pinned-parent-159.079.json',
     '159.87876', '0.0005', 159.07876040364795, 'e28fba007f8031d49f'),
    ('g4', 22, f'{TRUE}/finer-ladder/pinned-fs-parent-164.0376.json',
     '164.837568', '0.0005', 164.0375677990678, '49f094d7e59a9008'),
]


def argv_for(binary, mode, parent, target, allowance, seed='5', clamp='0'):
    argv = ([binary, REQ] + [a.format(clamp=clamp, seed=seed) for a in ARGS]
            + [str(mode), parent, str(target)])
    if allowance:
        argv += ['', allowance]
    return argv


def run(binary, tag, mode, parent, target, allowance, outdir, env=None,
        seed='5'):
    os.makedirs(outdir, exist_ok=True)
    path = f'{outdir}/{tag}.json'
    environment = dict(os.environ)
    # The gates are run on the shipping benchmark, so no round flag may leak in
    # from the shell that launched the driver.
    for name in ('POLYGON_NESTING_CONTACT_BLOCK', 'POLYGON_NESTING_SE2_WITNESS',
                 'POLYGON_NESTING_SE2_CERTIFICATE', 'POLYGON_NESTING_PROFILE'):
        environment.pop(name, None)
    if env:
        environment.update(env)
    start = time.time()
    with open(path, 'w') as handle:
        proc = subprocess.run(
            argv_for(binary, mode, parent, target, allowance, seed=seed),
            stdout=handle, stderr=subprocess.PIPE, env=environment, check=False)
    wall = time.time() - start
    try:
        doc = json.load(open(path))
    except json.JSONDecodeError:
        doc = {'_loadError': (proc.stderr or b'').decode()[-2000:]}
    return doc, wall, (proc.stderr or b'').decode()


def run_gate(binary, gate, outdir, env=None, label=''):
    tag, mode, parent, target, allowance = gate[:5]
    return run(binary, f'{label}{tag}', mode, parent, target, allowance,
               outdir, env=env)


def collect(node, keys, path='', out=None):
    if out is None:
        out = {}
    if isinstance(node, dict):
        for key, value in node.items():
            if key in keys and not isinstance(value, (dict, list)):
                out.setdefault(f'{path}/{key}', value)
            collect(value, keys, f'{path}/{key}', out)
    elif isinstance(node, list):
        for index, value in enumerate(node):
            collect(value, keys, f'{path}[{index}]', out)
    return out


DEPTH_KEYS = ('rawSourceDepthMm', 'rawDepthMm', 'independentDepthMm',
              'independentUsedLongAxisDepthMm')
PRINT_KEYS = ('finalPlacementFingerprint', 'placementFingerprint',
              'fingerprint')

# Every field whose value is a wall-clock reading, and the summary statistics
# derived from them. Stripped before digesting so two builds of the same
# trajectory digest equal.
TIME_KEYS = {'elapsedMs', 'wallSeconds', 'elapsedSeconds', 'publishedSeconds',
             'coordinatorSeconds', 'repairMs', 'confirmationMs', 'se2WitnessMs',
             'rotationSurrogateBuildMs', 'ms', 'millis', 'durationMs',
             'engineWorktreeStatus', 'timestamp', 'startedAt', 'finishedAt',
             'medianSeconds', 'meanSeconds', 'minSeconds', 'maxSeconds',
             'p95Seconds', 'stdevSeconds', 'totalSeconds', 'buildMs',
             'solveMs', 'scoreMs', 'sweepMs', 'phaseMs', 'evaluationsPerSecond',
             'candidateQueriesPerSecond'}


def strip_times(node):
    if isinstance(node, dict):
        return {key: strip_times(value) for key, value in node.items()
                if key not in TIME_KEYS
                and not key.endswith('Ms')
                and not key.endswith('Seconds')
                and not key.endswith('PerSecond')}
    if isinstance(node, list):
        return [strip_times(value) for value in node]
    return node


def doc_digest(doc):
    stripped = strip_times(doc)
    return hashlib.sha256(
        json.dumps(stripped, sort_keys=True, separators=(',', ':'))
        .encode()).hexdigest()


def gate_check(gate, doc):
    tag, _, _, _, _, want_depth, want_print = gate
    depths = collect(doc, DEPTH_KEYS)
    prints = collect(doc, PRINT_KEYS)
    depth_hit = any(abs(value - want_depth) < 5e-7 for value in depths.values()
                    if isinstance(value, (int, float)))
    print_hit = any(isinstance(value, str) and value.startswith(want_print)
                    for value in prints.values())
    return {
        'gate': tag,
        'wantDepthMm': want_depth,
        'wantFingerprintPrefix': want_print,
        'depthHit': depth_hit,
        'fingerprintHit': print_hit,
        'hit': bool(depth_hit and print_hit),
        'depthsSeen': sorted({round(v, 9) for v in depths.values()
                              if isinstance(v, (int, float))})[:6],
        'loadError': doc.get('_loadError'),
    }
