#!/usr/bin/env python3
"""**Round 1 against the rerun, recomputed from both committed documents.**

Sources, all committed:

  * `cutclose-round1/evidence/wall.json`      - round 1's reduction;
  * `cutclose-rerun/evidence/wall.json`       - the rerun's;
  * `cutclose-rerun/evidence/round1-bites-red.json` - round 1's raw bite rows,
    copied verbatim out of its raw cell documents and pinned by `sourceSha256`.

Claims recomputed:

  C1  the quorum move: round 1 0 of 9 at 10 s, the rerun 2 of 9, both recomputed
      from `bestStrictChildMm` against 168.484 rather than read off `verdict`;
  C2  §2's second table - the 18 round-1-vs-rerun deltas, to the printed 3 dp;
  C3  §12's "5 -> 7" sub-bar count at 30 s;
  C4  §1's "Round 1's best was 169.00246";
  C5  **the shared prefix.** The repair is one predicate inside `observe_raw`.
      A bite that never reaches `iterations_without_improvement` cannot see it,
      so the two rounds' bite rows should agree from bite 1 until the first bite
      whose separation went long enough to consult the counter - and then
      diverge. This measures that prefix per cell rather than asserting it, and
      reports where the first divergence is and in which field.
  C6  the `everyPublicationDualValid` / `invalidPublications` clause in both
      rounds, and the publication and bite totals of each.

Exit 0 iff C1-C4 and C6 match the README; C5 is a measurement and prints.
"""
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ICS = os.path.abspath(os.path.join(HERE, '..', '..'))
BAR_MM = 168.484
BUDGETS = ('3', '10', '30')
# The bite-row fields that describe the trajectory. `minRawPhi` is a float and
# is compared by value; everything else is an integer or a bool.
BITE_FIELDS = ('ordinal', 'phase', 'widthBeforeMm', 'widthAfterMm', 'deltaMm',
               'splitYMm', 'movedPieces', 'step', 'attempts', 'disruptions',
               'masterIterations', 'strikes', 'minRawPhi', 'proxyBandReached',
               'exactAttempts', 'published')


def load(path):
    with open(os.path.join(ICS, path)) as handle:
        return json.load(handle)


def main():
    r1 = load('cutclose-round1/evidence/wall.json')
    re_ = load('cutclose-rerun/evidence/wall.json')
    r1b = load('cutclose-rerun/evidence/round1-bites-red.json')
    fails = []

    claims = [0]

    def want(tag, got, exp):
        claims[0] += 1
        if got != exp:
            fails.append({'claim': tag, 'recomputed': got, 'readmeSays': exp})

    def best(doc, budget):
        return {row['seed']: row['bestStrictChildMm']
                for row in doc['cells'][budget]['seeds']}

    # C1 / C3
    counts = {}
    for budget in BUDGETS:
        for tag, doc in (('round1', r1), ('rerun', re_)):
            b = best(doc, budget)
            counts[(tag, budget)] = sorted(
                s for s, v in b.items() if v is not None and v <= BAR_MM)
    want('C1 round1 quorum at 10 s', len(counts[('round1', '10')]), 0)
    want('C1 rerun quorum at 10 s', len(counts[('rerun', '10')]), 2)
    want('C1 rerun qualifying seeds', counts[('rerun', '10')], [2, 3])
    want('C3 round1 sub-bar at 30 s', len(counts[('round1', '30')]), 5)
    want('C3 rerun sub-bar at 30 s', len(counts[('rerun', '30')]), 7)

    # C4
    want('C4 round1 best at 10 s',
         round(min(best(r1, '10').values()), 5), 169.00246)
    want('C4 rerun best at 10 s',
         round(min(best(re_, '10').values()), 5), 167.31508)
    want('C4 rerun best at 30 s',
         round(min(best(re_, '30').values()), 5), 161.05499)
    want('C4 round1 best at 30 s',
         round(min(best(r1, '30').values()), 5), 163.69242)

    # C2 - the delta table, as printed
    printed = {
        '10': {0: (179.07686, 179.07609, -0.001), 1: (179.08099, 179.08099, 0.000),
               2: (179.07957, 167.95169, -11.128), 3: (169.21860, 167.31508, -1.904),
               4: (179.08123, 179.08123, 0.000), 5: (179.07170, 179.07170, 0.000),
               6: (169.00246, 169.17186, 0.169), 7: (179.08211, 179.08210, -0.000),
               8: (179.08211, 179.08210, -0.000)},
        '30': {0: (164.00236, 161.05499, -2.947), 1: (179.08099, 165.00578, -14.075),
               2: (168.66303, 163.56062, -5.102), 3: (164.00577, 164.00461, -0.001),
               4: (165.05518, 164.00094, -1.054), 5: (163.69242, 162.40477, -1.288),
               6: (164.00972, 164.00930, -0.000), 7: (179.08210, 179.08210, 0.000),
               8: (179.08210, 179.06000, -0.022)}}
    c2 = {}
    for budget in ('10', '30'):
        b1, b2 = best(r1, budget), best(re_, budget)
        c2[budget] = {}
        for seed in range(9):
            got = (round(b1[seed], 5), round(b2[seed], 5),
                   round(b2[seed] - b1[seed], 3))
            c2[budget][seed] = got
            claims[0] += 1
            exp = printed[budget][seed]
            # `-0.000` and `+0.000` print differently and compare equal at 0.0
            if (got[0], got[1]) != (exp[0], exp[1]) or \
               abs(got[2] - exp[2]) > 5e-4:
                fails.append({'claim': f'C2 §2 delta {budget}s seed{seed}',
                              'recomputed': got, 'readmeSays': exp})

    # C5 - the shared prefix
    r1_bites = {}
    for key, cell in r1b['cells'].items():
        r1_bites[(key.split('s-')[0], cell['seed'])] = cell['bites']
    prefixes = []
    for budget in BUDGETS:
        for row in re_['cells'][budget]['seeds']:
            seed = row['seed']
            a = r1_bites[(budget, seed)]
            b = row['bites']
            n = 0
            first = None
            for i in range(min(len(a), len(b))):
                diff = [f for f in BITE_FIELDS if a[i].get(f) != b[i].get(f)]
                if diff:
                    first = {'index': i, 'ordinal': a[i]['ordinal'],
                             'phase': a[i]['phase'], 'fields': diff,
                             'round1': {f: a[i].get(f) for f in diff},
                             'rerun': {f: b[i].get(f) for f in diff}}
                    break
                n += 1
            prefixes.append({'cell': f'{budget}s-seed{seed}',
                             'round1Bites': len(a), 'rerunBites': len(b),
                             'identicalPrefix': n, 'firstDivergence': first})

    # C6
    def totals(doc):
        return {
            'publications': sum(r['publicationsTotal']
                                for b in BUDGETS for r in doc['cells'][b]['seeds']),
            'invalid': sum(r['invalidPublications']
                           for b in BUDGETS for r in doc['cells'][b]['seeds']),
            'everyPublicationDualValid':
                doc['verdict']['everyPublicationDualValid'],
            'quorumReached': doc['verdict']['quorumReached'],
        }
    t1, t2 = totals(r1), totals(re_)
    want('C6 rerun publications', t2['publications'], 1701)
    want('C6 rerun invalid', t2['invalid'], 0)
    want('C6 round1 invalid', t1['invalid'], 0)

    doc = {
        'what': 'round 1 against the rerun, from both committed reductions',
        'C1_qualifyingSeedsByRoundAndBudget':
            {f'{t}-{b}': v for (t, b), v in counts.items()},
        'C2_deltaTable': c2,
        'C4_bests': {'round1_10s': min(best(r1, '10').values()),
                     'rerun_10s': min(best(re_, '10').values()),
                     'round1_30s': min(best(r1, '30').values()),
                     'rerun_30s': min(best(re_, '30').values())},
        'C5_sharedPrefix': prefixes,
        'C5_summary': {
            'cellsWithIdenticalPrefixAtLeast21':
                sum(1 for p in prefixes if p['identicalPrefix'] >= 21),
            'cellsWholeTrajectoryIdentical':
                sum(1 for p in prefixes if p['firstDivergence'] is None
                    and p['round1Bites'] == p['rerunBites']),
            'minPrefix': min(p['identicalPrefix'] for p in prefixes),
            'prefixHistogram': sorted(
                {p['identicalPrefix'] for p in prefixes}),
        },
        'C6_totals': {'round1': t1, 'rerun': t2},
        'claimsChecked': claims[0],
        'prefixCellsMeasured': len(prefixes),
        'failures': fails,
        'ALL_CROSS_ROUND_CLAIMS_REPRODUCE': not fails,
    }
    if len(sys.argv) > 1:
        with open(sys.argv[1], 'w') as handle:
            handle.write(json.dumps(doc, indent=1, sort_keys=True) + '\n')
    print('qualifying:', {f'{t}-{b}': v for (t, b), v in counts.items()})
    print('bests:', doc['C4_bests'])
    print('prefix summary:', doc['C5_summary'])
    for p in prefixes:
        fd = p['firstDivergence']
        print(f"  {p['cell']:>12}  r1={p['round1Bites']:>3} re={p['rerunBites']:>3} "
              f"prefix={p['identicalPrefix']:>3} "
              + ('identical' if fd is None
                 else f"first diff at ordinal {fd['ordinal']} ({fd['phase']}): "
                      + ','.join(fd['fields'])))
    print('claims checked:', claims[0], ' prefix cells:', len(prefixes))
    print('failures:', len(fails))
    for f in fails:
        print('  FAIL', f)
    return 0 if not fails else 1


if __name__ == '__main__':
    sys.exit(main())
