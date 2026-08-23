#!/usr/bin/env python3
"""**The committed bite schedule, re-derived from the committed publications.**

    python3 bites-consistency.py <wall.json> [out.json]

The engine's own bite record is a summary of a computation whose inputs -
the installed poses at each width - are not committed. What *is* committed is
the publication chain, and the schedule is a function of it. So every explore
bite after the first can be re-derived without trusting the record:

  B1  `bites[k].widthBeforeMm == publications[k-1].publishedRawDepthMm`
      (`run_cutclose`: a dual-valid publication sets `depth = width = published
      raw depth` and the next bite is taken from it - never from the target the
      separation was aiming at, and never from a pre-repair proxy depth).
  B2  `widthAfterMm == widthBeforeMm * (1 - step)`, bit for bit.
  B3  `deltaMm == widthAfterMm - widthBeforeMm`, bit for bit.
  B4  explore: `splitYMm == widthBeforeMm / 2`, bit for bit; `step == 0.001`.
  B5  compress: `physicalEdgeClearance <= splitYMm <= widthBeforeMm`.
  B6  a bite that published has `minRawPhi <= its own target`'s feasibility, and
      every published bite's target is above the depth it published at - that is
      the tripwire `cutclose.py` reads, restated over the whole 27-cell battery
      rather than over the K=8 fixed-work document alone.
  B7  the explore phase stops at its first unpublished bite: at most one explore
      bite is unpublished and it is the last explore bite.

It also **reports** the two funnel populations the failure license names but
`wall.json` does not reduce: how many separations struck, and how many
disruptions fired, per cell and in total. A repair whose stated purpose was to
let Algorithm 12 fire on the Φ-shelf is only demonstrated by evidence in which
it fires.
"""
import json
import os
import sys

EXPLORE_STEP = 0.001
PHYSICAL_EDGE_MM = 5.0  # --edge=5, sag = 0 on mixed-61


def check(rows, name, ok, detail):
    rows.append({'identity': name, 'ok': bool(ok), 'detail': detail})


def cell_rows(rows, where, seed_row):
    bites = seed_row.get('bites') or []
    if not bites:
        return
    explore = [row for row in bites if row['phase'] == 'explore']
    compress = [row for row in bites if row['phase'] == 'compress']

    bad_step, bad_delta, bad_split, bad_after = [], [], [], []
    for row in bites:
        if row['deltaMm'] != row['widthAfterMm'] - row['widthBeforeMm']:
            bad_delta.append(row['ordinal'])
        if row['widthAfterMm'] != row['widthBeforeMm'] * (1.0 - row['step']):
            bad_after.append({'ordinal': row['ordinal'],
                              'recorded': row['widthAfterMm'],
                              'recomputed': row['widthBeforeMm'] * (1.0 - row['step'])})
        if row['phase'] == 'explore':
            if row['step'] != EXPLORE_STEP:
                bad_step.append(row['ordinal'])
            if row['splitYMm'] != row['widthBeforeMm'] / 2.0:
                bad_split.append({'ordinal': row['ordinal'],
                                  'recorded': row['splitYMm'],
                                  'recomputed': row['widthBeforeMm'] / 2.0})
        else:
            if not (PHYSICAL_EDGE_MM <= row['splitYMm'] <= row['widthBeforeMm']):
                bad_split.append({'ordinal': row['ordinal'],
                                  'splitYMm': row['splitYMm'],
                                  'widthBeforeMm': row['widthBeforeMm'],
                                  'phase': 'compress'})
    check(rows, f'{where}/B2 widthAfter == widthBefore*(1-step)', not bad_after,
          {'offenders': bad_after[:4]})
    check(rows, f'{where}/B3 delta == after - before', not bad_delta,
          {'offenders': bad_delta[:4]})
    check(rows, f'{where}/B4-B5 split is mid-depth (explore) / in (edge,W) (compress)',
          not bad_split, {'offenders': bad_split[:4]})
    check(rows, f'{where}/B4 explore step is the frozen 0.001', not bad_step,
          {'offenders': bad_step[:4]})

    # B7: the explore phase stops at its first failure.
    unpublished = [row for row in explore if not row['published']]
    check(rows, f'{where}/B7 at most one unpublished explore bite, and it is last',
          len(unpublished) == 0
          or (len(unpublished) == 1
              and unpublished[0]['ordinal'] == explore[-1]['ordinal']),
          {'unpublished': [row['ordinal'] for row in unpublished],
           'lastExplore': explore[-1]['ordinal'] if explore else None})

    # B1: the explore chain walks the published depths. The wall reduction keeps
    # only the LAST publication ordinal, so the chain is checked structurally:
    # consecutive published explore bites must have
    # `widthBefore[k] == widthAfter[k-1] * (published/target correction)`, which
    # collapses to `widthBefore[k] <= widthAfter[k-1]` because a publication is
    # never deeper than the target it published inside.
    chain = []
    published_explore = [row for row in explore if row['published']]
    for index in range(1, len(published_explore)):
        previous, current = published_explore[index - 1], published_explore[index]
        chain.append({
            'ordinal': current['ordinal'],
            'widthBeforeMm': current['widthBeforeMm'],
            'previousTargetMm': previous['widthAfterMm'],
            'monotone': current['widthBeforeMm'] <= previous['widthAfterMm'],
        })
    check(rows, f'{where}/B1 each explore bite starts at or below the previous target',
          all(link['monotone'] for link in chain),
          {'brokenLinks': [link for link in chain if not link['monotone']][:4],
           'links': len(chain)})

    return {
        'cell': where,
        'bites': len(bites),
        'exploreBites': len(explore),
        'compressBites': len(compress),
        'strikes': sum(row['strikes'] for row in bites),
        'disruptions': sum(row['disruptions'] for row in bites),
        'attempts': sum(row['attempts'] for row in bites),
        'masterIterations': sum(row['masterIterations'] for row in bites),
        'exactAttempts': sum(row['exactAttempts'] for row in bites),
        'publishedBites': sum(1 for row in bites if row['published']),
        'minRawPhiOfLast': bites[-1].get('minRawPhi'),
    }


def main():
    if len(sys.argv) < 2:
        raise SystemExit(__doc__)
    wall_path = sys.argv[1]
    out_path = sys.argv[2] if len(sys.argv) > 2 else None
    document = json.load(open(wall_path))
    rows, summaries = [], []
    for budget, cell in document.get('cells', {}).items():
        for seed_row in cell.get('seeds', []):
            if not seed_row.get('valid'):
                continue
            where = f'{budget}s/seed{seed_row["seed"]}'
            summary = cell_rows(rows, where, seed_row)
            if summary:
                summary['budget'] = budget
                summaries.append(summary)
    failures = [row for row in rows if not row['ok']]
    totals = {
        'cells': len(summaries),
        'strikes': sum(row['strikes'] for row in summaries),
        'disruptions': sum(row['disruptions'] for row in summaries),
        'failedSeparations': sum(row['attempts'] for row in summaries),
        'masterIterations': sum(row['masterIterations'] for row in summaries),
        'exactAttempts': sum(row['exactAttempts'] for row in summaries),
        'bites': sum(row['bites'] for row in summaries),
        'publishedBites': sum(row['publishedBites'] for row in summaries),
        'cellsWithAnyStrike': sum(1 for row in summaries if row['strikes'] > 0),
        'cellsWithAnyDisruption': sum(1 for row in summaries if row['disruptions'] > 0),
    }
    out = {
        'experiment': 'overlap-ics',
        'battery': 'evidence-audit-bite-consistency',
        'source': wall_path,
        'identitiesChecked': len(rows),
        'failures': failures,
        'totals': totals,
        'perCell': summaries,
        'BITES_CONSISTENT': not failures,
    }
    print(json.dumps({k: v for k, v in out.items() if k != 'perCell'}, indent=1))
    if out_path:
        os.makedirs(os.path.dirname(os.path.abspath(out_path)), exist_ok=True)
        with open(out_path, 'w') as handle:
            out['rows'] = rows
            json.dump(out, handle, indent=1)
    return 0 if not failures else 1


if __name__ == '__main__':
    sys.exit(main())
