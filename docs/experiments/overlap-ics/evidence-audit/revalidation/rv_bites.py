#!/usr/bin/env python3
"""**The rerun README's per-bite claims, recomputed from the committed raw rows.**

Written from the README's own definitions and the raw row schema; `bites.py` is
deliberately not imported and not read. Sources are the two committed
documents only - `cutclose-rerun/evidence/wall.json` (rerun, raw `bites` arrays)
and `cutclose-rerun/evidence/round1-bites-red.json` (round 1, same arrays) - so
this is a check on committed evidence, not on the box.

Claims recomputed:

  B1  §2's `bites` column - "explore bites published", per cell;
  B2  §5's closing line - 145 strikes / 164 disruptions on 1,825 bites for the
      rerun, 88 / 122 on 1,391 for round 1;
  B3  §5's green vector - seed 1, 30 s, the 22nd bite, both rounds;
  B4  §5's nine-seed 30 s bite-22 table;
  B5  §6's nine-seed 10 s bite-22 table;
  B6  §9's funnel at 10 s - bitesStarted/proxyBandReached/exactAttempted/
      dualValidPublished, summed over the nine seeds, and the round-1 numerator;
  B7  §9's overclaim arithmetic - seed 2's per-bite `exactAttempts` sum against
      its funnel row;
  B8  the funnel row of every cell, recomputed from that cell's raw bite rows.

Exit 0 iff every recomputation matches the README's printed number.
"""
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
EV = os.path.abspath(os.path.join(HERE, '..', '..', 'cutclose-rerun',
                                  'evidence'))
BUDGETS = ('3', '10', '30')


def rerun_cells():
    with open(os.path.join(EV, 'wall.json')) as handle:
        doc = json.load(handle)
    out = {}
    for budget in BUDGETS:
        for row in doc['cells'][budget]['seeds']:
            out[(budget, row['seed'])] = row
    return doc, out


def round1_cells():
    with open(os.path.join(EV, 'round1-bites-red.json')) as handle:
        doc = json.load(handle)
    out = {}
    for key, cell in doc['cells'].items():
        budget = key.split('s-')[0]
        out[(budget, cell['seed'])] = cell
    return doc, out


def bite22(cell):
    """The 22nd bite. `bites[21]`, in ordinal order; ordinals are 1-based."""
    rows = [r for r in cell['bites'] if r['ordinal'] == 22]
    if len(rows) != 1:
        return None
    r = rows[0]
    return {'masterIterations': r['masterIterations'], 'strikes': r['strikes'],
            'disruptions': r['disruptions'], 'attempts': r['attempts'],
            'published': r['published'], 'minRawPhi': r['minRawPhi'],
            'phase': r['phase']}


def funnel_of(cell):
    rows = cell['bites']
    return {'bitesStarted': len(rows),
            'proxyBandReached': sum(1 for r in rows if r['proxyBandReached']),
            'exactAttempted': sum(1 for r in rows if r['exactAttempts'] > 0),
            'dualValidPublished': sum(1 for r in rows if r['published'])}


def main():
    wall, rerun = rerun_cells()
    r1doc, r1 = round1_cells()
    fails = []

    def want(tag, got, expected):
        if got != expected:
            fails.append({'claim': tag, 'recomputed': got,
                          'readmeSays': expected})
        return got

    # B1
    b1 = {}
    for (budget, seed), cell in sorted(rerun.items()):
        b1[f'{budget}s-seed{seed}'] = sum(
            1 for r in cell['bites']
            if r['phase'] == 'explore' and r['published'])
    readme_bites = {  # §2, the `bites` columns, transcribed from the table
        '3': [21, 19, 21, 21, 21, 21, 21, 21, 21],
        '10': [21, 21, 63, 72, 21, 21, 74, 21, 21],
        '30': [120, 76, 111, 109, 109, 117, 105, 21, 21]}
    for budget in BUDGETS:
        for seed in range(9):
            want(f'B1 §2 bites {budget}s seed{seed}',
                 b1[f'{budget}s-seed{seed}'], readme_bites[budget][seed])

    # B2
    def totals(cells):
        rows = [r for c in cells.values() for r in c['bites']]
        return {'bites': len(rows),
                'strikes': sum(r['strikes'] for r in rows),
                'disruptions': sum(r['disruptions'] for r in rows)}
    t_re, t_r1 = totals(rerun), totals(r1)
    want('B2 rerun strikes', t_re['strikes'], 145)
    want('B2 rerun disruptions', t_re['disruptions'], 164)
    want('B2 rerun bites', t_re['bites'], 1825)
    want('B2 round1 strikes', t_r1['strikes'], 88)
    want('B2 round1 disruptions', t_r1['disruptions'], 122)
    want('B2 round1 bites', t_r1['bites'], 1391)

    # B3 / B4 - the 30 s bite-22 table
    readme_30 = {  # seed: (iters, strikes, disr, attempts, published)
        0: ((2061, 2, 0, 0, True), (1424, 2, 0, 0, True)),
        1: ((5319, 0, 0, 1, False), (3059, 6, 2, 2, True)),
        2: ((7450, 3, 1, 1, True), (1283, 3, 1, 1, True)),
        3: ((137, 0, 0, 0, True), (137, 0, 0, 0, True)),
        4: ((3622, 6, 2, 2, True), (2032, 6, 2, 2, True)),
        5: ((1700, 2, 0, 0, True), (1142, 0, 0, 0, True)),
        6: ((131, 0, 0, 0, True), (131, 0, 0, 0, True)),
        7: ((3906, 0, 0, 1, False), (5638, 17, 5, 6, False)),
        8: ((3825, 4, 1, 2, False), (6483, 15, 5, 6, False))}
    b4 = {}
    for seed, (want_r1, want_re) in readme_30.items():
        got_r1 = bite22(r1[('30', seed)])
        got_re = bite22(rerun[('30', seed)])
        b4[seed] = {'round1': got_r1, 'rerun': got_re}
        for label, got, exp in (('round1', got_r1, want_r1),
                                ('rerun', got_re, want_re)):
            tup = (got['masterIterations'], got['strikes'],
                   got['disruptions'], got['attempts'], got['published'])
            want(f'B4 §5 30s bite22 seed{seed} {label}', tup, exp)

    # B5 - the 10 s bite-22 table
    readme_10 = {
        0: ((1290, 1, 0, False), (1408, 2, 0, False)),
        1: ((797, 0, 0, False), (809, 0, 0, False)),
        2: ((1754, 0, 0, False), (1283, 3, 1, True)),
        3: ((137, 0, 0, True), (137, 0, 0, True)),
        4: ((1072, 1, 0, False), (1125, 2, 0, False)),
        5: ((855, 0, 0, False), (854, 0, 0, False)),
        6: ((131, 0, 0, True), (131, 0, 0, True)),
        7: ((892, 0, 0, False), (893, 2, 0, False)),
        8: ((922, 1, 0, False), (925, 2, 0, False))}
    b5 = {}
    for seed, (want_r1, want_re) in readme_10.items():
        got_r1 = bite22(r1[('10', seed)])
        got_re = bite22(rerun[('10', seed)])
        b5[seed] = {'round1': got_r1, 'rerun': got_re}
        for label, got, exp in (('round1', got_r1, want_r1),
                                ('rerun', got_re, want_re)):
            tup = (got['masterIterations'], got['strikes'],
                   got['disruptions'], got['published'])
            want(f'B5 §6 10s bite22 seed{seed} {label}', tup, exp)

    # B6 - the funnel at 10 s
    gate = [rerun[('10', s)] for s in range(9)]
    summed = {k: sum(funnel_of(c)[k] for c in gate) for k in
              ('bitesStarted', 'proxyBandReached', 'exactAttempted',
               'dualValidPublished')}
    want('B6 §9 bitesStarted', summed['bitesStarted'], 607)
    want('B6 §9 proxyBandReached', summed['proxyBandReached'], 601)
    want('B6 §9 exactAttempted', summed['exactAttempted'], 601)
    want('B6 §9 dualValidPublished', summed['dualValidPublished'], 584)
    r1_gate_bites = sum(len(r1[('10', s)]['bites']) for s in range(9))
    want('B6 §9 round1 bitesStarted at 10 s', r1_gate_bites, 350)

    # B7 - the overclaim
    seed2 = rerun[('10', 2)]
    attempts_sum = sum(r['exactAttempts'] for r in seed2['bites'])
    want('B7 §9 seed2 exactAttempts sum', attempts_sum, 1313)
    want('B7 §9 seed2 funnel exactAttempted',
         seed2['funnel']['exactAttempted'], 174)

    # B8 - every cell's funnel, from its own rows
    b8 = []
    for (budget, seed), cell in sorted(rerun.items()):
        got, emitted = funnel_of(cell), cell['funnel']
        b8.append({'cell': f'{budget}s-seed{seed}', 'recomputed': got,
                   'emitted': emitted})
        if got != emitted:
            fails.append({'claim': f'B8 funnel {budget}s-seed{seed}',
                          'recomputed': got, 'readmeSays': emitted})

    doc = {
        'what': "the rerun README's per-bite claims, recomputed from the "
                'committed raw bite rows by an independent reduction',
        'sources': [os.path.join(EV, 'wall.json'),
                    os.path.join(EV, 'round1-bites-red.json')],
        'B1_exploreBitesPublished': b1,
        'B2_totals': {'rerun': t_re, 'round1': t_r1},
        'B4_thirtySecondBite22': b4,
        'B5_tenSecondBite22': b5,
        'B6_gateFunnel': summed,
        'B6_round1GateBites': r1_gate_bites,
        'B7_seed2': {'perBiteExactAttemptsSum': attempts_sum,
                     'funnelExactAttempted':
                         seed2['funnel']['exactAttempted'],
                     'ratio': attempts_sum
                     / seed2['funnel']['exactAttempted']},
        'B8_funnelPerCell': b8,
        'failures': fails,
        'ALL_README_BITE_CLAIMS_REPRODUCE': not fails,
    }
    if len(sys.argv) > 1:
        with open(sys.argv[1], 'w') as handle:
            handle.write(json.dumps(doc, indent=1, sort_keys=True) + '\n')
    print('rerun totals :', t_re)
    print('round1 totals:', t_r1)
    print('gate funnel  :', summed, ' round1 bitesStarted:', r1_gate_bites)
    print('green vector : seed1 30s bite22 rerun =', b4[1]['rerun'])
    print('               seed1 30s bite22 round1 =', b4[1]['round1'])
    print('seed2 overclaim: perBite sum', attempts_sum, 'vs funnel',
          seed2['funnel']['exactAttempted'])
    print('failures:', len(fails))
    for f in fails[:40]:
        print('  FAIL', f)
    return 0 if not fails else 1


if __name__ == '__main__':
    sys.exit(main())
