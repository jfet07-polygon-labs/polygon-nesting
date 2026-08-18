#!/usr/bin/env python3
"""PR7 measurement: the coordinator's depth-versus-time curve against the
m0+coupled baseline, from the bare request, at a ten-second wall budget.

Both arms run FROM REQUEST ONLY - no pinned parent, no warm start, the
production default search-offset allowance (0.002 mm, passed explicitly because
the coordinator spec is a later positional argument than the allowance slot;
the value is the default, so the arm is the documented baseline).

Both arms are quality-trace armed with `POLYGON_NESTING_QUALITY_TRACE_COUNTERS=0`
so the clock is the one the production build runs on and no work ordinal
distorts the curve.

Rounds are INTERLEAVED and the arm order alternates every round, because
another agent benchmarks on this box concurrently and an un-paired timing claim
here would be worthless.

Usage: curve.py ROUNDS BINARY [SEED ...]
"""
import json
import os
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib  # noqa: E402

OUT = '/var/lib/t3/tmp/pr7/curve'
DEFAULT_ALLOWANCE = '0.002'
# The three salt sets. Each is a list of void-grid cell divisors the basin
# phase cycles over its slots. They are *tickets*, not tunings: the ledger's
# eighteen-size sweep found no region of good cell sizes, so a coordinator's
# job is to draw several and keep the distinct ones.
SALT_SETS = {
    0: '13:15:17:19',
    1: '11:15:21:27',
    2: '15:23:31:39',
}
# Three arms, all from the bare request at the same ten-second wall budget:
#
#   base    the protected m0 + coupled search, no coordinator at all
#   coord   the coordinator running the review's own schedule - four salted
#           constructor basin slots, then alternation quanta round-robined over
#           the three best structurally distinct archive states
#   focus   the same coordinator with the basin slice priced at zero
#           (`slots=0`) and the alternation phase chaining on the single best
#           archive state, which is what the measured operator economics say
#           the budget is worth
#
# `focus` exists to price `coord`'s constructor slice against its own
# opportunity cost rather than against nothing, which is the only way to say
# whether the review's 1.9-4.0 s allocation earns its place at this budget.
ARMS = {
    'base': None,
    'coord': 'wall=10000,slots=4,states=3,cycles=1,epochs=4,cells={cells}',
    'focus': 'wall=10000,slots=0,states=1,cycles=1,epochs=4',
}


def invoke(tag, seed, spec, out_dir, trace_dir):
    argv = ([BINARY, lib.REQ]
            + [a.format(clamp='0', seed=seed) for a in lib.ARGS]
            + ['0', '', '', '', DEFAULT_ALLOWANCE])
    if spec:
        argv += [spec]
    env = dict(os.environ)
    env.pop('POLYGON_NESTING_PROFILE', None)
    env['POLYGON_NESTING_QUALITY_TRACE'] = f'{trace_dir}/{tag}.jsonl'
    env['POLYGON_NESTING_QUALITY_TRACE_COUNTERS'] = '0'
    os.makedirs(out_dir, exist_ok=True)
    os.makedirs(trace_dir, exist_ok=True)
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
    return doc, seconds


def incumbent_series(trace_path):
    """The depth-versus-time curve, joined to raw source depth.

    `incumbent` events carry the grid-snapped depth; `exactCandidate` events
    carry the raw `f64` reading for the same fingerprint. The curve is quoted in
    raw depth wherever the join succeeds, which is the reading a threshold may
    be compared against.
    """
    series = []
    raw_by_fingerprint = {}
    scope = []
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
            if kind == 'scopeEnter':
                scope.append(event.get('operator'))
            elif kind == 'scopeExit' and scope:
                scope.pop()
            elif kind == 'exactCandidate':
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


def summarize(tag, doc, seconds, trace_dir):
    row = {
        'tag': tag,
        'processSeconds': seconds,
        'engineDepthMm': doc.get('independentUsedLongAxisDepthMm'),
        'incumbentSeries': incumbent_series(f'{trace_dir}/{tag}.jsonl'),
    }
    portfolio = doc.get('portfolio')
    if portfolio:
        row['rawDepthMm'] = portfolio['incumbent']['rawDepthMm']
        row['dualGateValid'] = portfolio['incumbent']['dualGateValid']
        row['publishedSeconds'] = portfolio['incumbent']['publishedSeconds']
        row['coordinatorSeconds'] = portfolio['elapsedSeconds']
        row['phases'] = portfolio['phases']
        row['publications'] = portfolio['publications']
        row['operatorCalls'] = portfolio['operatorCalls']
        row['archive'] = portfolio['archive']
    else:
        row['rawDepthMm'] = doc.get('rawSourceDepthMm')
    if '_loadError' in doc:
        row['loadError'] = doc['_loadError']
    return row


def main():
    global BINARY
    rounds = int(sys.argv[1])
    BINARY = sys.argv[2]
    seeds = [int(value) for value in sys.argv[3:]] or [0, 1, 2]
    out_dir = f'{OUT}/runs'
    trace_dir = f'{OUT}/traces'
    result = {'binary': BINARY, 'rounds': rounds, 'seeds': seeds,
              'allowance': DEFAULT_ALLOWANCE, 'arms': ARMS,
              'saltSets': SALT_SETS, 'rows': []}
    for round_index in range(rounds):
        for seed in seeds:
            cells = SALT_SETS[seed % len(SALT_SETS)]
            arms = [(label, spec.format(cells=cells) if spec else None)
                    for label, spec in ARMS.items()]
            # The arm order rotates every round, so no arm is systematically
            # first or last while another agent benchmarks on the same box.
            arms = arms[round_index % len(arms):] + arms[:round_index % len(arms)]
            for label, arm_spec in arms:
                tag = f'{label}-s{seed}-r{round_index}'
                doc, seconds = invoke(tag, seed, arm_spec, out_dir, trace_dir)
                row = summarize(tag, doc, seconds, trace_dir)
                row.update({'arm': label, 'seed': seed, 'round': round_index})
                result['rows'].append(row)
                print(f"{tag}: engine={row['engineDepthMm']} "
                      f"raw={row.get('rawDepthMm')} "
                      f"process={seconds:.2f}s", flush=True)
    os.makedirs(OUT, exist_ok=True)
    json.dump(result, open(f'{OUT}/curve.json', 'w'), indent=1)


if __name__ == '__main__':
    main()
