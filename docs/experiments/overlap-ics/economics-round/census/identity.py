#!/usr/bin/env python3
"""**The trajectory did not move.** Four fixed-work cells, three binaries.

    python3 identity.py <base-binary> [work-dir]

Wave 1 adds instrumentation to `mod.rs`, and the one thing instrumentation is
never allowed to do is change the answer. This is the red/green for that, and
it is a *measurement across binaries*, not an argument:

  D1  the pre-Wave-1 binary against this one, **left-subset**: every field the
      old document carried is present in the new one with a bit-identical
      value. New fields are allowed - `poses`, `placements`,
      `exactCheckpointCalls`, `profile` - and nothing else may differ.
  D2  two processes of the new default build, bit-identical after stripping
      `wall`. The shipped determinism claim, re-run on the new binary.
  D3  the new default build against a `--features ics-profile` build of the
      same tree, ignoring only the nanosecond fields the feature exists to
      produce. **This is what makes the feature gate a fact rather than a
      promise**: the profiling build takes the same trajectory, so its
      measurement is a measurement of the shipped one.
  D4  two processes of the `ics-profile` build, same rule.

The four cells are fixed-work by construction, so no clock is read inside any
trajectory and none of these comparisons is load-dependent:

    A  8 explore bites,  8 workers, seed 0   - the FAST K=8 shape
    B  21 explore bites, 8 workers, seed 0   - the 179 shelf's parent
    C  21 explore bites, 8 workers, seed 5   - the strike-starved watch seed
    D  8 explore bites,  1 worker,  seed 0   - the no-thread path

`<base-binary>` is a copy of the example built at the round's base commit. It
has to be passed in rather than rebuilt here: a script that builds its own
"before" can only ever compare a tree to itself.
"""
import json
import os
import subprocess
import sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)),
                                    '..', '..', '..', '..', '..'))
REQUEST = (f'{ROOT}/tests/fixtures/mixed-61/'
           'mixed61-request-exact-clearance.json')
DEFAULT_BIN = f'{ROOT}/target/release/examples/overlap_ics_benchmark'
PROFILE_BIN = f'{ROOT}/target/profile-build/release/examples/overlap_ics_benchmark'

# One process each, twice, per binary.
CELLS = {
    'A': ['--mode=fixed', '--bites=8', '--attempts=1', '--iters=400',
          '--compressbites=0', '--workers=8', '--seed=0'],
    'B': ['--mode=fixed', '--bites=21', '--attempts=1', '--iters=400',
          '--compressbites=0', '--workers=8', '--seed=0'],
    'C': ['--mode=fixed', '--bites=21', '--attempts=1', '--iters=400',
          '--compressbites=0', '--workers=8', '--seed=5'],
    'D': ['--mode=fixed', '--bites=8', '--attempts=1', '--iters=400',
          '--compressbites=0', '--workers=1', '--seed=0'],
}

# Keys whose *value* is allowed to differ between two builds of the same tree.
# `wall` is the clock object every determinism comparison in this campaign has
# always stripped; `executableSha256` and `buildFeatures` identify the binary
# and are the point of the comparison rather than a casualty of it; the rest
# are the nanosecond fields the `ics-profile` feature exists to produce, and
# the counters beside them (`iterations`, `bandEntries`, `exactCalls`,
# `sampleEvaluations`, `repairRows`, `disruptionMoves`) are deliberately NOT in
# this set - they are counters, they are populated in both builds, and if they
# ever disagreed the two builds would be taking different trajectories.
TIMING_KEYS = {
    'measured', 'barrierToBarrierNs', 'barrierToBarrierNsPerIteration',
    'prepNs', 'dispatchNs', 'sweepCriticalNs', 'sweepTotalNs', 'mergeGlsNs',
    'exactNs', 'bandFoldNs', 'snapshotNs', 'residualNs', 'prepPlusDispatchNs',
    'prepPlusDispatchShare', 'ns', 'share', 'sampleEvaluationsPerSecond',
}
BUILD_KEYS = {'wall', 'executableSha256', 'buildFeatures'}


def run(binary, cell, out_path):
    os.makedirs(os.path.dirname(out_path), exist_ok=True)
    command = ([binary, '--cell=cutclose', f'--request={REQUEST}',
                '--edge=5', '--pair=5'] + CELLS[cell])
    with open(out_path, 'w') as handle:
        result = subprocess.run(command, stdout=handle, stderr=subprocess.PIPE,
                                check=False)
    status = result.returncode
    stderr = (result.stderr or b'').decode()[-400:]
    try:
        with open(out_path) as handle:
            document = json.load(handle)
    except (OSError, json.JSONDecodeError) as error:
        return None, status, f'{error}'
    return document, status, stderr


def differences(left, right, ignore, subset, path='$', found=None):
    """Every place `right` fails to reproduce `left`.

    `subset` is what separates D1 from D3: under it, a key present in `right`
    but not in `left` is an *addition* and is fine, which is the only way a
    round that adds evidence fields can prove it changed nothing else. Without
    it the two documents must carry exactly the same keys.
    """
    found = [] if found is None else found
    if len(found) >= 40:
        return found
    if isinstance(left, dict) and isinstance(right, dict):
        for key in left:
            if key in ignore:
                continue
            if key not in right:
                found.append(f'{path}.{key}: missing on the right')
                continue
            differences(left[key], right[key], ignore, subset,
                        f'{path}.{key}', found)
        if not subset:
            for key in right:
                if key not in ignore and key not in left:
                    found.append(f'{path}.{key}: present only on the right')
        return found
    if isinstance(left, list) and isinstance(right, list):
        if len(left) != len(right):
            found.append(f'{path}: length {len(left)} vs {len(right)}')
            return found
        for index, (one, two) in enumerate(zip(left, right)):
            differences(one, two, ignore, subset, f'{path}[{index}]', found)
        return found
    # Floats are compared on their bits, not with a tolerance: a trajectory
    # that reproduces "closely" has not reproduced.
    if isinstance(left, float) or isinstance(right, float):
        same = (isinstance(left, (int, float)) and isinstance(right, (int, float))
                and repr(float(left)) == repr(float(right)))
    else:
        same = left == right
    if not same:
        found.append(f'{path}: {left!r} vs {right!r}')
    return found


def vector(name, question, left, right, ignore, subset):
    rows = differences(left, right, ignore, subset)
    return {
        'vector': name,
        'question': question,
        'subsetComparison': subset,
        'ignoredKeys': sorted(ignore),
        'differences': len(rows),
        'detail': rows[:12],
        'pass': not rows,
    }


def main():
    if len(sys.argv) < 2:
        raise SystemExit('usage: identity.py <base-binary> [work-dir]')
    base_bin = sys.argv[1]
    out = sys.argv[2] if len(sys.argv) > 2 else '/var/lib/t3/tmp/census-wave1/identity'
    vectors = []
    exits = []
    for cell in sorted(CELLS):
        docs = {}
        for tag, binary in (('base', base_bin), ('head', DEFAULT_BIN),
                            ('profile', PROFILE_BIN)):
            for process in ('a', 'b'):
                document, status, stderr = run(
                    binary, cell, f'{out}/{tag}-{cell}-{process}.json')
                exits.append({'cell': cell, 'binary': tag, 'process': process,
                              'exit': status, 'stderr': stderr})
                docs[(tag, process)] = document
        vectors.append(vector(
            f'D1-{cell}',
            'every field the pre-Wave-1 binary emitted, reproduced bit for bit',
            docs[('base', 'a')], docs[('head', 'a')],
            BUILD_KEYS, subset=True))
        vectors.append(vector(
            f'D2-{cell}',
            'two processes of the default build agree bit for bit',
            docs[('head', 'a')], docs[('head', 'b')],
            {'wall'}, subset=False))
        vectors.append(vector(
            f'D3-{cell}',
            'the ics-profile build takes the default build\'s trajectory',
            docs[('head', 'a')], docs[('profile', 'a')],
            BUILD_KEYS | TIMING_KEYS, subset=False))
        vectors.append(vector(
            f'D4-{cell}',
            'two processes of the ics-profile build agree on everything but time',
            docs[('profile', 'a')], docs[('profile', 'b')],
            BUILD_KEYS | TIMING_KEYS, subset=False))

    document = {
        'experiment': 'overlap-ics',
        'battery': 'economics-round-census-trajectory-identity',
        'baseBinary': base_bin,
        'headBinary': DEFAULT_BIN,
        'profileBinary': PROFILE_BIN,
        'cells': CELLS,
        'processExits': exits,
        'vectors': vectors,
        'allExitsZero': all(row['exit'] == 0 for row in exits),
        'IDENTITY_PASS': (all(row['pass'] for row in vectors)
                          and all(row['exit'] == 0 for row in exits)),
    }
    print(json.dumps(document, indent=1))
    os.makedirs(out, exist_ok=True)
    with open(f'{out}/identity.json', 'w') as handle:
        json.dump(document, handle, indent=1)
    return 0 if document['IDENTITY_PASS'] else 1


if __name__ == '__main__':
    sys.exit(main())
