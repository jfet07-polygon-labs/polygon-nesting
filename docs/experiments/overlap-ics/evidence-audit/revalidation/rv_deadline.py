#!/usr/bin/env python3
"""**The overrun, in the engine's own frame** — no offset model required.

The offset-bound argument in `rv_frame.py` needs a claim about where the
request clock starts. This one does not. The example hands the loop

    Budget::Wall { remaining_seconds: wall_budget_s - started.elapsed() }

(`overlap_ics_benchmark.rs`, the `cutclose` arm), and `started` is taken at the
top of the run, strictly **before** `constructor_started`. So the loop's own
deadline satisfies

    total_s = budget - started.elapsed()@budget  <  budget - constructorSeconds

and any publication whose `wallSeconds` exceeds `budget - constructorSeconds`
is past the deadline the engine itself was given - by the engine's own clock,
with no assumption about what happened before the loop started.

Reported per cell:

  * `loopDeadlineUpperBoundSeconds` = budget - constructorSeconds;
  * publications whose `wallSeconds` exceeds it, i.e. published after the
    engine's own deadline;
  * `loopOverrunMs` = (constructorSeconds + loopSearchSeconds) - budget, the
    loop-exit overrun README caveat 7 reports, recomputed;
  * `publicationOverrunMs`, the same thing for the latest publication.
"""
import json
import os
import sys

RAW = os.environ.get('ICS_RAW', '/var/lib/t3/tmp/overlapics/rerun')
BUDGETS = {'3': 3.000, '10': 10.000, '30': 30.000}


def main():
    rows = []
    for budget, limit in BUDGETS.items():
        for seed in range(9):
            with open(f'{RAW}/wall-{budget}s-seed{seed}.json') as handle:
                doc = json.load(handle)
            wall = doc['wall']
            pubs = doc['outcome']['publications']
            cons = wall['constructorSeconds']
            bound = limit - cons
            late = [p for p in pubs if p['wallSeconds'] > bound]
            cf = doc['constructor'].get('placementFingerprint')
            strict = [p for p in pubs if p['placementFingerprint'] != cf]
            best = min(p['publishedRawDepthMm'] for p in strict)
            best_rows = [p for p in strict
                         if p['publishedRawDepthMm'] == best]
            rows.append({
                'cell': f'{budget}s-seed{seed}',
                'budgetSeconds': limit,
                'constructorSeconds': cons,
                'loopDeadlineUpperBoundSeconds': bound,
                'maxPublicationWallSeconds': max(p['wallSeconds']
                                                 for p in pubs),
                'publicationsPastEngineDeadline': len(late),
                'publicationOverrunMs': max(
                    (p['wallSeconds'] - bound) * 1000.0 for p in pubs),
                'loopOverrunMs':
                    (cons + wall['loopSearchSeconds'] - limit) * 1000.0,
                'processOverrunMs': (wall['totalSeconds'] - limit) * 1000.0,
                'bestStrictChildMm': best,
                'bestStrictChildPastEngineDeadline':
                    any(p['wallSeconds'] > bound for p in best_rows),
                'bestStrictChildOverrunMs':
                    max((p['wallSeconds'] - bound) * 1000.0
                        for p in best_rows),
            })
    affected = [r for r in rows if r['bestStrictChildPastEngineDeadline']]
    doc = {
        'what': "publications past the engine's own deadline, no offset model",
        'raw': RAW,
        'cells': rows,
        'cellsWhoseReportedBestIsPastTheEngineDeadline': affected,
        'maxLoopOverrunMs': max(r['loopOverrunMs'] for r in rows),
        'maxLoopOverrunCell': max(rows, key=lambda r: r['loopOverrunMs'])['cell'],
        'maxProcessOverrunMs': max(r['processOverrunMs'] for r in rows),
        'cellsWithAnyLatePublication':
            [r['cell'] for r in rows if r['publicationsPastEngineDeadline']],
    }
    if len(sys.argv) > 1:
        with open(sys.argv[1], 'w') as handle:
            handle.write(json.dumps(doc, indent=1, sort_keys=True) + '\n')
    for r in rows:
        mark = '  <<< reported best is late' \
            if r['bestStrictChildPastEngineDeadline'] else ''
        print(f"{r['cell']:>12} bound={r['loopDeadlineUpperBoundSeconds']:.6f} "
              f"maxPub={r['maxPublicationWallSeconds']:.6f} "
              f"late={r['publicationsPastEngineDeadline']} "
              f"pubOverrun={r['publicationOverrunMs']:+.3f} ms "
              f"loopOverrun={r['loopOverrunMs']:+.3f} ms{mark}")
    print('max loop overrun:', doc['maxLoopOverrunMs'], 'ms on',
          doc['maxLoopOverrunCell'])
    print('cells with any late publication:',
          doc['cellsWithAnyLatePublication'])
    return 0


if __name__ == '__main__':
    sys.exit(main())
