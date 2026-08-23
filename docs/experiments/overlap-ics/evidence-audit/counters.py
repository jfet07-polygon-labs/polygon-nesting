#!/usr/bin/env python3
"""**Counter-identity reconciliation, by a second route.**

    python3 counters.py <evidence-dir> [out.json]

Every identity below is derived from the *engine source* by hand and then
checked against the *committed evidence* without running anything. None of them
is a threshold; each is an arithmetic consequence of where a counter is
incremented, so a violation is a counter lying about its own name.

The derivations, with the line each is read off:

  R1  `focusedSamples == 25 * relocates`
      `relocate.rs` increments `work.relocates` once per relocate that ran
      (`entry_raw > 0`), then draws exactly `config.focused_samples` focused
      poses, bumping `work.focused_samples` once each. The loop is unconditional
      and has no early exit, so the ratio is the frozen constant or a counter is
      wrong.

  R2  `containerSamples == 50 * relocates`      (same argument, container half)

  R3  `containerWinners + focusedWinners + stayPutWinners == relocates`
      the `match best.origin` at the end of `relocate` is total and runs exactly
      once per relocate that ran.

  R4  `containerCommits <= containerWinners`
      `container_commits` is incremented only inside the `Container` arm, under
      `if moved`.

  R5  `sampleEvaluations - focusedSamples - containerSamples` is a NON-NEGATIVE
      EVEN number.
      `evaluate()` is the only site that touches `sample_evaluations`. Its
      callers are: the 25 focused draws (1 each), the 50 container draws
      (1 each), and `coord_descent`, which evaluates candidates in PAIRS -
      `candidates[0]` and `candidates[1]` - once per step. The entry pose is
      scored by `incident_totals` and is deliberately not an evaluation. So the
      residual is 2x the total number of coordinate-descent steps taken, and an
      ODD residual means some evaluation is being counted or skipped off-pair.

  R6  `acceptedMoves <= relocates`
      `accepted_moves` is bumped at most once per relocate, under `if moved`.

  R7  `disruptionMoves >= 2 * disruptions`, and both are zero together.
      `disrupt()` pushes the two swapped pieces into `moved` before any
      follower, then charges `moved.len()`.

  R8  `sum(bite.disruptions) == disruptions`
      the explore loop counts `if disruption.fired`, and `disrupt()` bumps
      `work.disruptions` on exactly the paths that return `fired: true`.

  R9  the funnel row is the bite array reduced:
      `bitesStarted == len(bites)`,
      `proxyBandReached == #{bites: proxyBandReached}`,
      `exactAttempted == #{bites: exactAttempts > 0}`,
      `dualValidPublished == #{bites: published}`.
      **R9c is a KNOWN OVERCLAIM and is reported, not asserted**: the funnel's
      third rung counts BITES that attempted, not attempts. `sum(exactAttempts)`
      is printed beside it so a reader can see the two numbers.

  R10 `len(exactCheckpoints) == work.exactCheckpoints`
      `publish::attempt` bumps `work.exact_checkpoints` immediately before its
      first `Some(..)` return and returns `None` on every path that precedes it;
      `Engine::attempt_publication` pushes the checkpoint of every `Some`.

  R11 every published checkpoint is dual valid, and
      `publishedRawDepthMm` is present iff `kernelExclusiveValid && contractValid`.

  R12 `repairDepthGivebackMm == publishedRawDepth - proxyRawDepth` on every
      published checkpoint, to the last bit. This is the giveback-accounting
      clause of the measurement path: the published depth is measured on the
      INSTALLED (post-repair) placements and the giveback is the difference
      against the pre-repair proxy, so a repair that bought legality with depth
      cannot hide inside the published number.

  R13 `publishedRawDepthMm <= targetDepthMm` on every published checkpoint
      (`publish.rs` refuses "repair would have enlarged the locked strip").

  R14 `repairMaxDisplacementMm <= 4 * epsilon_grid = 0.016` and
      `repairRows <= 4 * n` on every checkpoint.

  R15 `pieceProposals == workers * publicationOrdinal.proposals` at any point
      where both are legible: the trajectory ordinal advances by `n` per master
      iteration (winner only) while the work vector is charged for all eight
      workers. Reported per cell where the numbers exist.

Exit status is 1 if any ASSERTED identity fails; the reported-only rows never
change it.
"""
import json
import os
import sys

FOCUSED_PER_RELOCATE = 25
CONTAINER_PER_RELOCATE = 50
EPSILON_GRID_MM = 0.004
MAX_PIECE_DISPLACEMENT_MM = 4.0 * EPSILON_GRID_MM
REPAIR_ROWS_PER_PIECE = 4


def check(rows, name, ok, detail):
    rows.append({'identity': name, 'ok': bool(ok), 'detail': detail})
    return bool(ok)


def economics(rows, where, econ):
    """R1-R7 on any `relocateEconomics` object."""
    if not econ:
        return
    relocates = econ.get('relocates')
    if relocates is None:
        return
    check(rows, f'{where}/R1 focusedSamples==25*relocates',
          econ.get('focusedSamples') == FOCUSED_PER_RELOCATE * relocates,
          {'focusedSamples': econ.get('focusedSamples'),
           'expected': FOCUSED_PER_RELOCATE * relocates})
    check(rows, f'{where}/R2 containerSamples==50*relocates',
          econ.get('containerSamples') == CONTAINER_PER_RELOCATE * relocates,
          {'containerSamples': econ.get('containerSamples'),
           'expected': CONTAINER_PER_RELOCATE * relocates})
    winners = (econ.get('containerWinners', 0) + econ.get('focusedWinners', 0)
               + econ.get('stayPutWinners', 0))
    check(rows, f'{where}/R3 winners sum==relocates', winners == relocates,
          {'sum': winners, 'relocates': relocates,
           'container': econ.get('containerWinners'),
           'focused': econ.get('focusedWinners'),
           'stayPut': econ.get('stayPutWinners')})
    check(rows, f'{where}/R4 containerCommits<=containerWinners',
          econ.get('containerCommits', 0) <= econ.get('containerWinners', 0),
          {'commits': econ.get('containerCommits'),
           'winners': econ.get('containerWinners')})
    # Absent fields are skipped, never defaulted: `cutclose.py`'s
    # `neuteredRelocate` projection carries only part of the vector, and a `0`
    # default would manufacture a violation the evidence does not contain.
    if econ.get('sampleEvaluations') is not None:
        residual = (econ['sampleEvaluations'] - econ['focusedSamples']
                    - econ['containerSamples'])
        check(rows, f'{where}/R5 CD residual non-negative and even',
              residual >= 0 and residual % 2 == 0,
              {'residual': residual, 'coordDescentSteps': residual // 2,
               'sampleEvaluations': econ['sampleEvaluations']})
    if econ.get('acceptedMoves') is not None:
        check(rows, f'{where}/R6 acceptedMoves<=relocates',
              econ['acceptedMoves'] <= relocates,
              {'acceptedMoves': econ['acceptedMoves'], 'relocates': relocates})
    if econ.get('disruptions') is not None:
        disruptions = econ['disruptions']
        moves = econ.get('disruptionMoves', 0)
        check(rows, f'{where}/R7 disruptionMoves>=2*disruptions',
              moves >= 2 * disruptions and (disruptions == 0) == (moves == 0),
              {'disruptions': disruptions, 'disruptionMoves': moves})


def bite_funnel(rows, reported, where, bites, funnel, publications_count,
                econ):
    """R8-R9 on a wall cell row (or any document carrying `bites`)."""
    if bites is None:
        return
    if econ is not None and econ.get('disruptions') is not None:
        summed = sum(row.get('disruptions', 0) for row in bites)
        check(rows, f'{where}/R8 sum(bite.disruptions)==work.disruptions',
              summed == econ['disruptions'],
              {'summed': summed, 'work': econ['disruptions']})
    if not funnel:
        return
    check(rows, f'{where}/R9a bitesStarted==len(bites)',
          funnel.get('bitesStarted') == len(bites),
          {'funnel': funnel.get('bitesStarted'), 'len': len(bites)})
    band = sum(1 for row in bites if row.get('proxyBandReached'))
    check(rows, f'{where}/R9b proxyBandReached==#bites reaching band',
          funnel.get('proxyBandReached') == band,
          {'funnel': funnel.get('proxyBandReached'), 'counted': band})
    attempted_bites = sum(1 for row in bites if row.get('exactAttempts', 0) > 0)
    attempts_total = sum(row.get('exactAttempts', 0) for row in bites)
    reported.append({
        'note': f'{where}/R9c exactAttempted is a BITE count, not an attempt count',
        'funnelExactAttempted': funnel.get('exactAttempted'),
        'bitesThatAttempted': attempted_bites,
        'attemptsActuallyMade': attempts_total,
        'agreesWithBiteCount': funnel.get('exactAttempted') == attempted_bites,
    })
    published = sum(1 for row in bites if row.get('published'))
    check(rows, f'{where}/R9d dualValidPublished==#published bites',
          funnel.get('dualValidPublished') == published == publications_count
          if publications_count is not None
          else funnel.get('dualValidPublished') == published,
          {'funnel': funnel.get('dualValidPublished'), 'countedBites': published,
           'publications': publications_count})


def checkpoints(rows, where, outcome, pieces):
    """R10-R14 on a full cell document's `outcome`."""
    checks = outcome.get('exactCheckpoints')
    if checks is None:
        return
    work = outcome.get('work', {})
    if 'exactCheckpoints' in work:
        check(rows, f'{where}/R10 len(checkpoints)==work.exactCheckpoints',
              len(checks) == work['exactCheckpoints'],
              {'rows': len(checks), 'counter': work['exactCheckpoints']})
    bad_dual, bad_give, bad_target, bad_cap, bad_rows = [], [], [], [], []
    for index, row in enumerate(checks):
        depth = row.get('publishedRawDepthMm')
        dual = bool(row.get('kernelExclusiveValid') and row.get('contractValid'))
        if (depth is not None) != dual:
            bad_dual.append(index)
        if depth is not None:
            expected = depth - row['proxyRawDepthMm']
            if row['repairDepthGivebackMm'] != expected:
                bad_give.append({'index': index,
                                 'recorded': row['repairDepthGivebackMm'],
                                 'recomputed': expected})
            if depth > row['targetDepthMm']:
                bad_target.append(index)
        if row.get('repairMaxDisplacementMm', 0.0) > MAX_PIECE_DISPLACEMENT_MM:
            bad_cap.append({'index': index,
                            'mm': row['repairMaxDisplacementMm']})
        if pieces and row.get('repairRows', 0) > REPAIR_ROWS_PER_PIECE * pieces:
            bad_rows.append({'index': index, 'rows': row['repairRows']})
    check(rows, f'{where}/R11 published iff dual valid', not bad_dual,
          {'offenders': bad_dual[:5]})
    check(rows, f'{where}/R12 giveback==published-proxy (bit exact)',
          not bad_give, {'offenders': bad_give[:5]})
    check(rows, f'{where}/R13 published<=target', not bad_target,
          {'offenders': bad_target[:5]})
    check(rows, f'{where}/R14a repair<=16um', not bad_cap,
          {'offenders': bad_cap[:5]})
    check(rows, f'{where}/R14b repairRows<=4n', not bad_rows,
          {'offenders': bad_rows[:5], 'pieces': pieces})


def wall_document(rows, reported, path):
    document = json.load(open(path))
    for budget, cell in document.get('cells', {}).items():
        for seed_row in cell.get('seeds', []):
            if not seed_row.get('valid'):
                continue
            where = f'wall/{budget}s/seed{seed_row["seed"]}'
            econ = seed_row.get('relocateEconomics')
            economics(rows, where, econ)
            bite_funnel(rows, reported, where, seed_row.get('bites'),
                        seed_row.get('funnel'), None, econ)
            # R15: the trajectory ordinal is the winner's; the work vector is
            # charged for all eight. Reported, not asserted, because only the
            # last publication's ordinal survives the wall reduction.
            ordinal = seed_row.get('lastPublicationOrdinal')
            if ordinal:
                reported.append({
                    'note': f'{where}/R15 trajectory ordinal is winner-only',
                    'proposals': ordinal['proposals'],
                    'divisibleBy61': ordinal['proposals'] % 61 == 0,
                })


def cell_document(rows, reported, path, pieces=None):
    document = json.load(open(path))
    outcome = document.get('outcome', {})
    where = os.path.basename(path)
    economics(rows, where, outcome.get('relocateEconomics'))
    bite_funnel(rows, reported, where, outcome.get('bites'),
                outcome.get('funnel'), outcome.get('publicationCount'),
                outcome.get('relocateEconomics'))
    checkpoints(rows, where, outcome, pieces)
    work = outcome.get('work')
    if work:
        # The same R1-R7 family, computed off the raw work vector rather than
        # off the driver's `relocateEconomics` projection, so a projection that
        # dropped or renamed a field is caught.
        economics(rows, where + '#work', {
            'relocates': work.get('relocates'),
            'focusedSamples': work.get('focusedSamples'),
            'containerSamples': work.get('containerSamples'),
            'containerWinners': work.get('containerWinners'),
            'focusedWinners': work.get('focusedWinners'),
            'stayPutWinners': work.get('stayPutWinners'),
            'containerCommits': work.get('containerCommits'),
            'sampleEvaluations': work.get('sampleEvaluations'),
            'acceptedMoves': work.get('acceptedMoves'),
            'disruptions': work.get('disruptions'),
            'disruptionMoves': work.get('disruptionMoves'),
        })


def fast_document(rows, reported, path):
    document = json.load(open(path))
    for stage in document.get('stages', []):
        if stage.get('stage') == 'tripwires':
            economics(rows, 'cutclose-fast/tripwires',
                      stage.get('neuteredRelocate'))


def main():
    if len(sys.argv) < 2:
        raise SystemExit(__doc__)
    evidence = sys.argv[1]
    out_path = sys.argv[2] if len(sys.argv) > 2 else None
    rows, reported = [], []

    wall = os.path.join(evidence, 'wall.json')
    if os.path.exists(wall):
        wall_document(rows, reported, wall)
    fast = os.path.join(evidence, 'cutclose-fast.json')
    if os.path.exists(fast):
        fast_document(rows, reported, fast)
    for name, pieces in (('triangle20.json', 20),):
        path = os.path.join(evidence, name)
        if os.path.exists(path):
            cell_document(rows, reported, path, pieces)

    failures = [row for row in rows if not row['ok']]
    document = {
        'experiment': 'overlap-ics',
        'battery': 'evidence-audit-counter-identities',
        'evidenceDir': evidence,
        'identitiesChecked': len(rows),
        'failures': failures,
        'reported': reported,
        'COUNTERS_PASS': not failures,
    }
    print(json.dumps({k: v for k, v in document.items() if k != 'reported'},
                     indent=1))
    if out_path:
        os.makedirs(os.path.dirname(os.path.abspath(out_path)), exist_ok=True)
        with open(out_path, 'w') as handle:
            document['rows'] = rows
            json.dump(document, handle, indent=1)
    return 0 if not failures else 1


if __name__ == '__main__':
    sys.exit(main())
