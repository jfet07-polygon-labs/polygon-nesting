#!/usr/bin/env python3
"""Does the continuous-rotation tax change sign under the round authority?

    crotflip.py REACHWORK.json REACHWALL.json OUT.json

`continuous-rotation`'s README measures the blanket operator at **+3.721 mm**
(worse) at ten seconds on mixed-61, 0 of 9 paired rounds better, under the miter
authority. Grok review 7 §3 names reachability as a co-requirement of any
150@10s claim and Sol review 12 §3.3 endorsed asking it with the existing tools
rather than a new family. So the question is exactly one paired difference,
computed twice:

    tax_miter = crot(miter) - base(miter)
    tax_round = crot(round) - base(round)

and the answer is whether `tax_round` is negative where `tax_miter` is positive.
A **diagnostic**: nothing here promotes anything, and the answer "it does not
flip" is as useful as the answer "it does".

The off-lattice census is reported beside it, because a round-armed publication
that uses no off-2.5-degree pose has not reached anything the miter authority
could not have reached, and that would make the tax question moot rather than
answered.
"""
import json
import statistics
import sys

BASE, CROT, REK, REKCROT = 'base', 'crot', 'rek', 'rekcrot'
MITER_TAX_AT_10S_MM = 3.721


def paired(rows, left, right):
    """`left - right`, paired on (budget, seed, round)."""
    index = {}
    for row in rows:
        index.setdefault((row['budget'], row['seed'], row['round']),
                         {})[row['arm']] = row
    out = []
    for key, arms in sorted(index.items()):
        a, b = arms.get(left), arms.get(right)
        if not a or not b:
            continue
        if a['rawDepthMm'] is None or b['rawDepthMm'] is None:
            continue
        out.append({'budget': key[0], 'seed': key[1], 'round': key[2],
                    'leftMm': a['rawDepthMm'], 'rightMm': b['rawDepthMm'],
                    'deltaMm': a['rawDepthMm'] - b['rawDepthMm'],
                    'leftOffLattice2p5': a['offLattice2p5Count'],
                    'rightOffLattice2p5': b['offLattice2p5Count'],
                    'leftDualGateValid': a['dualGateValid'],
                    'rightDualGateValid': b['dualGateValid']})
    return out


def stat(rows):
    deltas = [r['deltaMm'] for r in rows]
    if not deltas:
        return None
    return {'n': len(deltas), 'medianMm': statistics.median(deltas),
            'betterCount': sum(1 for d in deltas if d < 0),
            'worseCount': sum(1 for d in deltas if d > 0),
            'range': [min(deltas), max(deltas)]}


def main():
    out_path = sys.argv[-1]
    rows = []
    sources = []
    for path in sys.argv[1:-1]:
        document = json.load(open(path))
        rows.extend(document['rows'])
        sources.append(path)
    budgets = sorted({row['budget'] for row in rows})
    result = {'sources': sources, 'budgets': budgets,
              'miterTaxAt10sFromContinuousRotationReadmeMm':
                  MITER_TAX_AT_10S_MM, 'perBudget': {}}
    for budget in budgets:
        block = [row for row in rows if row['budget'] == budget]
        miter_tax = paired(block, CROT, BASE)
        round_tax = paired(block, REKCROT, REK)
        authority = paired(block, REK, BASE)
        result['perBudget'][budget] = {
            'crotTaxUnderMiter': {'stats': stat(miter_tax), 'rows': miter_tax},
            'crotTaxUnderRound': {'stats': stat(round_tax), 'rows': round_tax},
            'roundAuthorityAtCrot0': {'stats': stat(authority),
                                      'rows': authority},
            'offLattice2p5': {
                arm: sorted({r['offLattice2p5Count'] for r in block
                             if r['arm'] == arm})
                for arm in (BASE, CROT, REK, REKCROT)},
            'distinctRotations': {
                arm: sorted({r['distinctRotations'] for r in block
                             if r['arm'] == arm})
                for arm in (BASE, CROT, REK, REKCROT)},
            'allDualGateValid': all(r['dualGateValid'] for r in block
                                    if r['rawDepthMm'] is not None),
        }
        miter_stat = result['perBudget'][budget]['crotTaxUnderMiter']['stats']
        round_stat = result['perBudget'][budget]['crotTaxUnderRound']['stats']
        result['perBudget'][budget]['FLIPPED'] = bool(
            miter_stat and round_stat
            and miter_stat['medianMm'] > 0 and round_stat['medianMm'] < 0)
    result['ANSWER'] = {
        budget: {
            'miterTaxMedianMm': (block['crotTaxUnderMiter']['stats'] or {})
            .get('medianMm'),
            'roundTaxMedianMm': (block['crotTaxUnderRound']['stats'] or {})
            .get('medianMm'),
            'flipped': block['FLIPPED'],
            'roundArmedPublicationsUsingOffLatticePoses':
                block['offLattice2p5'][REK],
        }
        for budget, block in result['perBudget'].items()}
    json.dump(result, open(out_path, 'w'), indent=1)
    print(json.dumps(result['ANSWER'], indent=1))


if __name__ == '__main__':
    main()
