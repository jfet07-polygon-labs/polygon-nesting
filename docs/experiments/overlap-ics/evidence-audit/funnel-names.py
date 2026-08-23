#!/usr/bin/env python3
"""**What the funnel's rungs actually count, measured on full cell documents.**

    python3 funnel-names.py <cell-doc.json> [more...] [--out=doc.json]

The failure license names the funnel `bitesStarted -> proxyBandReached ->
exactAttempted -> dualValidPublished`. Three of those four rungs count
something other than the noun they carry, and this measures the gap on real
cells rather than arguing about it:

  N1  `funnel.exactAttempted` counts **bites that attempted**, not attempts.
      Already recorded in the rerun README §9; the per-cell ratio is printed so
      a reader can see how big the difference is on this fixture.

  N2  `bites[].exactAttempts` counts **entries into the 4 um band**, not calls
      into `publish::attempt`. `Engine::attempt_publication` returns early on an
      unchanged pose digest, and `publish::attempt` returns `None` (without a
      checkpoint row and without bumping `work.exactCheckpoints`) whenever
      `max_g > band`, `proxy > T`, or `proxy > incumbent - 1 um`. So
      `sum(exactAttempts)` is an upper bound on the number of times exact
      geometry was asked anything, and `len(exactCheckpoints)` is the number of
      times it actually was.

  N3  `invalidPublications` is **structurally zero**. `publish::attempt` writes
      `published_raw_depth_mm = Some(..)` only after `kernel_exclusive_valid`
      and `contract_valid` are both true, so the predicate
      `published.is_some() && !(kernel && contract)` has no reachable witness.
      The number is an invariant of the emitter, not a measurement of the
      layouts, and the committed evidence carries no placements for an
      independent validator to re-check. Reported, with the count of emitted
      publications that carry no placement data.
"""
import json
import os
import sys


def analyse(path):
    document = json.load(open(path))
    outcome = document.get('outcome', {})
    bites = outcome.get('bites', [])
    checkpoints = outcome.get('exactCheckpoints', [])
    work = outcome.get('work', {})
    funnel = outcome.get('funnel', {})
    publications = outcome.get('publications', [])
    attempts = sum(row.get('exactAttempts', 0) for row in bites)
    attempting_bites = sum(1 for row in bites if row.get('exactAttempts', 0) > 0)
    refusals = sum(1 for row in checkpoints if row.get('refusal'))
    return {
        'cell': os.path.basename(path),
        'seed': document.get('seed'),
        'N1': {
            'funnelExactAttempted': funnel.get('exactAttempted'),
            'bitesThatAttempted': attempting_bites,
            'attemptsSummedOverBites': attempts,
            'overclaimFactor': (attempts / attempting_bites) if attempting_bites else None,
        },
        'N2': {
            'bandEntriesCountedAsAttempts': attempts,
            'exactCheckpointRows': len(checkpoints),
            'workExactCheckpoints': work.get('exactCheckpoints'),
            'bandEntriesThatNeverReachedExactGeometry':
                attempts - len(checkpoints),
            'checkpointRowsEqualWorkCounter':
                len(checkpoints) == work.get('exactCheckpoints'),
            'checkpointsThatRefused': refusals,
        },
        'N3': {
            'invalidPublications': outcome.get('invalidPublications'),
            'publications': len(publications),
            'publicationsCarryingPlacements':
                sum(1 for row in publications if 'placements' in row),
            'structurallyUnfalsifiable': True,
        },
    }


def main():
    argv = [value for value in sys.argv[1:] if not value.startswith('--out=')]
    out_doc = next((value[6:] for value in sys.argv[1:]
                    if value.startswith('--out=')), None)
    if not argv:
        raise SystemExit(__doc__)
    cells = [analyse(path) for path in argv]
    totals = {
        'cells': len(cells),
        'funnelExactAttempted': sum(row['N1']['funnelExactAttempted'] or 0 for row in cells),
        'attemptsSummedOverBites': sum(row['N1']['attemptsSummedOverBites'] for row in cells),
        'exactCheckpointRows': sum(row['N2']['exactCheckpointRows'] for row in cells),
        'bandEntriesThatNeverReachedExactGeometry':
            sum(row['N2']['bandEntriesThatNeverReachedExactGeometry'] for row in cells),
        'checkpointsThatRefused': sum(row['N2']['checkpointsThatRefused'] for row in cells),
        'allCheckpointRowsEqualWorkCounter':
            all(row['N2']['checkpointRowsEqualWorkCounter'] for row in cells),
        'publicationsCarryingPlacements':
            sum(row['N3']['publicationsCarryingPlacements'] for row in cells),
        'publications': sum(row['N3']['publications'] for row in cells),
    }
    document = {
        'experiment': 'overlap-ics',
        'battery': 'evidence-audit-funnel-names',
        'totals': totals,
        'cells': cells,
    }
    print(json.dumps({k: v for k, v in document.items() if k != 'cells'}, indent=1))
    if out_doc:
        os.makedirs(os.path.dirname(os.path.abspath(out_doc)), exist_ok=True)
        with open(out_doc, 'w') as handle:
            json.dump(document, handle, indent=1)
    return 0


if __name__ == '__main__':
    sys.exit(main())
