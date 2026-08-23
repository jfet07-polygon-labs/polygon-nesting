#!/usr/bin/env python3
"""**The frozen wall.** Nine seeds, three budgets, one binary, one verdict.

    python3 wall.py                    # the whole battery
    python3 wall.py 10                 # one budget cell

Reads `docs/experiments/overlap-ics/cutclose-round1/README.md` §0 and nothing
else. Every number this prints is a measurement; the verdict is §0's arithmetic
applied to them and this file contains no threshold of its own.

**It refuses to start without the first-bite canary.** Grok review 12 Round 2
§6.3.4 is a stop, not a report - "FAIL here is a member fail; do not run the
9-seed wall" - so `main` loads `cutclose-fast.json`, checks `CANARY_PASS`, and
exits 2 without spending a wall second if it is false or missing.

What it runs, in order:

  1. the nine 10.000 s cells - **the gate**;
  2. the nine 3.000 s and nine 30.000 s cells - the non-interpolated curve,
     reported in full, unable to pass or fail anything;
  3. the fixed-work replay of every wall publication ordinal, in two
     processes - "wall publications record their fixed-work ordinal" (Grok
     review 12 Round 2 §6.8) is only worth recording if something checks it.

The interleaved AB/BA wall-arm control is `control.py`; it is a separate
process family and a separate document, because the old stack is never a lane.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib  # noqa: E402

# §0.1. The only threshold in this round, and it is quoted rather than chosen.
BAR_MM = 168.484
SEEDS = list(range(9))
BUDGETS = ['3', '10', '30']
GATE_BUDGET = '10'
QUORUM = 3
WORKERS = 8
# The wall targets, in seconds, exactly as §0.4 clause 5 names them.
SECONDS = {'3': 3.000, '10': 10.000, '30': 30.000}


def cell(out, seed, budget):
    """One process: one seed, one budget, the bare mixed-61 request."""
    tag = f'wall-{budget}s-seed{seed}'
    doc, wall, status, err = lib.run(
        'cutclose', 'mixed-61', f'{out}/{tag}.json', mode='wall',
        wall=SECONDS[budget], workers=WORKERS, seed=seed)
    if status != 0:
        return {'seed': seed, 'budgetSeconds': SECONDS[budget], 'exit': status,
                'stderr': err[-800:], 'qualifies': False, 'valid': False}
    outcome = doc.get('outcome', {})
    constructor = doc.get('constructor', {})
    publications = outcome.get('publications', [])
    # **The checkpoint filter, not an interpolation.** A publication whose own
    # `wallSeconds` exceeds the budget does not count for it (§0.1: "a
    # publication completed after 10.000 s cannot change that verdict"). In
    # practice the loop cannot publish after its own deadline, but the filter is
    # written out so the claim is checked rather than assumed.
    limit = SECONDS[budget]
    within = [row for row in publications
              if row.get('wallSeconds') is None or row['wallSeconds'] <= limit]
    strict = [row for row in within
              if row['placementFingerprint']
              != constructor.get('placementFingerprint')]
    best = min((row['publishedRawDepthMm'] for row in strict), default=None)
    incumbent = outcome.get('incumbent', {})
    return {
        'seed': seed,
        'budgetSeconds': limit,
        'exit': status,
        'valid': True,
        'constructorDepthMm': constructor.get('rawSourceDepthMm'),
        'constructorSeconds': doc.get('wall', {}).get('constructorSeconds'),
        'searchSeconds': doc.get('wall', {}).get('searchSeconds'),
        'totalSeconds': doc.get('wall', {}).get('totalSeconds'),
        'processWallSeconds': wall,
        # The anytime answer: the best STRICT non-constructor dual-valid child
        # published at or before the budget. `None` means the constructor floor
        # is still the incumbent, which §0 allows and expects at 3 s.
        'bestStrictChildMm': best,
        'incumbentMm': incumbent.get('rawSourceDepthMm'),
        'incumbentIsConstructor': incumbent.get('fromConstructor'),
        'publicationsWithinBudget': len(within),
        'publicationsTotal': len(publications),
        'strictChildren': len(strict),
        'exploreBites': outcome.get('exploreBites'),
        'compressBites': outcome.get('compressBites'),
        'funnel': outcome.get('funnel'),
        'invalidPublications': outcome.get('invalidPublications'),
        'repairMaxUm': (outcome.get('repairMaxDisplacementMm') or 0.0) * 1000.0,
        'repairMaxGivebackMm': outcome.get('repairMaxGivebackMm'),
        'relocateEconomics': outcome.get('relocateEconomics'),
        'lastPublicationOrdinal': (publications[-1]['ordinal']
                                   if publications else None),
        'finalWidthMm': outcome.get('finalWidthMm'),
        'minRawPhiOfLastBite': (outcome.get('bites') or [{}])[-1].get('minRawPhi'),
        # §0.1's pass predicate, per seed.
        'qualifies': bool(best is not None and best <= BAR_MM),
    }


def replay(out, rows):
    """The fixed-work replay of every wall publication ordinal, two processes.

    A wall run's publications carry `(bite, attempt, iteration, proposals)`.
    The replay re-runs the same seed with the wall removed and the same bite
    count as a quota, twice, and asserts the two processes agree bit for bit -
    which is the claim `wall publications record their fixed-work ordinal` is
    only useful if someone checks. It is **not** a claim that a wall run and a
    fixed-work run take the same trajectory: they do not, and cannot, because a
    wall run's separations end on a clock.
    """
    results = []
    for row in rows:
        if not row.get('valid') or not row.get('lastPublicationOrdinal'):
            results.append({'seed': row['seed'], 'skipped': True,
                            'reason': 'no publication to replay'})
            continue
        bites = row['lastPublicationOrdinal']['bite']
        seed = row['seed']
        options = dict(mode='fixed', bites=bites, attempts=1, iters=400,
                       compressbites=0, workers=WORKERS, seed=seed)
        first, _, status_a, err_a = lib.run(
            'cutclose', 'mixed-61', f'{out}/replay-seed{seed}-a.json', **options)
        second, _, status_b, err_b = lib.run(
            'cutclose', 'mixed-61', f'{out}/replay-seed{seed}-b.json', **options)
        same = (status_a == 0 and status_b == 0
                and lib.stripped(first) == lib.stripped(second))
        outcome = first.get('outcome', {})
        results.append({
            'seed': seed,
            'skipped': False,
            'replayedBites': bites,
            'exitA': status_a,
            'exitB': status_b,
            'stderrA': err_a[-300:] if err_a else '',
            'stderrB': err_b[-300:] if err_b else '',
            'digestA': lib.digest(first),
            'digestB': lib.digest(second),
            'bitIdentical': same,
            'replayPublications': outcome.get('publicationCount'),
            'replayDepthMm': outcome.get('depthMm'),
            'replayOrdinals': [p['ordinal'] for p in
                               outcome.get('publications', [])],
            'invalidPublications': outcome.get('invalidPublications'),
        })
    return results


def canary_licence(out):
    path = f'{out}/cutclose-fast.json'
    try:
        with open(path) as handle:
            document = json.load(handle)
    except (OSError, json.JSONDecodeError) as error:
        return False, f'{path}: {error}'
    if not document.get('CANARY_PASS'):
        return False, 'CANARY_PASS is false in ' + path
    return True, path


def main():
    out = os.environ.get('ICS_OUT', lib.OUT)
    os.makedirs(out, exist_ok=True)
    licensed, where = canary_licence(out)
    if not licensed:
        print(json.dumps({
            'experiment': 'overlap-ics',
            'battery': 'cutclose-round1-wall',
            'REFUSED': True,
            'reason': ('the first-bite canary has not passed; Grok review 12 '
                       'Round 2 §6.3.4 forbids running the 9-seed wall'),
            'detail': where,
        }, indent=1))
        return 2

    budgets = sys.argv[1:] or BUDGETS
    cells = {}
    for budget in budgets:
        if budget not in SECONDS:
            raise SystemExit(f'unknown budget {budget}')
        rows = [cell(out, seed, budget) for seed in SEEDS]
        qualifying = [row['seed'] for row in rows if row.get('qualifies')]
        invalid = sum(row.get('invalidPublications') or 0 for row in rows)
        cells[budget] = {
            'budgetSeconds': SECONDS[budget],
            'seeds': rows,
            'qualifyingSeeds': qualifying,
            'qualifyingCount': len(qualifying),
            'invalidPublicationsAcrossAllSeeds': invalid,
            'allSeedsValid': all(row.get('valid') for row in rows),
            'isTheGate': budget == GATE_BUDGET,
        }

    gate = cells.get(GATE_BUDGET)
    verdict = None
    if gate is not None:
        # §0.1, applied. Two clauses, both necessary: the quorum, and "a single
        # invalid publication is a FAIL even if some other seed is under
        # 168.484" - which is scored across EVERY cell, not only the gate one.
        invalid_everywhere = sum(
            row.get('invalidPublications') or 0
            for c in cells.values() for row in c['seeds'])
        verdict = {
            'barMm': BAR_MM,
            'quorumRequired': QUORUM,
            'quorumReached': gate['qualifyingCount'],
            'qualifyingSeeds': gate['qualifyingSeeds'],
            'everyPublicationDualValid': invalid_everywhere == 0,
            'invalidPublicationsAcrossEveryCell': invalid_everywhere,
            'allNineSeedsValid': gate['allSeedsValid'],
            'GATE_PASS': bool(gate['qualifyingCount'] >= QUORUM
                              and invalid_everywhere == 0
                              and gate['allSeedsValid']),
        }

    replays = replay(out, gate['seeds']) if gate is not None else []
    document = {
        'experiment': 'overlap-ics',
        'battery': 'cutclose-round1-wall',
        'binary': lib.BIN,
        'canaryLicence': where,
        'workers': WORKERS,
        'seeds': SEEDS,
        'request': lib.REQUESTS['mixed-61'],
        'cells': cells,
        'fixedWorkReplay': replays,
        'replayAllBitIdentical': all(row.get('bitIdentical')
                                     for row in replays if not row['skipped']),
        'verdict': verdict,
    }
    print(json.dumps(document, indent=1))
    with open(f'{out}/wall.json', 'w') as handle:
        json.dump(document, handle, indent=1)
    if verdict is None:
        return 0
    return 0 if verdict['GATE_PASS'] else 1


if __name__ == '__main__':
    sys.exit(main())
