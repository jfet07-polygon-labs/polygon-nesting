#!/usr/bin/env python3
"""§1: the mispricing table, and the count vectors the profile is fitted on.

Two sources, kept separate because they answer two different questions and one
of them is free:

* **the corpus** - every `portfolio.operatorCalls` row this repository has
  already recorded, across `basin-race/`, `replan/` and `calibrated-plan/`.
  That is hundreds of calls of five operator classes at no cost, and it is what
  makes the rate table a pooled measurement rather than one afternoon's.
  It carries wall and shipped-meter units per call and nothing else.
* **the observing arm** - runs of this round's binary at `cur2=2`, which
  prices every call and charges none of it. That is the only source of the
  per-class *count vectors*, because no previous round recorded them.

Usage:
    python3 rates.py OUT_JSON BINARY [corpus-root ...]
"""
import collections
import glob
import json
import os
import statistics
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import runlib  # noqa: E402

# The observing cells. Three fixtures at a pinned work budget for the classes
# the queue buys anyway, plus a race arm on each fixture, which is the only
# configuration that dispatches mode 20 often enough to price it.
#
# `race=3:1:3` is `docs/experiments/basin-race/` §4.2's own specified arm. The
# race stays off in every shipping recommendation; it is used here as a mode-20
# *generator*, because it is the one phase that draws constructors on demand.
CELLS = [
    ('mixed-61', 0, 'cur2=2'),
    ('mixed-61', 1, 'cur2=2'),
    ('mixed-61', 0, 'cur2=2,race=3:1:3'),
    ('shapes-17', 0, 'cur2=2,race=3:1:3'),
    ('triangle-20', 0, 'cur2=2,race=3:1:3'),
]

CORPUS_DEFAULT = ['basin-race', 'replan', 'calibrated-plan']

# The currency's count vector, in the order the profile's weights are written.
# The first five are the profiling array's; the next four are the operator's
# own account, and §1.2 is why the second group had to exist at all.
COUNT_KEYS = [
    'candidateQueries', 'exactPairTests', 'collisionBuilds', 'neighborTests',
    'fullRescores', 'positionSourceAttempts', 'returnedPositions',
    'pairVisits', 'operatorCollisionBuilds', 'confirmations',
]


def harvest_corpus(roots):
    """Every recorded operator call in the named evidence directories."""
    calls = []

    def walk(node, source):
        if isinstance(node, dict):
            rows = node.get('operatorCalls')
            if (isinstance(rows, list) and rows
                    and isinstance(rows[0], dict) and 'operator' in rows[0]):
                for row in rows:
                    calls.append((source, row))
            for value in node.values():
                walk(value, source)
        elif isinstance(node, list):
            for value in node:
                walk(value, source)

    for name in roots:
        pattern = f'{runlib.ROOT}/docs/experiments/{name}/evidence/*.json'
        for path in sorted(glob.glob(pattern)):
            try:
                walk(json.load(open(path)), path)
            except (json.JSONDecodeError, OSError):
                continue
    return calls


def rate_rows(calls):
    """Pooled and per-call rates, by operator."""
    by = collections.defaultdict(list)
    for _, call in calls:
        operator = call.get('operator')
        units, seconds = call.get('workUnits'), call.get('elapsedSeconds')
        if operator is None or units is None or seconds is None:
            continue
        by[operator].append((units, seconds))
    rows = []
    for operator in sorted(by, key=lambda o: -len(by[o])):
        entries = by[operator]
        units = sum(u for u, _ in entries)
        seconds = sum(s for _, s in entries)
        # Per-call rates are only meaningful where the call is long enough for
        # the process clock to mean anything; 1 ms is the floor.
        percall = [u / s for u, s in entries if s > 1e-3]
        rows.append({
            'operator': operator,
            'calls': len(entries),
            'totalUnits': units,
            'totalSeconds': seconds,
            'pooledUnitsPerSecond': (units / seconds) if seconds else None,
            'medianUnitsPerSecond': (statistics.median(percall)
                                     if percall else None),
            'minUnitsPerSecond': min(percall) if percall else None,
            'maxUnitsPerSecond': max(percall) if percall else None,
            'medianSecondsPerCall': statistics.median(
                [s for _, s in entries]) if entries else None,
        })
    return rows


def observe(binary, outdir):
    """The `cur2=2` runs, and every count vector they produced."""
    observed = []
    for request, seed, extra in CELLS:
        spec = runlib.spec_for(seed, 'work', runlib.WORK_10S, True, extra)
        tag = f'{request}-s{seed}-{"race" if "race" in extra else "queue"}'
        doc, wall, err = runlib.run(binary, request, seed, spec,
                                    f'{outdir}/observe-{tag}.json')
        portfolio = doc.get('portfolio') or {}
        rows = []
        for call in portfolio.get('operatorCalls', []):
            currency = call.get('workCurrency')
            if currency is None:
                continue
            rows.append({
                'operator': call['operator'],
                'action': call.get('action'),
                'phase': call.get('phase'),
                'elapsedSeconds': call['elapsedSeconds'],
                'globalUnits': call['globalUnits'],
                'selfMeteredUnits': call.get('selfMeteredUnits'),
                'counts': {key: currency[key] for key in COUNT_KEYS},
                'classUnits': currency['classUnits'],
            })
        observed.append({
            'request': request, 'seed': seed, 'spec': spec, 'tag': tag,
            'processSeconds': wall,
            'workCurrency': portfolio.get('workCurrency'),
            'rawDepthMm': (portfolio.get('incumbent') or {}).get('rawDepthMm'),
            'stderr': err[-400:] if err else None,
            'calls': rows,
        })
    return observed


def main():
    out = sys.argv[1]
    binary = sys.argv[2]
    roots = sys.argv[3:] or CORPUS_DEFAULT
    corpus = harvest_corpus(roots)
    document = {
        'binary': binary,
        'binarySha256': runlib.sha256_of(binary),
        'corpusRoots': roots,
        'corpusCalls': len(corpus),
        'corpusRates': rate_rows(corpus),
        'observed': observe(binary, os.path.dirname(out)),
        'boxLoad': runlib.LOAD,
    }
    # The same rate table, recomputed over the observing runs alone, so a
    # reader can see whether this session agrees with the corpus.
    session = [(row['tag'], call)
               for row in document['observed'] for call in row['calls']]
    document['sessionRates'] = rate_rows(
        [(tag, {'operator': call['operator'],
                'workUnits': call['globalUnits'],
                'elapsedSeconds': call['elapsedSeconds']})
         for tag, call in session])
    os.makedirs(os.path.dirname(out), exist_ok=True)
    with open(out, 'w') as handle:
        json.dump(document, handle, indent=1, sort_keys=True)
    print(f'wrote {out}: {len(corpus)} corpus calls, '
          f'{len(session)} observed calls')


if __name__ == '__main__':
    main()
