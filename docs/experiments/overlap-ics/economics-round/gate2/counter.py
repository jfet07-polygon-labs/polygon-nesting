#!/usr/bin/env python3
"""**Rider (i): the counter, proven bit-identical BEFORE any coefficient.**

    python3 counter.py [work-dir]

docs/currency-amendment.md, ox-alpha's riders, binding:

> (i) `published_bites` proven bit-identical across runs BEFORE fitting `P`

and, in his own words, why:

> `published_bites` must be an instrumented counter proven bit-identical across
> runs *before* any coefficient is fitted - `P` must not repeat `R`'s fate of
> being fitted to a residual that moves 7× between repetitions of its own
> calibration.

`R` moved by 6.89× between three repetitions of its own calibration and was
never a price. That happened because `R` was fitted to a **residual**. `P` is
fitted to a **counter**, and this file is the difference: it runs each of the
three fixed-work cells in **two processes**, and requires the per-bite
`published` vector, its sum, and the whole document apart from the wall to be
identical bit for bit. A red here stops the wave before a coefficient exists.

# Where the counter is, and why it was not added

The amendment asks for an *instrumented, deterministic counter*. One already
exists and has since before this campaign: `BiteRecord::published` is the
trajectory's own publication record, emitted per bite as `"published": <bool>`
by `overlap_ics_benchmark` in **every** build - no feature gate, no clock read,
no branch on it anywhere in the engine - and `published_bites` is its sum over
the cell. It is inside the whole-document two-process comparison every
determinism claim in this campaign already rests on.

So no engine code was edited to create it. A second counter incremented
somewhere else would not be more instrumented; it would be a second source of
truth for one fact, and the first thing anyone would then have to check is that
the two agree. What this file does instead is **prove the one that exists**, and
prove it against a number the engine computes by a different route:
`outcome.publicationCount` is the length of the publication list, built where
publications are appended, and `sum(published)` is a fold over the bite records.
They must agree on every cell, and `publicationCountReconciles` is that check.

# Fixed work, and what identity means here

The cells are the currency calibration's own - `--mode=fixed`, the shape
`meter/currency.py` uses - so the counters are a deterministic function of the
request and the seed. Two processes may differ only in nanoseconds, so the
comparison strips `wall` and nothing else. A document that differs anywhere else
is a determinism failure and not a counter failure, and it is reported as what
it is.

Exit status is the verdict, taken directly and never through a pipe:

* `0` - the counter is bit-identical across two processes on all three
  fixtures, and reconciles with the publication count. `P` may be fitted.
* `1` - it is not. **No coefficient may be fitted**, and the wave stops on
  rider (i) rather than on the reject rule.
* `2` - the check could not run: a missing binary, or a cell that did not
  exit 0.
"""
import hashlib
import importlib.util
import json
import os
import platform
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, '..', '..', '..', '..', '..'))
sys.path.insert(0, f'{ROOT}/docs/experiments/overlap-ics/drivers')
import lib  # noqa: E402

_spec = importlib.util.spec_from_file_location(
    'census_identity', f'{HERE}/../census/identity.py')
census_identity = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(census_identity)
# `wall` plus the census's nanosecond set: everything that is a clock reading
# and nothing that is a counter.
IGNORED_KEYS = set(lib.WALL_FIELDS) | census_identity.TIMING_KEYS

# The census's `ics-profile` build: the currency's cells are measured on it, so
# the counter has to be proven on it. Its own target directory, so the default
# build every gate is measured on is never overwritten.
PROFILE_BIN = os.environ.get(
    'ICS_PROFILE_BIN',
    f'{ROOT}/target/profile-build/release/examples/overlap_ics_benchmark')
# And the default build, because `published` is not a profiling field and the
# claim is that it is the same counter in a binary with no timers at all.
PLAIN_BIN = os.environ.get(
    'ICS_BIN', f'{ROOT}/target/release/examples/overlap_ics_benchmark')

FIXTURES = ['mixed-61', 'shapes-17', 'triangle-20']
SEED = 0
# Identical to `meter/currency.py`'s cell shape. Not similar: identical, so the
# counter is proven on the cells the coefficient is fitted from.
BITES = int(os.environ.get('ICS_CURRENCY_BITES', '30'))
ATTEMPTS = int(os.environ.get('ICS_CURRENCY_ATTEMPTS', '3'))
ITERS = int(os.environ.get('ICS_CURRENCY_ITERS', '120'))
COMPRESS_BITES = int(os.environ.get('ICS_CURRENCY_COMPRESSBITES', '4'))

RIDER = ('docs/currency-amendment.md, ox-alpha rider (i): "published_bites '
         'proven bit-identical across runs BEFORE fitting P".')


def sha256_of(path):
    try:
        with open(path, 'rb') as handle:
            return hashlib.sha256(handle.read()).hexdigest()
    except OSError:
        return None


def loadavg():
    try:
        with open('/proc/loadavg') as handle:
            return handle.read().split()[:3]
    except OSError:
        return None


def cell(binary, out, fixture, tag):
    """One fixed-work cutclose document, from one process."""
    path = f'{out}/counter-{fixture}-{tag}.json'
    command = [binary, '--cell=cutclose',
               f'--request={lib.REQUESTS[fixture]}',
               f'--edge={lib.EDGE_MM}', f'--pair={lib.PAIR_MM}',
               '--mode=fixed', '--workers=8', f'--seed={SEED}',
               f'--bites={BITES}', f'--attempts={ATTEMPTS}',
               f'--iters={ITERS}', f'--compressbites={COMPRESS_BITES}']
    started = time.monotonic()
    with open(path, 'w') as handle:
        result = subprocess.run(command, stdout=handle, stderr=subprocess.PIPE,
                                check=False)
    wall = time.monotonic() - started
    try:
        with open(path) as handle:
            document = json.load(handle)
    except (OSError, json.JSONDecodeError) as error:
        document = {'_loadError': f'{error}'}
    return {
        'fixture': fixture,
        'process': tag,
        'binary': binary,
        'path': path,
        'sourceSha256': sha256_of(path),
        'command': command,
        'exit': result.returncode,
        'stderr': (result.stderr or b'').decode()[-400:],
        'driverWallSeconds': wall,
    }, document


def counter_of(document):
    """The counter, and everything a reader needs to check it by hand."""
    outcome = document.get('outcome') or {}
    bites = outcome.get('bites') or []
    vector = [bool(row.get('published')) for row in bites]
    return {
        # The vector itself, per bite, so identity is checkable rather than
        # asserted: a scalar that matches can hide two bites swapping.
        'publishedVector': vector,
        'publishedBites': sum(1 for flag in vector if flag),
        'bites': len(vector),
        # The engine's own count, built by appending to the publication list -
        # a different route to the same fact.
        'publicationCount': outcome.get('publicationCount'),
        'depthMm': outcome.get('depthMm'),
    }


def stripped(value):
    """The document with every clock reading removed, and nothing else.

    Two processes of an `ics-profile` build differ in nanoseconds by
    construction - the feature exists to produce them - so a comparison that
    kept them would be red on every green cell. The set of keys that count as a
    clock reading is **the census's**, imported from
    `census/identity.py::TIMING_KEYS` rather than re-listed here: a second copy
    of that list is a second thing to keep in step, and the one field this
    check is about (`published`) is deliberately not in it, nor are the five
    counters beside it.
    """
    if isinstance(value, dict):
        return {key: stripped(inner) for key, inner in value.items()
                if key not in IGNORED_KEYS}
    if isinstance(value, list):
        return [stripped(inner) for inner in value]
    return value


def cross_stripped(value, keys=None):
    """[`stripped`], and also the two keys that name the binary itself.

    A default build and an `ics-profile` build are different executables and
    say so; that is the point of the comparison rather than a casualty of it.
    """
    keys = IGNORED_KEYS | census_identity.BUILD_KEYS if keys is None else keys
    if isinstance(value, dict):
        return {key: cross_stripped(inner, keys) for key, inner in value.items()
                if key not in keys}
    if isinstance(value, list):
        return [cross_stripped(inner, keys) for inner in value]
    return value


def digest(value):
    return hashlib.sha256(
        json.dumps(value, sort_keys=True, separators=(',', ':')).encode()
    ).hexdigest()


def main():
    out = sys.argv[1] if len(sys.argv) > 1 else '/var/lib/t3/tmp/overlapics/gate2/counter'
    os.makedirs(out, exist_ok=True)
    document = {
        'experiment': 'overlap-ics',
        'battery': 'economics-round-published-bites-counter',
        'rider': RIDER,
        'counterSource': (
            'BiteRecord::published, emitted per bite as `published` by '
            'overlap_ics_benchmark in every build. No engine code was edited '
            'to create it: it is the trajectory\'s own publication record, it '
            'costs no clock read, and no engine decision reads it.'),
        'profileBinary': PROFILE_BIN,
        'profileBinarySha256': sha256_of(PROFILE_BIN),
        'plainBinary': PLAIN_BIN,
        'plainBinarySha256': sha256_of(PLAIN_BIN),
        'machine': {'platform': platform.platform(), 'cpus': os.cpu_count(),
                    'loadBefore': loadavg()},
        'cellShape': {'seed': SEED, 'workers': 8, 'mode': 'fixed',
                      'bites': BITES, 'attempts': ATTEMPTS, 'iters': ITERS,
                      'compressBites': COMPRESS_BITES},
    }
    for binary in (PROFILE_BIN, PLAIN_BIN):
        if not os.path.exists(binary):
            document['error'] = f'{binary} is missing'
            document['COUNTER_BIT_IDENTICAL'] = False
            print(json.dumps(document, indent=1))
            return 2

    rows = []
    for fixture in FIXTURES:
        # Two processes on the calibration's own binary, and a third on the
        # binary with no timers in it at all.
        a_meta, a_doc = cell(PROFILE_BIN, out, fixture, 'profile-a')
        b_meta, b_doc = cell(PROFILE_BIN, out, fixture, 'profile-b')
        c_meta, c_doc = cell(PLAIN_BIN, out, fixture, 'plain')
        if any(meta['exit'] != 0 for meta in (a_meta, b_meta, c_meta)):
            document['cells'] = rows + [a_meta, b_meta, c_meta]
            document['error'] = f'{fixture}: a cell did not exit 0'
            document['COUNTER_BIT_IDENTICAL'] = False
            print(json.dumps(document, indent=1))
            return 2
        a, b, c = counter_of(a_doc), counter_of(b_doc), counter_of(c_doc)
        row = {
            'fixture': fixture,
            'processes': [a_meta, b_meta, c_meta],
            'counter': a,
            'twoProcessIdentical': a == b,
            # The default build has no `ics-profile` timers, so its `profile`
            # object is all zeros and its whole document is NOT expected to
            # match. The counter is, and that is the claim being made.
            'crossBuildCounterIdentical': (a['publishedVector']
                                           == c['publishedVector']),
            'crossBuildDepthIdentical': a['depthMm'] == c['depthMm'],
            # Stronger than the counter, and free: the two builds' whole
            # documents agree once the clocks and the binary's own identity
            # are set aside. If this were red the counter's identity would be
            # a coincidence on two different trajectories.
            'crossBuildDocumentIdentical': (
                cross_stripped(a_doc) == cross_stripped(c_doc)),
            'publicationCountReconciles': (
                a['publishedBites'] == a['publicationCount']),
            'wholeDocumentIdentical': stripped(a_doc) == stripped(b_doc),
            'documentDigest': digest(stripped(a_doc)),
            'counterDigest': digest(a['publishedVector']),
            'walls': {'profileA': a_meta['driverWallSeconds'],
                      'profileB': b_meta['driverWallSeconds'],
                      'plain': c_meta['driverWallSeconds']},
        }
        rows.append(row)
        print(f'[counter] {fixture} publishedBites={a["publishedBites"]}/'
              f'{a["bites"]} twoProcess={row["twoProcessIdentical"]} '
              f'crossBuild={row["crossBuildCounterIdentical"]} '
              f'reconciles={row["publicationCountReconciles"]}',
              file=sys.stderr)
    document['fixtures'] = rows
    document['machine']['loadAfter'] = loadavg()

    # The design vector the coefficient will be fitted from, printed here so it
    # can be compared with the one the meter reports later.
    document['designVector'] = {row['fixture']: row['counter']['publishedBites']
                                for row in rows}
    verdict = all(row['twoProcessIdentical']
                  and row['wholeDocumentIdentical']
                  and row['crossBuildCounterIdentical']
                  and row['crossBuildDepthIdentical']
                  and row['crossBuildDocumentIdentical']
                  and row['publicationCountReconciles']
                  for row in rows)
    document['COUNTER_BIT_IDENTICAL'] = verdict
    document['consequence'] = (
        'rider (i) is satisfied: P may be fitted' if verdict else
        'rider (i) is NOT satisfied: no coefficient may be fitted, and the '
        'wave stops here rather than at the reject rule')
    print(json.dumps(document, indent=1))
    env_out = os.environ.get('ICS_OUT')
    if env_out:
        os.makedirs(env_out, exist_ok=True)
        with open(f'{env_out}/counter.json', 'w') as handle:
            json.dump(document, handle, indent=1)
    return 0 if verdict else 1


if __name__ == '__main__':
    sys.exit(main())
