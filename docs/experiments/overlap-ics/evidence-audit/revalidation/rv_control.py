#!/usr/bin/env python3
"""**The AB/BA control, recomputed** — README §3, §12 and caveat 9.

From the committed `cutclose-rerun/evidence/control-ab-ba.json`, and, where the
raw arm-A cell documents are still on the box, from those too.

  K1  §3's nine rows and its two medians, recomputed from the pair rows;
  K2  §3's `armBSpreadMm` 13.977 and `armBMedianDriftFromPublishedMm` 1.969;
  K3  §3's "beats the old wall arm on 3 of 9";
  K4  caveat 9's "six of nine arm-B cells returned exactly round 1's value",
      by diffing the two committed control documents;
  K5  §12's "169.21217 on seed 3 where the wall cell returned 167.31508" and
      "175.00538 on seed 4 where the wall cell returned 179.08123";
  K6  **arm A has no time filter at all** (`control.py`'s `arm_a` takes `min`
      over every strict publication). With the raw arm-A documents present this
      recomputes each arm-A number under the engine's own deadline bound
      (`budget - constructorSeconds`) and reports any that move.
"""
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ICS = os.path.abspath(os.path.join(HERE, '..', '..'))
RAW = os.environ.get('ICS_RAW', '/var/lib/t3/tmp/overlapics')
BAR_MM = 168.484
LIMIT = 10.000


def median(values):
    values = sorted(values)
    if not values:
        return None
    m = len(values) // 2
    return values[m] if len(values) % 2 else (values[m - 1] + values[m]) / 2.0


def main():
    with open(os.path.join(ICS,
                           'cutclose-rerun/evidence/control-ab-ba.json')) as h:
        rerun = json.load(h)
    with open(os.path.join(ICS,
                           'cutclose-round1/evidence/control-ab-ba.json')) as h:
        r1 = json.load(h)
    fails = []

    claims = [0]

    def want(tag, got, exp, tol=0.0):
        claims[0] += 1
        ok = got == exp if tol == 0.0 else abs(got - exp) <= tol
        if not ok:
            fails.append({'claim': tag, 'recomputed': got, 'readmeSays': exp})

    a = {p['seed']: p['A']['rawDepthMm'] for p in rerun['pairs']}
    b = {p['seed']: p['B']['rawDepthMm'] for p in rerun['pairs']}
    b1 = {p['seed']: p['B']['rawDepthMm'] for p in r1['pairs']}
    order = {p['seed']: p['order'] for p in rerun['pairs']}

    # K1
    printed = {0: (179.07609, 170.45273), 1: (179.08099, 165.65578),
               2: (167.91944, 174.28000), 3: (169.21217, 172.28409),
               4: (175.00538, 172.12900), 5: (179.07170, 179.63300),
               6: (169.08134, 168.46800), 7: (179.08210, 169.35992),
               8: (179.08211, 169.03159)}
    for seed, (pa, pb) in printed.items():
        want(f'K1 §3 armA seed{seed}', round(a[seed], 5), pa)
        want(f'K1 §3 armB seed{seed}', round(b[seed], 5), pb)
    want('K1 §3 armA median', round(median(a.values()), 5), 179.07170)
    want('K1 §3 armB median', round(median(b.values()), 5), 170.45273)

    # K2
    want('K2 armB spread', round(max(b.values()) - min(b.values()), 3),
         13.977)
    want('K2 armB median drift', round(abs(median(b.values()) - BAR_MM), 3),
         1.969)

    # K3
    wins = sorted(s for s in a if a[s] < b[s])
    want('K3 armA beats armB count', len(wins), 3)
    want('K3 armA beats armB seeds', wins, [2, 3, 5])

    # K4
    same = sorted(s for s in b if b[s] == b1[s])
    want('K4 armB cells identical to round 1', len(same), 6)
    want('K4 armB seeds that moved',
         sorted(s for s in b if b[s] != b1[s]), [0, 3, 7])
    want('K4 round1 armB seed0', round(b1[0], 5), 168.48360)
    want('K4 rerun armB seed0', round(b[0], 5), 170.45273)
    want('K4 armB seed6 both rounds', (round(b[6], 5), round(b1[6], 5)),
         (168.46800, 168.46800))

    # K5 - wall cell against control arm A
    with open(os.path.join(ICS, 'cutclose-rerun/evidence/wall.json')) as h:
        wall = {r['seed']: r['bestStrictChildMm']
                for r in json.load(h)['cells']['10']['seeds']}
    want('K5 seed3 armA', round(a[3], 5), 169.21217)
    want('K5 seed3 wall', round(wall[3], 5), 167.31508)
    want('K5 seed4 armA', round(a[4], 5), 175.00538)
    want('K5 seed4 wall', round(wall[4], 5), 179.08123)

    # K6 - arm A under the engine's own deadline bound
    k6 = []
    for seed in range(9):
        path = f'{RAW}/rerun/control/ctl-A-seed{seed}.json'
        if not os.path.exists(path):
            k6.append({'seed': seed, 'skipped': 'raw document absent'})
            continue
        with open(path) as handle:
            doc = json.load(handle)
        pubs = doc['outcome']['publications']
        cons = doc['wall']['constructorSeconds']
        bound = LIMIT - cons
        cf = doc['constructor'].get('placementFingerprint')
        strict = [p for p in pubs if p['placementFingerprint'] != cf]
        unfiltered = min(p['publishedRawDepthMm'] for p in strict)
        filtered = min((p['publishedRawDepthMm'] for p in strict
                        if p['wallSeconds'] <= bound), default=None)
        k6.append({
            'seed': seed,
            'committedArmAMm': a[seed],
            'unfilteredMm': unfiltered,
            'underEngineDeadlineMm': filtered,
            'publicationsPastEngineDeadline':
                sum(1 for p in pubs if p['wallSeconds'] > bound),
            'moves': unfiltered != filtered,
            'deltaMm': (None if filtered is None
                        else filtered - unfiltered),
            'qualifiesUnfiltered': unfiltered <= BAR_MM,
            'qualifiesFiltered': filtered is not None and filtered <= BAR_MM,
        })
        claims[0] += 1
        if abs(unfiltered - a[seed]) > 1e-12:
            fails.append({'claim': f'K6 armA seed{seed} matches committed',
                          'recomputed': unfiltered, 'readmeSays': a[seed]})

    doc = {
        'what': 'the AB/BA control, recomputed from committed evidence',
        'K1_armA': a, 'K1_armB': b, 'K1_order': order,
        'K1_armAMedian': median(a.values()),
        'K1_armBMedian': median(b.values()),
        'K2_armBSpreadMm': max(b.values()) - min(b.values()),
        'K3_armAWinsSeeds': wins,
        'K4_armBIdenticalToRound1Seeds': same,
        'K4_round1ArmB': b1,
        'K6_armAUnderEngineDeadline': k6,
        'K6_cellsThatMove': [r for r in k6 if r.get('moves')],
        'claimsChecked': claims[0],
        'failures': fails,
        'ALL_CONTROL_CLAIMS_REPRODUCE': not fails,
    }
    if len(sys.argv) > 1:
        with open(sys.argv[1], 'w') as handle:
            handle.write(json.dumps(doc, indent=1, sort_keys=True) + '\n')
    print('arm A median', doc['K1_armAMedian'], ' arm B median',
          doc['K1_armBMedian'], ' spread', round(doc['K2_armBSpreadMm'], 3))
    print('arm A wins on seeds', wins, ' arm B unchanged from round 1 on',
          same)
    print('round 1 arm B seed 0 =', b1[0], ' rerun arm B seed 0 =', b[0])
    print('arm A under the engine deadline:')
    for r in k6:
        if 'skipped' in r:
            print('  seed', r['seed'], r['skipped'])
            continue
        print(f"  seed{r['seed']} committed={r['committedArmAMm']} "
              f"filtered={r['underEngineDeadlineMm']} "
              f"late={r['publicationsPastEngineDeadline']} "
              f"moves={r['moves']}")
    print('claims checked:', claims[0])
    print('failures:', len(fails))
    for f in fails:
        print('  FAIL', f)
    return 0 if not fails else 1


if __name__ == '__main__':
    sys.exit(main())
