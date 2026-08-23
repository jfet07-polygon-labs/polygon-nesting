#!/usr/bin/env python3
"""**Every publication of the committed round, checked against the engine's own
exact-checkpoint record.**

The committed reduction keeps only counts. The raw cell documents keep, per
publication, `targetDepthMm`/`publishedRawDepthMm`/`repair*`/the two
fingerprints/`improvedIncumbent`, and, per exact checkpoint,
`kernelExclusiveValid`/`contractValid`/`refusal`/`proxyRawDepthMm`. Every
identity below is a statement about the *engine's* numbers, recomputed here
from nothing but those two arrays.

Per cell:

  P1  every publication is matched, by proposal ordinal and depth, to a
      checkpoint whose `kernelExclusiveValid` AND `contractValid` are true;
  P2  the converse: every checkpoint with `publishedRawDepthMm != null` is a
      publication, and no checkpoint is dual-valid-and-refused;
  P3  `publishedRawDepthMm <= targetDepthMm` (publish.rs refuses otherwise);
  P4  `repairDepthGivebackMm == publishedRawDepthMm - proxyRawDepthMm`, bit for
      bit, on the checkpoint (publish.rs:423);
  P5  `improvedIncumbent` is exactly "strictly below the running minimum";
  P6  `incumbent.rawSourceDepthMm` is the minimum over `improvedIncumbent`
      publications, bit for bit;
  P7  the parent chain: publication k+1's `parentFingerprint` is publication
      k's `placementFingerprint` whenever k improved the incumbent;
  P8  `repairRows <= 4 * 61` and `repairMaxDisplacementMm <= 0.016`;
  P9  `outcome.invalidPublications == 0` AND it is recomputed here as
      "published but not dual-valid", which must also be 0;
  P10 `work.exactCheckpoints == len(exactCheckpoints)`;
  P11 the depth series over `improvedIncumbent` rows is strictly decreasing.

Exit 0 iff every identity holds on every cell.
"""
import json
import os
import struct
import sys

RAW = os.environ.get('ICS_RAW', '/var/lib/t3/tmp/overlapics/rerun')
BUDGETS = ('3', '10', '30')
PIECES = 61
REPAIR_CAP_MM = 0.016


def b(x):
    return struct.pack('>d', x).hex()


def check(cell, doc, fails, counts):
    outcome = doc['outcome']
    pubs = outcome['publications']
    cps = outcome['exactCheckpoints']
    cons_fp = doc['constructor'].get('placementFingerprint')

    def fail(rule, detail):
        fails.append({'cell': cell, 'rule': rule, 'detail': detail})

    # P10
    counts['identities'] += 1
    if outcome['work']['exactCheckpoints'] != len(cps):
        fail('P10', {'work': outcome['work']['exactCheckpoints'],
                     'rows': len(cps)})

    published_cps = [c for c in cps if c['publishedRawDepthMm'] is not None]
    # P2
    counts['identities'] += 1
    if len(published_cps) != len(pubs):
        fail('P2', {'publishedCheckpoints': len(published_cps),
                    'publications': len(pubs)})
    for c in cps:
        counts['identities'] += 1
        dual = c['kernelExclusiveValid'] and c['contractValid']
        if (c['publishedRawDepthMm'] is not None) != dual:
            fail('P2', {'ordinal': c['proposalOrdinal'], 'dual': dual,
                        'published': c['publishedRawDepthMm']})
        counts['identities'] += 1
        if (c['refusal'] is None) != (c['publishedRawDepthMm'] is not None):
            fail('P2r', {'ordinal': c['proposalOrdinal'],
                         'refusal': c['refusal'],
                         'published': c['publishedRawDepthMm']})
        # P4 on every checkpoint that got as far as measuring a depth
        if c['publishedRawDepthMm'] is not None:
            counts['identities'] += 1
            if b(c['repairDepthGivebackMm']) != \
               b(c['publishedRawDepthMm'] - c['proxyRawDepthMm']):
                fail('P4', {'ordinal': c['proposalOrdinal'],
                            'giveback': c['repairDepthGivebackMm'],
                            'recomputed': c['publishedRawDepthMm']
                            - c['proxyRawDepthMm']})
            counts['identities'] += 1
            if c['publishedRawDepthMm'] > c['targetDepthMm']:
                fail('P3c', {'ordinal': c['proposalOrdinal'],
                             'published': c['publishedRawDepthMm'],
                             'target': c['targetDepthMm']})

    # P1: match publications to dual-valid checkpoints, in order.
    counts['identities'] += 1
    pub_key = [(p['ordinal']['proposals'], b(p['publishedRawDepthMm']),
                p['repairRows'], b(p['repairDepthGivebackMm']))
               for p in pubs]
    cp_key = [(c['proposalOrdinal'], b(c['publishedRawDepthMm']),
               c['repairRows'], b(c['repairDepthGivebackMm']))
              for c in published_cps]
    if pub_key != cp_key:
        first = next((i for i, (x, y) in enumerate(zip(pub_key, cp_key))
                      if x != y), min(len(pub_key), len(cp_key)))
        fail('P1', {'firstDivergenceIndex': first,
                    'publication': pub_key[first:first + 1],
                    'checkpoint': cp_key[first:first + 1]})

    running = float('inf')
    prev_fp = cons_fp
    improved = []
    for i, p in enumerate(pubs):
        counts['publications'] += 1
        # P3
        counts['identities'] += 1
        if p['publishedRawDepthMm'] > p['targetDepthMm']:
            fail('P3', {'i': i, 'published': p['publishedRawDepthMm'],
                        'target': p['targetDepthMm']})
        # P8
        counts['identities'] += 1
        if p['repairRows'] > 4 * PIECES:
            fail('P8rows', {'i': i, 'rows': p['repairRows']})
        counts['identities'] += 1
        if p['repairMaxDisplacementMm'] > REPAIR_CAP_MM:
            fail('P8disp', {'i': i, 'mm': p['repairMaxDisplacementMm']})
        # P5
        counts['identities'] += 1
        want = p['publishedRawDepthMm'] < running
        if p['improvedIncumbent'] != want:
            fail('P5', {'i': i, 'flag': p['improvedIncumbent'],
                        'depth': p['publishedRawDepthMm'],
                        'runningMin': running})
        # P7
        counts['identities'] += 1
        if p['parentFingerprint'] != prev_fp:
            fail('P7', {'i': i, 'parent': p['parentFingerprint'],
                        'previousPublished': prev_fp})
        if p['improvedIncumbent']:
            improved.append(p['publishedRawDepthMm'])
            running = min(running, p['publishedRawDepthMm'])
            prev_fp = p['placementFingerprint']

    # P6
    counts['identities'] += 1
    inc = outcome['incumbent']['rawSourceDepthMm']
    if improved:
        if b(min(improved)) != b(inc):
            fail('P6', {'incumbent': inc, 'minImproved': min(improved)})
    # P11
    counts['identities'] += 1
    if any(improved[i] >= improved[i - 1] for i in range(1, len(improved))):
        fail('P11', {'series': improved[:6]})
    # P9
    counts['identities'] += 1
    recomputed_invalid = sum(
        1 for c in cps if c['publishedRawDepthMm'] is not None
        and not (c['kernelExclusiveValid'] and c['contractValid']))
    if outcome['invalidPublications'] != 0 or recomputed_invalid != 0:
        fail('P9', {'reported': outcome['invalidPublications'],
                    'recomputed': recomputed_invalid})
    return {
        'cell': cell,
        'publications': len(pubs),
        'checkpoints': len(cps),
        'publishedCheckpoints': len(published_cps),
        'refusedCheckpoints': len(cps) - len(published_cps),
        'improvingPublications': len(improved),
        'incumbentMm': inc,
        'minImprovedMm': min(improved) if improved else None,
        'maxRepairRows': max((p['repairRows'] for p in pubs), default=0),
        'maxRepairDisplacementMm': max(
            (p['repairMaxDisplacementMm'] for p in pubs), default=0.0),
        'maxGivebackMm': max((p['repairDepthGivebackMm'] for p in pubs),
                             default=0.0),
    }


def main():
    fails, counts = [], {'identities': 0, 'publications': 0}
    rows = []
    refusals = {}
    for budget in BUDGETS:
        for seed in range(9):
            cell = f'{budget}s-seed{seed}'
            with open(f'{RAW}/wall-{budget}s-seed{seed}.json') as handle:
                doc = json.load(handle)
            rows.append(check(cell, doc, fails, counts))
            for c in doc['outcome']['exactCheckpoints']:
                if c['refusal']:
                    refusals[c['refusal']] = refusals.get(c['refusal'], 0) + 1
    doc = {
        'what': 'every publication and every exact checkpoint of the 27 cells',
        'raw': RAW,
        'cells': rows,
        'totalPublications': sum(r['publications'] for r in rows),
        'totalCheckpoints': sum(r['checkpoints'] for r in rows),
        'totalRefusedCheckpoints': sum(r['refusedCheckpoints'] for r in rows),
        'identitiesChecked': counts['identities'],
        'failures': fails,
        'ALL_IDENTITIES_HOLD': not fails,
        'refusalReasons': refusals,
        'maxRepairDisplacementMmAcrossRound': max(
            r['maxRepairDisplacementMm'] for r in rows),
        'maxRepairRowsAcrossRound': max(r['maxRepairRows'] for r in rows),
        'maxGivebackMmAcrossRound': max(r['maxGivebackMm'] for r in rows),
    }
    if len(sys.argv) > 1:
        with open(sys.argv[1], 'w') as handle:
            handle.write(json.dumps(doc, indent=1, sort_keys=True) + '\n')
    print('publications:', doc['totalPublications'],
          ' checkpoints:', doc['totalCheckpoints'],
          ' refused:', doc['totalRefusedCheckpoints'])
    print('identities checked:', counts['identities'],
          ' failures:', len(fails))
    for f in fails[:30]:
        print('  FAIL', f)
    print('max repair displacement (mm):',
          doc['maxRepairDisplacementMmAcrossRound'],
          ' max rows:', doc['maxRepairRowsAcrossRound'],
          ' max giveback (mm):', doc['maxGivebackMmAcrossRound'])
    print('refusal reasons:', json.dumps(refusals, indent=1))
    return 0 if not fails else 1


if __name__ == '__main__':
    sys.exit(main())
