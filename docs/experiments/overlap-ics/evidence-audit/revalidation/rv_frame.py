#!/usr/bin/env python3
"""**Is the §0.1 time clause live on the committed round?** — decided, not bounded.

The code auditor's F1 says the anytime checkpoint filter compared a LOOP-relative
clock (`Pacer::elapsed_s`, started inside `Engine::run_cutclose`) against a
REQUEST-relative budget, so it could never exclude anything, and its F2 says the
committed reduction dropped every per-publication clock reading so the question
"is a late publication actually present in the committed round?" is undeterminable.

The raw cell documents the committed `wall.json` was reduced from are still on
this box at `$ICS_RAW` (default `/var/lib/t3/tmp/overlapics/rerun`). They carry
`publications[].wallSeconds`. So the question is decidable, and this decides it:
for every one of the 27 cells, recompute `bestStrictChildMm` three ways —

  * `old`   - the committed filter (`wallSeconds <= limit`), which never fires;
  * `lower` - `constructorSeconds + wallSeconds <= limit`, a LOWER bound on a
              publication's request-relative age, so the most permissive repair;
  * `upper` - `(totalSeconds - searchSeconds) + wallSeconds <= limit`, an UPPER
              bound, so the strictest repair a cell document can license.

If `old == lower == upper` on all 27 cells then no publication in the committed
round is late under any reading and the inoperative guard changed no number.
Any cell where they differ is a committed number that moves.

Usage: python3 rv_frame.py [out.json]
"""
import json
import os
import sys

RAW = os.environ.get('ICS_RAW', '/var/lib/t3/tmp/overlapics/rerun')
BAR_MM = 168.484
BUDGETS = {'3': 3.000, '10': 10.000, '30': 30.000}
SEEDS = range(9)


def cell(budget, limit, seed):
    with open(f'{RAW}/wall-{budget}s-seed{seed}.json') as handle:
        doc = json.load(handle)
    pubs = doc['outcome']['publications']
    wall = doc['wall']
    cons_fp = doc['constructor'].get('placementFingerprint')
    lower_off = wall['constructorSeconds']
    upper_off = wall['totalSeconds'] - wall['searchSeconds']

    def best(offset):
        cand = [row['publishedRawDepthMm'] for row in pubs
                if row['placementFingerprint'] != cons_fp
                and (offset is None or offset + row['wallSeconds'] <= limit)]
        return min(cand) if cand else None

    loops = [row['wallSeconds'] for row in pubs]
    return {
        'budget': budget, 'seed': seed,
        'publications': len(pubs),
        'strict': sum(1 for r in pubs
                      if r['placementFingerprint'] != cons_fp),
        'bestOld': best(None),
        'bestLower': best(lower_off),
        'bestUpper': best(upper_off),
        'constructorSeconds': lower_off,
        'outsideLoopSeconds': upper_off,
        'maxLoopSeconds': max(loops) if loops else None,
        'maxRequestSecondsLower': (lower_off + max(loops)) if loops else None,
        'maxRequestSecondsUpper': (upper_off + max(loops)) if loops else None,
        'lateUnderLower': sum(1 for r in pubs
                              if lower_off + r['wallSeconds'] > limit),
        'lateUnderUpper': sum(1 for r in pubs
                              if upper_off + r['wallSeconds'] > limit),
        # How much room the old filter had left: limit - max loop reading.
        'oldFilterHeadroomSeconds': (limit - max(loops)) if loops else None,
    }


def main():
    rows = [cell(b, lim, s) for b, lim in BUDGETS.items() for s in SEEDS]
    moved = [r for r in rows
             if not (r['bestOld'] == r['bestLower'] == r['bestUpper'])]
    late = [r for r in rows if r['lateUnderUpper'] or r['lateUnderLower']]
    qual = {}
    for key in ('bestOld', 'bestLower', 'bestUpper'):
        qual[key] = sorted(r['seed'] for r in rows
                           if r['budget'] == '10' and r[key] is not None
                           and r[key] <= BAR_MM)
    doc = {
        'what': 'the §0.1 anytime clause, decided on the committed round',
        'raw': RAW,
        'cells': rows,
        'cellsWhereBestStrictChildMoves': moved,
        'cellsWithAnyLatePublication': [
            {k: r[k] for k in ('budget', 'seed', 'lateUnderLower',
                               'lateUnderUpper', 'maxRequestSecondsLower',
                               'maxRequestSecondsUpper')} for r in late],
        'gateQualifyingSeedsByFilter': qual,
        'minOldFilterHeadroomSeconds': min(r['oldFilterHeadroomSeconds']
                                           for r in rows),
        'maxRequestRelativeAgeAgainstBudget': max(
            r['maxRequestSecondsUpper'] - BUDGETS[r['budget']] for r in rows),
        'NO_COMMITTED_NUMBER_MOVES': not moved,
        'GATE_VERDICT_STABLE_UNDER_ALL_THREE_FILTERS':
            qual['bestOld'] == qual['bestLower'] == qual['bestUpper'],
    }
    out = sys.argv[1] if len(sys.argv) > 1 else None
    text = json.dumps(doc, indent=1, sort_keys=True)
    if out:
        with open(out, 'w') as handle:
            handle.write(text + '\n')
    for row in rows:
        mark = '  MOVES' if row in moved else ''
        print(f"{row['budget']:>2}s seed{row['seed']}  old={row['bestOld']} "
              f"lower={row['bestLower']} upper={row['bestUpper']} "
              f"late(low/up)={row['lateUnderLower']}/{row['lateUnderUpper']} "
              f"maxReqUp={row['maxRequestSecondsUpper']:.6f}{mark}")
    print('gate qualifying seeds:', qual)
    print('min old-filter headroom (s):', doc['minOldFilterHeadroomSeconds'])
    print('max request-relative age minus budget (s):',
          doc['maxRequestRelativeAgeAgainstBudget'])
    print('NO_COMMITTED_NUMBER_MOVES:', doc['NO_COMMITTED_NUMBER_MOVES'])
    return 0 if doc['NO_COMMITTED_NUMBER_MOVES'] else 1


if __name__ == '__main__':
    sys.exit(main())
