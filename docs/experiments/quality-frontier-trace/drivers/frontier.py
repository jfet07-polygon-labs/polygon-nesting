#!/usr/bin/env python3
"""The quality frontier trace's measurement runs.

One process per configuration, FROM REQUEST ONLY - no pinned parent, no warm
start, the production default search-offset allowance. Two variants of each:

  * `work`  - `POLYGON_NESTING_QUALITY_TRACE_COUNTERS` left on, so every event
              carries live work ordinals. Its clock is stretched by the
              counting sites (measured separately by ab.py).
  * `clock` - counters off, so the timeline is the one the unprofiled build
              runs on and the ordinals are zero.

Neither run alone is honest about both axes, which is why both exist and why
the summary quotes the stretch between them rather than one number.

The mode-20 construction clamp is *derived*, not pinned: it is
`MODE20_CLAMP_MULTIPLE` times the request's own area lower-bound depth, which
scales with the sheet and the piece set instead of carrying a fixture's
constant. The multiple is reported with every run.

Usage: frontier.py [work|clock|both]
"""
import json
import os
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib  # noqa: E402

OUT = '/var/lib/t3/tmp/qft/frontier'
SEEDS = (0, 1)
# The clamp mode 20 constructs under, as a multiple of the request's own area
# lower-bound depth. Construction itself inserts at full-sheet settings; this
# bound is what the publication audit runs under, so it has to be loose enough
# to admit a first complete layout and tight enough to be a bound at all. Two
# lower bounds is the scale-free reading of the 320 mm clamp the pinned
# Mixed-61 anchor used (that request's lower bound is 158.7 mm).
MODE20_CLAMP_MULTIPLE = 2.0


def base_args(seed):
    return [a.format(clamp='0', seed=seed) for a in lib.ARGS]


def invoke(tag, seed, mode, target, counters, out_dir):
    argv = [lib.TRACE_BIN, lib.REQ] + base_args(seed) + [str(mode)]
    if target is not None:
        argv += ['', str(target)]
    env = dict(os.environ)
    env.pop('POLYGON_NESTING_PROFILE', None)
    env['POLYGON_NESTING_QUALITY_TRACE'] = f'{out_dir}/{tag}.jsonl'
    env['POLYGON_NESTING_QUALITY_TRACE_COUNTERS'] = '1' if counters else '0'
    # Mode 20's entry point refuses an unpinned parent, so a from-request
    # mode-20 run is only reachable behind the opt-in flag. Mode 0 does not
    # need it and does not get it.
    if mode:
        env['POLYGON_NESTING_UNPINNED_VACANCY_PARENT'] = '1'
    path = f'{out_dir}/{tag}.json'
    started = time.monotonic()
    with open(path, 'w') as handle:
        result = subprocess.run(argv, stdout=handle, stderr=subprocess.PIPE,
                                check=False, env=env)
    seconds = time.monotonic() - started
    try:
        doc = json.load(open(path))
    except json.JSONDecodeError:
        doc = {'_loadError': (result.stderr or b'').decode()[-600:]}
    return doc, seconds, argv


def area_lower_bound(out_dir):
    """One short probe purely to read the request's own scale."""
    doc, _, _ = invoke('probe-lower-bound', 0, 0, None, False, out_dir)
    return doc['areaLowerBoundDepthMm']


def main():
    which = sys.argv[1] if len(sys.argv) > 1 else 'both'
    os.makedirs(OUT, exist_ok=True)
    lower_bound = area_lower_bound(OUT)
    clamp = round(MODE20_CLAMP_MULTIPLE * lower_bound, 3)
    manifest = {
        'binary': lib.TRACE_BIN,
        'request': lib.REQ,
        'areaLowerBoundDepthMm': lower_bound,
        'mode20ClampMultiple': MODE20_CLAMP_MULTIPLE,
        'mode20ClampMm': clamp,
        'runs': [],
    }
    variants = {'work': True, 'clock': False}
    if which != 'both':
        variants = {which: variants[which]}
    for variant, counters in variants.items():
        for seed in SEEDS:
            for label, mode, target in (
                    ('m0coupled', 0, None),
                    ('mode20', 20, clamp)):
                tag = f'{label}-seed{seed}-{variant}'
                doc, seconds, argv = invoke(tag, seed, mode, target, counters,
                                            OUT)
                pop = lib.population(doc) or {}
                row = {
                    'tag': tag, 'config': label, 'seed': seed,
                    'variant': variant, 'workOrdinalsArmed': counters,
                    'wallSeconds': seconds,
                    'argv': argv,
                    'engineDepthMm': doc.get(
                        'independentUsedLongAxisDepthMm'),
                    'engineFingerprint': doc.get('finalPlacementFingerprint'),
                    'modeExactValid': pop.get('exactValid'),
                    'modeDepthMm': pop.get('independentDepthMm'),
                    'modeRawDepthMm': pop.get('rawSourceDepthMm'),
                    'modeFailureReason': pop.get('failureReason'),
                    'error': doc.get('_loadError'),
                }
                manifest['runs'].append(row)
                print(json.dumps(row), flush=True)
    json.dump(manifest, open(f'{OUT}/manifest.json', 'w'), indent=1)


if __name__ == '__main__':
    main()
