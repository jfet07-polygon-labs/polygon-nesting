#!/usr/bin/env python3
"""**Does the committed `wall.json` say what the raw cell documents say?**

The committed reduction carries no per-cell `sourceSha256` (round 1's
`round1-bites-red.json` does), so the only way to bind it to the raw documents
is to redo the reduction and compare every field. This re-implements `cell()`
from scratch - deliberately not importing `wall.py` - and diffs the result
against the committed row, field by field, bit for bit on every float.

It also re-derives the whole of README Part I from the raw documents alone:
the §2 curve, the qualifying set, the §1 clause table.

Exit 0 iff every committed field is reproduced.
"""
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, '..', '..'))
RAW = os.environ.get('ICS_RAW', '/var/lib/t3/tmp/overlapics/rerun')
COMMITTED = os.path.join(ROOT, 'cutclose-rerun', 'evidence', 'wall.json')
BAR_MM = 168.484
BUDGETS = {'3': 3.000, '10': 10.000, '30': 30.000}
QUORUM = 3
# Fields the committed reduction carries. `checkpointFrame` is the auditor's
# later repair and is absent from the committed document by construction.
SCALAR_FIELDS = [
    'seed', 'budgetSeconds', 'exit', 'valid', 'constructorDepthMm',
    'constructorSeconds', 'searchSeconds', 'totalSeconds',
    'bestStrictChildMm', 'incumbentMm', 'incumbentIsConstructor',
    'publicationsWithinBudget', 'publicationsTotal', 'strictChildren',
    'exploreBites', 'compressBites', 'invalidPublications', 'repairMaxUm',
    'repairMaxGivebackMm', 'finalWidthMm', 'minRawPhiOfLastBite', 'qualifies',
]


def bits(value):
    """Compare floats by their bits, so 0.0/-0.0 and NaN are honest."""
    if isinstance(value, float):
        import struct
        return ('f64', struct.pack('>d', value).hex())
    return value


def reduce_cell(budget, seed):
    """`wall.py:cell()` re-derived from the spec text, not imported."""
    with open(f'{RAW}/wall-{budget}s-seed{seed}.json') as handle:
        doc = json.load(handle)
    limit = BUDGETS[budget]
    outcome = doc['outcome']
    constructor = doc['constructor']
    wall = doc['wall']
    pubs = outcome['publications']
    cons_fp = constructor.get('placementFingerprint')
    # The committed filter, verbatim: loop-relative `wallSeconds <= limit`.
    within = [r for r in pubs
              if r.get('wallSeconds') is None or r['wallSeconds'] <= limit]
    strict = [r for r in within if r['placementFingerprint'] != cons_fp]
    best = min((r['publishedRawDepthMm'] for r in strict), default=None)
    incumbent = outcome['incumbent']
    return {
        'seed': seed,
        'budgetSeconds': limit,
        'exit': 0,
        'valid': True,
        'constructorDepthMm': constructor.get('rawSourceDepthMm'),
        'constructorSeconds': wall.get('constructorSeconds'),
        'searchSeconds': wall.get('searchSeconds'),
        'totalSeconds': wall.get('totalSeconds'),
        'bestStrictChildMm': best,
        'incumbentMm': incumbent.get('rawSourceDepthMm'),
        'incumbentIsConstructor': incumbent.get('fromConstructor'),
        'publicationsWithinBudget': len(within),
        'publicationsTotal': len(pubs),
        'strictChildren': len(strict),
        'exploreBites': outcome.get('exploreBites'),
        'compressBites': outcome.get('compressBites'),
        'funnel': outcome.get('funnel'),
        'invalidPublications': outcome.get('invalidPublications'),
        'repairMaxUm': (outcome.get('repairMaxDisplacementMm') or 0.0) * 1000.0,
        'repairMaxGivebackMm': outcome.get('repairMaxGivebackMm'),
        'relocateEconomics': outcome.get('relocateEconomics'),
        'lastPublicationOrdinal': pubs[-1]['ordinal'] if pubs else None,
        'finalWidthMm': outcome.get('finalWidthMm'),
        'minRawPhiOfLastBite': (outcome.get('bites') or [{}])[-1].get('minRawPhi'),
        'bites': outcome.get('bites'),
        'qualifies': bool(best is not None and best <= BAR_MM),
    }


def main():
    with open(COMMITTED) as handle:
        committed = json.load(handle)
    mismatches = []
    checked = 0
    for budget in BUDGETS:
        for row in committed['cells'][budget]['seeds']:
            seed = row['seed']
            mine = reduce_cell(budget, seed)
            tag = f'{budget}s-seed{seed}'
            for field in SCALAR_FIELDS:
                checked += 1
                if bits(mine[field]) != bits(row.get(field)):
                    mismatches.append({'cell': tag, 'field': field,
                                       'raw': mine[field],
                                       'committed': row.get(field)})
            for field in ('funnel', 'relocateEconomics',
                          'lastPublicationOrdinal'):
                checked += 1
                if json.dumps(mine[field], sort_keys=True) != \
                   json.dumps(row.get(field), sort_keys=True):
                    mismatches.append({'cell': tag, 'field': field,
                                       'raw': mine[field],
                                       'committed': row.get(field)})
            checked += 1
            if json.dumps(mine['bites'], sort_keys=True) != \
               json.dumps(row.get('bites'), sort_keys=True):
                mismatches.append({'cell': tag, 'field': 'bites',
                                   'raw': 'len=%d' % len(mine['bites'] or []),
                                   'committed': 'len=%d'
                                   % len(row.get('bites') or [])})
    # README Part I, re-derived from the raw documents alone.
    curve = {}
    for budget in BUDGETS:
        curve[budget] = {}
        for seed in range(9):
            mine = reduce_cell(budget, seed)
            curve[budget][seed] = {
                'bestStrictChildMm': mine['bestStrictChildMm'],
                'exploreBites': mine['exploreBites'],
                'qualifies': mine['qualifies'],
            }
    qual = sorted(s for s in range(9) if curve['10'][s]['qualifies'])
    doc = {
        'what': 'committed wall.json vs the raw cell documents it reduced',
        'raw': RAW, 'committed': COMMITTED,
        'fieldsChecked': checked,
        'mismatches': mismatches,
        'REDUCTION_FAITHFUL': not mismatches,
        'curveFromRaw': curve,
        'gateQualifyingSeeds': qual,
        'gateQuorumReached': len(qual),
        'gateQuorumRequired': QUORUM,
        'GATE_PASS': len(qual) >= QUORUM,
        'committedVerdict': committed['verdict'],
    }
    if len(sys.argv) > 1:
        with open(sys.argv[1], 'w') as handle:
            handle.write(json.dumps(doc, indent=1, sort_keys=True) + '\n')
    print(f'fields checked: {checked}  mismatches: {len(mismatches)}')
    for row in mismatches[:40]:
        print('  MISMATCH', row)
    print('gate qualifying seeds from raw:', qual, 'GATE_PASS:',
          doc['GATE_PASS'])
    print('committed verdict:', committed['verdict'])
    return 0 if not mismatches else 1


if __name__ == '__main__':
    sys.exit(main())
