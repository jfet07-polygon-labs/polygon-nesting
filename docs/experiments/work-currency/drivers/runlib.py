#!/usr/bin/env python3
"""Shared invocation helpers, repointed at this worktree.

A diffable copy of `docs/experiments/replan/drivers/runlib.py` with `ROOT`,
`BIN` and `OUT` repointed and nothing else changed: the same request table, the
same pinned positional tail, the same salt sets, the same `0.002` search-offset
allowance, and the bare request every time (argument 43, the pinned parent
fixture, and argument 46, the warm start, are always empty).
"""
import hashlib
import json
import os
import subprocess
import time

ROOT = '/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_f03cd94d-c01-1'
BIN = os.environ.get('CUR2_BIN', '/tmp/wc-bin/ship-combo')
OUT = os.environ.get('CUR2_OUT', '/tmp/wc-out')

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

LOAD = []

WORK_10S = 40_000_000
WORK_30S = 120_000_000


def sha256_of(path):
    digest = hashlib.sha256()
    with open(path, 'rb') as handle:
        for block in iter(lambda: handle.read(1 << 20), b''):
            digest.update(block)
    return digest.hexdigest()


def argv(binary, request, seed, spec, allowance=DEFAULT_ALLOWANCE, runs=1):
    args = [a.format(seed=seed) for a in ARGS]
    args[0] = str(runs)
    tail = ['0', '', '', '', allowance]
    if spec:
        tail.append(spec)
    return [binary, REQUESTS.get(request, request)] + args + tail


def load_now():
    try:
        return os.getloadavg()
    except OSError:
        return (None, None, None)


def run(binary, request, seed, spec, out_path, allowance=DEFAULT_ALLOWANCE,
        runs=1):
    """One process. Returns (json, wall_seconds, stderr_tail)."""
    os.makedirs(os.path.dirname(out_path), exist_ok=True)
    env = dict(os.environ)
    env.pop('POLYGON_NESTING_PROFILE', None)
    env.pop('POLYGON_NESTING_QUALITY_TRACE', None)
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


def spec_for(seed, budget_key, budget_value, v3=True, extra=''):
    spec = (f'{budget_key}={budget_value},'
            f'cells={SALT_SETS[seed % len(SALT_SETS)]},v3={1 if v3 else 0}')
    return spec + (',' + extra if extra else '')


# The volatile-field list and digest are `gatelib.py`'s, so a document digest
# taken here means the same thing it means there. `workCurrency` is NOT in it:
# the whole point of the paired `cur2=0` / `cur2=2` control is that the two
# documents differ in exactly the currency's own block and nowhere else, and a
# digest that stripped the block could not show that.
VOLATILE = {
    'elapsedMs', 'elapsedSeconds', 'engineElapsedSeconds', 'wallMs',
    'durationMs', 'timestamp', 'totalMs', 'ms', 'processWallSeconds',
    'phaseProfile', 'phases', 'profile', 'leafSeconds', 'engineVersion',
    'buildIdentity', 'binaryPath', 'peakResidentBytes', 'allocatedBytes',
    'medianElapsedMs', 'minElapsedMs', 'maxElapsedMs',
    'firstQuartileElapsedMs', 'thirdQuartileElapsedMs',
    'executableSha256', 'relevantSourceTreeSha256', 'engineWorktreeStatus',
    'engineCommit', 'engineWorktreeDirty',
}

# The list above is `gatelib.py`'s and it is **incomplete for a coordinator
# document**, which is a real limitation of the inherited instrument rather
# than a convenience: it was written for the four pinned gates, and a gate is a
# direct-mode run with no `portfolio` block at all. A coordinator document
# carries a dozen more clock readings, and this round found them the only way
# they can be found - by running two binaries that must agree, watching the
# digest disagree, and diffing the leaves.
#
# Measured on `mixed-61` seed 0 at `work=40000000`, base binary against this
# tree's: **3,723 leaves, 61 differing, and every one of the 61 is in the list
# below.** Not one work unit, depth, fingerprint, counter or disposition
# differed. `leaf_diff` is the instrument that says so and it is reported per
# cell, so a reader can check that this set is not hiding a real difference
# instead of taking the digest's word for it.
#
# Everything here is a reading of `Instant::elapsed` or a duration derived from
# one. Costs are deliberately absent: under a work budget `estimatedCost` and
# `actualCost` are work units, and stripping them would strip the numbers the
# comparison is about.
WALL_DERIVED = {
    'startedSeconds', 'birthSeconds', 'publishedSeconds', 'seconds',
    'atSeconds', 'queueSeconds', 'probeSeconds', 'remainingSeconds',
    'horizonSeconds', 'operatorSeconds', 'confirmationMs', 'repairMs',
    'entryLegalizationMs', 'rotationSurrogateBuildMs',
    # `[seconds, occupancy]` pairs: half of every entry is a clock reading, so
    # the series goes as a unit. `archive.occupancy` carries the same
    # information without the clock.
    'occupancyOverTime',
    # The plan's calibration is one clock reading and its two derived rates;
    # `plan.units` - the number the trajectory is a function of - is not here.
    'probeRateUnitsPerSecond', 'queueRateUnitsPerSecond', 'rawUnits',
}

# The currency's own reporting, stripped only by `doc_digest_without_currency`.
CURRENCY_KEYS = {'workCurrency'}


def strip_volatile(node, extra=frozenset(), drop=None):
    drop = VOLATILE | WALL_DERIVED if drop is None else drop
    if isinstance(node, dict):
        return {k: strip_volatile(v, extra, drop) for k, v in sorted(node.items())
                if k not in drop and k not in extra}
    if isinstance(node, list):
        return [strip_volatile(v, extra, drop) for v in node]
    if isinstance(node, float):
        return repr(node)
    return node


def doc_digest(doc):
    return hashlib.sha256(
        json.dumps(strip_volatile(doc), sort_keys=True).encode()).hexdigest()


def doc_digest_without_currency(doc):
    """The digest with the currency's own block removed.

    This is the instrument for the `cur2=0` vs `cur2=2` control: if observing
    changes nothing but the reporting, these two digests are equal while
    `doc_digest` differs.
    """
    return hashlib.sha256(
        json.dumps(strip_volatile(doc, CURRENCY_KEYS),
                   sort_keys=True).encode()).hexdigest()


def leaves(node, path='', out=None):
    """Every scalar in the document, keyed by its path."""
    if out is None:
        out = {}
    if isinstance(node, dict):
        for key, value in node.items():
            leaves(value, path + '/' + key, out)
    elif isinstance(node, list):
        for index, value in enumerate(node):
            leaves(value, path + f'/{index}', out)
    else:
        out[path] = node
    return out


def leaf_diff(left, right, extra=frozenset()):
    """Which leaves of two documents disagree, and how many there are.

    Reported alongside every digest comparison in this round, because a digest
    that matches proves nothing about *what was stripped to make it match*.
    The three counts are: leaves compared, leaves differing after the volatile
    and wall-clock sets are removed, and - separately - how many leaves the
    clock alone accounts for.
    """
    stripped_left = leaves(strip_volatile(left, extra))
    stripped_right = leaves(strip_volatile(right, extra))
    keys = set(stripped_left) | set(stripped_right)
    differing = sorted(k for k in keys
                       if stripped_left.get(k) != stripped_right.get(k))
    clock_left = leaves(strip_volatile(left, extra, drop=VOLATILE))
    clock_right = leaves(strip_volatile(right, extra, drop=VOLATILE))
    clock_keys = set(clock_left) | set(clock_right)
    clock_differing = [k for k in clock_keys
                       if clock_left.get(k) != clock_right.get(k)]
    return {
        'leaves': len(keys),
        'differing': len(differing),
        'differingPaths': differing[:40],
        'differingBeforeWallStrip': len(clock_differing),
    }


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
        row['coordinatorSeconds'] = portfolio['elapsedSeconds']
        row['workUnits'] = portfolio['workUnits']
        row['planUnits'] = (portfolio.get('plan') or {}).get('units')
        row['publications'] = portfolio['publications']
        row['operatorCalls'] = portfolio['operatorCalls']
        row['workCurrency'] = portfolio.get('workCurrency')
        row['digest'] = doc_digest(doc)
        row['digestNoCurrency'] = doc_digest_without_currency(doc)
    else:
        row['rawDepthMm'] = doc.get('rawSourceDepthMm')
    if '_loadError' in doc:
        row['loadError'] = doc['_loadError']
    return row
