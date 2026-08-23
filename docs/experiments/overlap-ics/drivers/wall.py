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

**Every cell row carries its raw `bites` array now.** Round 1's reduction
dropped it, and Sol review 18's second non-gating risk is that the README's
per-bite statements were consequently not reconstructible from committed
evidence. They are the largest thing in this document and they are the point of
it: `bites[21]` is the 22nd bite, the one the whole autopsy is about.

**And every cell row now carries its `checkpointFrame`.** The same reduction
dropped every per-publication clock reading, and §0.1's "a publication
completed after 10.000 s cannot change that verdict" was implemented against
the engine's LOOP-relative `wallSeconds` rather than the request-relative
budget - a comparison that cannot fire, because the loop's own clock is bounded
by `budget - constructorSeconds`. See `cell()` and
`docs/experiments/overlap-ics/evidence-audit/checkpoint-frame.py`.
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
    # **The checkpoint filter, in the budget's own frame.**
    #
    # §0.1: "a publication completed after 10.000 s cannot change that verdict".
    # The engine's `PublishedBite.wallSeconds` is `Pacer::elapsed_s()`, and the
    # `Pacer` is constructed inside `Engine::run_cutclose` - **after** the
    # constructor has already spent its share of the request's budget. So
    # `wallSeconds` is measured from the moment the loop entered and the budget
    # is measured from the decoded request, and the two are 2.3 s apart on
    # mixed-61.
    #
    # Comparing `wallSeconds <= limit` directly - which is what this driver did
    # through round 1 and the rerun - is therefore not the §0.1 clause. It is a
    # comparison whose left side is bounded above by
    # `limit - constructorSeconds` by construction, so it can never exclude
    # anything, on any cell, whatever the loop does. Measured on nine 10 s
    # cells the headroom was 2.307 s, while the closest publication sat 1.9 ms
    # inside the budget in the frame that matters. `docs/experiments/
    # overlap-ics/evidence-audit/checkpoint-frame.py` is the vector.
    #
    # The offset is not emitted directly, so both bounds the document does carry
    # are computed and reported:
    #
    #   * `requestSecondsLower = constructorSeconds + wallSeconds` - the
    #     constructor alone; excludes the engine construction between the two
    #     clock reads, so it is a LOWER bound on a publication's age.
    #   * `requestSecondsUpper = loopEntrySeconds + wallSeconds` - the offset
    #     itself, emitted by the driver one statement before the `Pacer`
    #     exists. Cells written before the economics round do not carry it and
    #     fall back to `(totalSeconds - searchSeconds) + wallSeconds`, which
    #     also includes the document build and is a much looser bound.
    #
    # The verdict uses the lower bound: a publication is excluded only when it
    # is *certainly* late. A publication whose two bounds straddle the budget
    # is counted, and reported in `publicationsUndecidedByFrame` so the reader
    # can see that the document could not settle it.
    #
    # **The arithmetic now lives in `lib.within_budget`**, unchanged in every
    # bit, because `control.py` had the same defect one step milder and two
    # copies of a repair drift. `frame_vector.py` in the economics round's
    # census re-derives this cell's four frame fields for all 27 committed raw
    # cells through both the old inline code and the shared helper, and
    # requires them to be identical.
    limit = SECONDS[budget]
    constructor_s = doc.get('wall', {}).get('constructorSeconds')
    search_s = doc.get('wall', {}).get('searchSeconds')
    total_s = doc.get('wall', {}).get('totalSeconds')
    lower_offset, upper_offset = lib.checkpoint_frame(doc)
    within, late, undecided = lib.within_budget(publications, doc, limit)
    strict = [row for row in within
              if row['placementFingerprint']
              != constructor.get('placementFingerprint')]
    best = min((row['publishedRawDepthMm'] for row in strict), default=None)
    incumbent = outcome.get('incumbent', {})
    loop_seconds = [row['wallSeconds'] for row in publications
                    if row.get('wallSeconds') is not None]
    return {
        'seed': seed,
        'budgetSeconds': limit,
        'exit': status,
        'valid': True,
        # RV3: the reduction names the bytes it reduced. The audit's
        # revalidation had to re-derive all 702 committed cell-row fields to
        # bind this document to the raw cells it came from.
        'sourcePath': f'{out}/{tag}.json',
        'sourceSha256': lib.source_sha256(f'{out}/{tag}.json'),
        'constructorDepthMm': constructor.get('rawSourceDepthMm'),
        'constructorSeconds': constructor_s,
        'searchSeconds': search_s,
        'totalSeconds': total_s,
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
        # **The §0.1 clause, reconstructible.** Round 1 and the rerun dropped
        # every per-publication clock reading, so nobody downstream could check
        # whether a qualifying publication landed after the budget. These four
        # numbers are what the clause needs and they are cheap.
        'checkpointFrame': {
            'loopRelativeMaxSeconds': max(loop_seconds, default=None),
            'requestSecondsLowerMax': (None if lower_offset is None or not loop_seconds
                                       else lower_offset + max(loop_seconds)),
            'requestSecondsUpperMax': (None if upper_offset is None or not loop_seconds
                                       else upper_offset + max(loop_seconds)),
            'publicationsExcludedAsLate': len(late),
            'publicationsUndecidedByFrame': len(undecided),
            'bestStrictChildRequestSecondsLower': min(
                (lib.request_seconds(row, lower_offset) for row in strict
                 if row['publishedRawDepthMm'] == best
                 and lib.request_seconds(row, lower_offset) is not None),
                default=None),
        },
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
        # **The raw per-bite schedule, verbatim.** Round 1 reduced the cell
        # document to the aggregates above and dropped `outcome.bites`, so the
        # README's per-bite claims - 5,319 master iterations, 0 strikes, 0
        # disruptions on the 22nd bite - could not be reconstructed from
        # committed evidence and had to be read out of a temporary directory.
        # Sol review 18, general fidelity, risk 2. Not reduced, not rounded,
        # not renamed: every field the engine emitted, per bite.
        'bites': outcome.get('bites'),
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
            'publicationsExcludedAsLate': sum(
                (row.get('checkpointFrame') or {}).get('publicationsExcludedAsLate', 0)
                for row in rows),
            'publicationsUndecidedByFrame': sum(
                (row.get('checkpointFrame') or {}).get('publicationsUndecidedByFrame', 0)
                for row in rows),
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
            # §0.1's "completed after 10.000 s" clause, in the budget's own
            # frame. `publicationsExcludedAsLate` is what the clause removed;
            # `publicationsUndecidedByFrame` is what this document cannot
            # settle, because the engine emits a loop-relative clock and the
            # offset is only bracketed. Both belong beside the quorum.
            'publicationsExcludedAsLate': gate['publicationsExcludedAsLate'],
            'publicationsUndecidedByFrame': gate['publicationsUndecidedByFrame'],
            'GATE_PASS': bool(gate['qualifyingCount'] >= QUORUM
                              and invalid_everywhere == 0
                              and gate['allSeedsValid']),
        }

    replays = replay(out, gate['seeds']) if gate is not None else []
    document = {
        'experiment': 'overlap-ics',
        'battery': 'cutclose-round1-wall',
        'binary': lib.BIN,
        # RV3: every cell document this reduction spawned, with its
        # sha256, so a reader can bind any row here to the bytes it
        # came from without re-deriving the reduction.
        'cellSources': lib.MANIFEST,
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
