#!/usr/bin/env python3
"""The `CutCloseRelocate` FAST additions: the canary and the four tripwires.

    python3 cutclose.py            # every stage
    python3 cutclose.py canary     # one stage

Exits non-zero when any stage fails. Every stage runs the **fixed-work** arm of
`--cell=cutclose`, so nothing here constructs an `Instant` inside a trajectory
and two processes are compared bit for bit rather than statistically.

The stages, and which clause of the spec of record each one is:

  `canary`     Grok review 12 Round 2 §6.3.4 / Sol review 17 Round 2 §6:
               "mixed-61, seed 0, one bite + separate to publication or
               strike-out. **PASS = dual-valid child at `W = 0.999 x D*`.**
               FAIL here is a member fail; do not run the 9-seed wall."
               This stage is the one that licenses the wall battery, and
               `wall.py` refuses to start without its green document.

  `bites`      §6.3.5: `K = 8` explore bites, fixed work, no clock, two
               processes, stripped documents identical.

  `merge`      Sol review 17 Round 2 §2's mandatory addition 2, across two
               **processes** rather than two in-process runs: each worker seed,
               each master snapshot, the winning worker ordinal, the pose and
               weight fingerprint after every master iteration, and the exact
               parent after every bite. A test that only proves `workers = 1`
               deterministic is insufficient, and so is one that only proves it
               inside one address space - the eight OS threads this member
               spawns are new concurrency in the tree and completion order is
               exactly what a single process cannot vary on demand.

  `tripwires`  The four pre-named defects, read off the **driver's** document
               rather than off a unit test, because a unit vector proves the
               function and this proves the shipped binary:

                 * **neutered relocate** (Grok's #1): the 50 container-wide
                   samples run and at least one of them commits. A relocate
                   whose committed poses all lie inside the old `ladder_top` is
                   "PGS in a sampling costume".
                 * **exact-parent drift** (Sol's #1 addition, the old
                   `mod.rs:295`): every publication's `parentFingerprint` is the
                   previous publication's `placementFingerprint`, and the first
                   one's is the constructor's. The next bite's target is
                   `0.999 x` the *published raw depth*, never the target the
                   separation was aiming at and never a pre-repair proxy depth.
                 * **cut-close bits**: `delta = W_new - W_old` exactly, the
                   explore split is `W/2` exactly, and the bite moves the far
                   side rather than scaling everything.
                 * **shrink on Phi = 0**: no width advances without a dual-valid
                   publication at that width. `bitesStarted` above
                   `dualValidPublished` is legal; a *width* that moved without a
                   publication is not.

`tripwires` is scored on the same K=8 document the `bites` stage compares, so
the two stages cannot disagree about what ran.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib  # noqa: E402

# The constructor's own exact depth on mixed-61 under the exact-clearance
# contract, and the 0.1 % bite from it. Pinned rather than read back off the
# run: a canary that recomputed its own expectation from the number it measured
# would pass on any number at all.
D_STAR_MM = 182.976
EXPLORE_STEP = 0.001
FIRST_BITE_TARGET_MM = 182.793024

# The fixed-work quotas. `iters` is generous on purpose - the canary must be
# able to tell "the member cannot publish a 0.1 % bite" from "the cap stopped
# it", and a strike-out inside 2,000 master iterations is the former.
CANARY = dict(mode='fixed', bites=1, attempts=1, iters=2000, compressbites=0,
              workers=8, seed=0)
SEQUENCE = dict(mode='fixed', bites=8, attempts=2, iters=400, compressbites=2,
                workers=8, seed=0)
MERGE = dict(mode='fixed', bites=3, attempts=1, iters=12, compressbites=1,
             workers=8, seed=0, fingerprints=1)


def two_process(tag, out, **options):
    """Two separate processes, same options, compared after stripping `wall`."""
    first, _, status_a, err_a = lib.run(
        'cutclose', 'mixed-61', f'{out}/{tag}-process-a.json', **options)
    second, _, status_b, err_b = lib.run(
        'cutclose', 'mixed-61', f'{out}/{tag}-process-b.json', **options)
    return {
        'exitA': status_a,
        'exitB': status_b,
        'stderrA': err_a[-400:] if err_a else '',
        'stderrB': err_b[-400:] if err_b else '',
        'digestA': lib.digest(first),
        'digestB': lib.digest(second),
        'strippedFields': lib.WALL_FIELDS,
        'bitIdentical': (status_a == 0 and status_b == 0
                         and lib.stripped(first) == lib.stripped(second)),
    }, first, second


def canary(out):
    """**The stage that licenses the wall battery.**"""
    doc, wall, status, err = lib.run(
        'cutclose', 'mixed-61', f'{out}/cutclose-canary.json', **CANARY)
    if status != 0:
        return {'stage': 'canary', 'pass': False, 'exit': status,
                'stderr': err[-800:]}
    measured = doc.get('firstBiteCanary', {})
    outcome = doc.get('outcome', {})
    bites = outcome.get('bites', [])
    first = bites[0] if bites else {}
    detail = {
        'stage': 'canary',
        'exit': status,
        'constructorDepthMm': doc.get('constructor', {}).get('rawSourceDepthMm'),
        'pinnedConstructorDepthMm': D_STAR_MM,
        'pinnedFirstBiteTargetMm': FIRST_BITE_TARGET_MM,
        'measured': measured,
        'firstBite': first,
        'funnel': outcome.get('funnel'),
        'invalidPublications': outcome.get('invalidPublications'),
        'wallSeconds': wall,
        'searchSeconds': doc.get('wall', {}).get('searchSeconds'),
        'constructorSeconds': doc.get('wall', {}).get('constructorSeconds'),
    }
    detail['pass'] = bool(
        detail['constructorDepthMm'] == D_STAR_MM
        and measured.get('published') is True
        and measured.get('strictChild') is True
        and measured.get('dualValid') is True
        and measured.get('withinTarget') is True
        and measured.get('targetMatchesExpected') is True
        and measured.get('expectedTargetMm') == FIRST_BITE_TARGET_MM
        and measured.get('publishedRawDepthMm') is not None
        and measured.get('publishedRawDepthMm') <= FIRST_BITE_TARGET_MM
        and outcome.get('invalidPublications') == 0)
    return detail


def bites(out):
    """K = 8 explore bites, fixed work, two processes, stripped-identical."""
    compare, first, _ = two_process('cutclose-k8', out, **SEQUENCE)
    outcome = first.get('outcome', {})
    return {
        'stage': 'bites',
        'options': SEQUENCE,
        'twoProcess': compare,
        'exploreBites': outcome.get('exploreBites'),
        'compressBites': outcome.get('compressBites'),
        'publicationCount': outcome.get('publicationCount'),
        'funnel': outcome.get('funnel'),
        'depthMm': outcome.get('depthMm'),
        'pass': bool(compare['bitIdentical']
                     and outcome.get('exploreBites') == SEQUENCE['bites']),
    }


def merge(out):
    """Eight-worker merge determinism, **across two processes**."""
    compare, first, second = two_process('cutclose-merge', out, **MERGE)
    a = first.get('outcome', {}).get('fingerprints', [])
    b = second.get('outcome', {}).get('fingerprints', [])
    # The per-iteration comparison, spelled out rather than left to the
    # whole-document digest: Sol's addition 2 names the four things it wants
    # compared, and a reader must be able to see that they were.
    winners_same = [row['winner'] for row in a] == [row['winner'] for row in b]
    states_same = [row['state'] for row in a] == [row['state'] for row in b]
    guided_same = ([row['winnerGuided'] for row in a]
                   == [row['winnerGuided'] for row in b])
    parents_a = [(row['parentFingerprint'], row['placementFingerprint'])
                 for row in first.get('outcome', {}).get('publications', [])]
    parents_b = [(row['parentFingerprint'], row['placementFingerprint'])
                 for row in second.get('outcome', {}).get('publications', [])]
    contested = sum(1 for row in a if row['contested'])
    return {
        'stage': 'merge',
        'options': MERGE,
        'twoProcess': compare,
        'iterations': len(a),
        'iterationsB': len(b),
        'winnerOrdinalsIdentical': winners_same,
        'masterStateFingerprintsIdentical': states_same,
        'winnerGuidedIdentical': guided_same,
        'exactParentChainIdentical': parents_a == parents_b,
        'contestedIterations': contested,
        'distinctWinners': sorted({row['winner'] for row in a}),
        'workers': MERGE['workers'],
        'pass': bool(compare['bitIdentical'] and len(a) > 0
                     and len(a) == len(b) and winners_same and states_same
                     and guided_same and parents_a == parents_b),
    }


def tripwires(out):
    """The four pre-named defects, on the shipped binary's own document."""
    doc, _, status, err = lib.run(
        'cutclose', 'mixed-61', f'{out}/cutclose-tripwires.json', **SEQUENCE)
    if status != 0:
        return {'stage': 'tripwires', 'pass': False, 'exit': status,
                'stderr': err[-800:]}
    outcome = doc.get('outcome', {})
    economics = outcome.get('relocateEconomics', {})
    publications = outcome.get('publications', [])
    rows = outcome.get('bites', [])
    constructor_fp = doc.get('constructor', {}).get('placementFingerprint')

    # 1. Neutered relocate. The container-wide half of the pool ran, and at
    #    least one of its winners moved a piece.
    relocates = economics.get('relocates', 0)
    neutered = {
        'relocates': relocates,
        'containerSamples': economics.get('containerSamples'),
        'focusedSamples': economics.get('focusedSamples'),
        'containerSamplesPerRelocate':
            (economics.get('containerSamples', 0) / relocates) if relocates else 0.0,
        'focusedSamplesPerRelocate':
            (economics.get('focusedSamples', 0) / relocates) if relocates else 0.0,
        'containerWinners': economics.get('containerWinners'),
        'containerCommits': economics.get('containerCommits'),
        'focusedWinners': economics.get('focusedWinners'),
        'stayPutWinners': economics.get('stayPutWinners'),
    }
    neutered['pass'] = bool(
        relocates > 0
        and neutered['containerSamplesPerRelocate'] >= 50.0
        and neutered['focusedSamplesPerRelocate'] >= 25.0
        and economics.get('containerCommits', 0) >= 1)

    # 2. Exact-parent drift. The publication chain is a chain, and each bite's
    #    target is 0.1 % off the previous PUBLISHED raw depth.
    chain = []
    parent_ok = True
    expected_parent = constructor_fp
    previous_depth = doc.get('constructor', {}).get('rawSourceDepthMm')
    for row in publications:
        target_ok = True
        if row['phase'] == 'explore':
            expected_target = previous_depth * (1.0 - EXPLORE_STEP)
            target_ok = row['targetDepthMm'] == expected_target
        chain.append({
            'bite': row['ordinal']['bite'],
            'phase': row['phase'],
            'parentMatches': row['parentFingerprint'] == expected_parent,
            'targetFromPublishedDepth': target_ok,
            'publishedWithinTarget':
                row['publishedRawDepthMm'] <= row['targetDepthMm'],
            'repairRows': row['repairRows'],
        })
        parent_ok = (parent_ok and chain[-1]['parentMatches']
                     and chain[-1]['targetFromPublishedDepth']
                     and chain[-1]['publishedWithinTarget'])
        expected_parent = row['placementFingerprint']
        previous_depth = row['publishedRawDepthMm']
    drift = {'links': chain, 'firstParentIsConstructor':
             bool(publications) and publications[0]['parentFingerprint'] == constructor_fp,
             'pass': bool(publications) and parent_ok}

    # 3. Cut-close bits, as the driver sees them: the delta is exactly the width
    #    change, the explore split is exactly mid-depth, and a bite that moved
    #    nothing is reported rather than hidden.
    cut = []
    for row in rows:
        expected_delta = row['widthAfterMm'] - row['widthBeforeMm']
        expected_split = (row['widthBeforeMm'] / 2.0
                          if row['phase'] == 'explore' else None)
        cut.append({
            'ordinal': row['ordinal'],
            'phase': row['phase'],
            'deltaExact': row['deltaMm'] == expected_delta,
            'splitIsMidDepth': (row['splitYMm'] == expected_split
                                if expected_split is not None else None),
            'stepIsExploreStep': (row['step'] == EXPLORE_STEP
                                  if row['phase'] == 'explore' else None),
            'movedPieces': row['movedPieces'],
        })
    cut_pass = all(row['deltaExact'] for row in cut) and all(
        row['splitIsMidDepth'] is not False and row['stepIsExploreStep'] is not False
        for row in cut) and any(row['movedPieces'] > 0 for row in cut)

    # 4. Shrink on Phi = 0. Every width the loop LEFT was one it published at.
    #    The explore phase stops at the first bite it cannot publish, so the
    #    number of distinct explore widths visited must equal the number of
    #    explore publications plus at most the one unpublished bite it died on.
    explore_rows = [row for row in rows if row['phase'] == 'explore']
    explore_published = [row for row in publications if row['phase'] == 'explore']
    unpublished = [row for row in explore_rows if not row['published']]
    phi_zero = {
        'exploreBites': len(explore_rows),
        'explorePublications': len(explore_published),
        'unpublishedExploreBites': len(unpublished),
        'everyPublishedBiteEndedAtItsTarget': all(
            row['publishedRawDepthMm'] <= row['targetDepthMm']
            for row in publications),
        # A width is only ever left behind on a publication: the loop breaks out
        # of explore on the first failure, so at most one bite can be unpublished
        # and it is always the last one.
        'atMostOneUnpublishedAndItIsLast': (
            len(unpublished) == 0
            or (len(unpublished) == 1
                and unpublished[0]['ordinal'] == explore_rows[-1]['ordinal'])),
    }
    phi_zero['pass'] = bool(
        phi_zero['everyPublishedBiteEndedAtItsTarget']
        and phi_zero['atMostOneUnpublishedAndItIsLast'])

    return {
        'stage': 'tripwires',
        'options': SEQUENCE,
        'neuteredRelocate': neutered,
        'exactParentDrift': drift,
        'cutCloseBits': {'bites': cut, 'pass': bool(cut_pass)},
        'shrinkOnPhiZero': phi_zero,
        'invalidPublications': outcome.get('invalidPublications'),
        'pass': bool(neutered['pass'] and drift['pass'] and cut_pass
                     and phi_zero['pass']
                     and outcome.get('invalidPublications') == 0),
    }


STAGES = {
    'canary': canary,
    'bites': bites,
    'merge': merge,
    'tripwires': tripwires,
}
ORDER = ['canary', 'tripwires', 'bites', 'merge']


def main():
    out = os.environ.get('ICS_OUT', lib.OUT)
    os.makedirs(out, exist_ok=True)
    wanted = sys.argv[1:] or ORDER
    results = []
    for name in wanted:
        if name not in STAGES:
            raise SystemExit(f'unknown stage {name}')
        results.append(STAGES[name](out))
    failures = [row['stage'] for row in results if not row['pass']]
    document = {
        'experiment': 'overlap-ics',
        'battery': 'cutclose-fast-additions',
        'binary': lib.BIN,
        'stages': results,
        'failures': failures,
        # Named so `wall.py` can read one field and refuse to start.
        'CANARY_PASS': all(row['pass'] for row in results
                           if row['stage'] == 'canary'),
        'CUTCLOSE_FAST_PASS': not failures,
    }
    print(json.dumps(document, indent=1))
    with open(f'{out}/cutclose-fast.json', 'w') as handle:
        json.dump(document, handle, indent=1)
    return 0 if not failures else 1


if __name__ == '__main__':
    sys.exit(main())
