#!/usr/bin/env python3
"""**The exact-parent chain and the bite/publication join, on full cell documents.**

    python3 chain.py <cell-doc.json> [more...] [--out=doc.json]

`wall.json` reduces a cell to aggregates and keeps `bites`, but drops the
`publications` array, so the join between the two - which is where the
exact-parent-drift defect would show - is not checkable from the committed
reduction alone. These are the identities that need both arrays, run on whole
cell documents (the raw wall cells, or the fixed-work replays):

  C1  every publication's `parentFingerprint` is the previous publication's
      `placementFingerprint`, and the first one's is the constructor's.
      (Sol review 17 Round 2 §2's exact-parent drift.)

  C2  for every published EXPLORE bite after the first, the bite's
      `widthBeforeMm` is the previous publication's `publishedRawDepthMm`,
      **bit for bit** - not its `targetDepthMm` and not a pre-repair proxy
      depth. This is the clause `install_publication` exists to enforce and it
      is the one a driver-level tautology cannot see: the driver's own
      `targetFromPublishedDepth` check recomputes `previous * 0.999` from the
      publication list, which is the same arithmetic on the same side of the
      seam; this one joins two INDEPENDENT arrays.

  C3  `bites[k].widthAfterMm == publications-for-that-bite.targetDepthMm`
      exactly (the record and the publication agree about which width the
      separation was aiming at).

  C4  every publication's `publishedRawDepthMm <= targetDepthMm`, and the
      published depth series is non-increasing across the explore phase.

  C5  the `improvedIncumbent` flag is true exactly on publications that lowered
      the running best published depth by more than the 1 um minimum. The flag
      is written by comparing the engine's incumbent AFTER the attempt has
      already updated it, which is a subtle ordering; this recomputes it from
      the depth series alone.

  C6  `repairDepthGivebackMm` on a published bite equals the same publication's
      exact checkpoint row, so the giveback the anytime curve reports is the one
      the publication path measured.
"""
import json
import os
import sys

MIN_IMPROVEMENT_MM = 0.001


def check(rows, name, ok, detail):
    rows.append({'identity': name, 'ok': bool(ok), 'detail': detail})


def analyse(rows, path):
    document = json.load(open(path))
    where = os.path.basename(path)
    outcome = document.get('outcome', {})
    constructor = document.get('constructor', {})
    publications = outcome.get('publications', [])
    bites = outcome.get('bites', [])
    checkpoints = outcome.get('exactCheckpoints', [])
    if not publications:
        check(rows, f'{where}/C0 the cell published at least once', False,
              {'publications': 0})
        return

    # C1
    expected = constructor.get('placementFingerprint')
    broken = []
    for row in publications:
        if row['parentFingerprint'] != expected:
            broken.append({'bite': row['ordinal']['bite'],
                           'parent': row['parentFingerprint'][:16],
                           'expected': (expected or '')[:16]})
        expected = row['placementFingerprint']
    check(rows, f'{where}/C1 the publication chain is a chain from the constructor',
          not broken, {'offenders': broken[:4], 'links': len(publications)})

    # C2 / C3
    by_bite = {}
    for row in publications:
        by_bite.setdefault(row['ordinal']['bite'], row)
    drift, target_gap = [], []
    previous_depth = None
    for bite in bites:
        published = by_bite.get(bite['ordinal'])
        if bite['phase'] == 'explore' and previous_depth is not None:
            if bite['widthBeforeMm'] != previous_depth:
                drift.append({'bite': bite['ordinal'],
                              'widthBeforeMm': bite['widthBeforeMm'],
                              'previousPublishedMm': previous_depth,
                              'deltaMm': bite['widthBeforeMm'] - previous_depth})
        if published is not None:
            if published['targetDepthMm'] != bite['widthAfterMm']:
                target_gap.append({'bite': bite['ordinal'],
                                   'publicationTarget': published['targetDepthMm'],
                                   'biteWidthAfter': bite['widthAfterMm']})
            previous_depth = published['publishedRawDepthMm']
    check(rows,
          f'{where}/C2 each explore bite starts at the previous PUBLISHED depth, bit for bit',
          not drift, {'offenders': drift[:4]})
    check(rows, f'{where}/C3 the bite record and its publication agree on the target',
          not target_gap, {'offenders': target_gap[:4]})

    # C4
    over = [row['ordinal']['bite'] for row in publications
            if row['publishedRawDepthMm'] > row['targetDepthMm']]
    explore_depths = [row['publishedRawDepthMm'] for row in publications
                      if row['phase'] == 'explore']
    monotone = all(explore_depths[index] <= explore_depths[index - 1]
                   for index in range(1, len(explore_depths)))
    check(rows, f'{where}/C4 published <= target, and the explore series is non-increasing',
          not over and monotone,
          {'overTarget': over[:4], 'exploreMonotone': monotone,
           'firstExploreDepth': explore_depths[0] if explore_depths else None,
           'lastExploreDepth': explore_depths[-1] if explore_depths else None})

    # C5
    best = constructor.get('rawSourceDepthMm')
    flag_gap = []
    for row in publications:
        improved = row['publishedRawDepthMm'] < best - MIN_IMPROVEMENT_MM
        if bool(row['improvedIncumbent']) != improved:
            flag_gap.append({'bite': row['ordinal']['bite'],
                             'flag': row['improvedIncumbent'],
                             'recomputed': improved,
                             'depthMm': row['publishedRawDepthMm'],
                             'incumbentBeforeMm': best})
        if improved:
            best = row['publishedRawDepthMm']
    check(rows, f'{where}/C5 improvedIncumbent == a >1 um gain on the running best',
          not flag_gap, {'offenders': flag_gap[:4], 'finalIncumbentMm': best})

    # C6
    published_checkpoints = [row for row in checkpoints
                             if row.get('publishedRawDepthMm') is not None]
    give_gap = []
    if len(published_checkpoints) == len(publications):
        for checkpoint, publication in zip(published_checkpoints, publications):
            if (checkpoint['publishedRawDepthMm'] != publication['publishedRawDepthMm']
                    or checkpoint['repairDepthGivebackMm']
                    != publication['repairDepthGivebackMm']
                    or checkpoint['repairRows'] != publication['repairRows']):
                give_gap.append({'bite': publication['ordinal']['bite'],
                                 'checkpoint': checkpoint['publishedRawDepthMm'],
                                 'publication': publication['publishedRawDepthMm']})
    check(rows,
          f'{where}/C6 every publication row equals its own exact-checkpoint row',
          len(published_checkpoints) == len(publications) and not give_gap,
          {'publishedCheckpoints': len(published_checkpoints),
           'publications': len(publications), 'offenders': give_gap[:4]})


def main():
    argv = [value for value in sys.argv[1:] if not value.startswith('--out=')]
    out_doc = next((value[6:] for value in sys.argv[1:]
                    if value.startswith('--out=')), None)
    if not argv:
        raise SystemExit(__doc__)
    rows = []
    for path in argv:
        analyse(rows, path)
    failures = [row for row in rows if not row['ok']]
    document = {
        'experiment': 'overlap-ics',
        'battery': 'evidence-audit-publication-chain',
        'documents': argv,
        'identitiesChecked': len(rows),
        'failures': failures,
        'CHAIN_PASS': not failures,
    }
    print(json.dumps(document, indent=1))
    if out_doc:
        os.makedirs(os.path.dirname(os.path.abspath(out_doc)), exist_ok=True)
        with open(out_doc, 'w') as handle:
            document['rows'] = rows
            json.dump(document, handle, indent=1)
    return 0 if not failures else 1


if __name__ == '__main__':
    sys.exit(main())
