#!/usr/bin/env python3
"""Whole-document equivalence of the four pinned gates, flag-off vs flag-on.

    python3 gatecompare.py OFF_GATES_JSON ON_GATES_JSON OUT_JSON

`gates.py` runs the gates against one binary and records, per gate, the pinned
scalars and a whole-document digest with the wall-clock and build-identity
fields removed. This reduces the two runs to the claim the feature actually
makes, which is stronger than the pinned scalars:

  * flag-off reproduces the four pinned depths and fingerprints - the standard
    campaign gate, that the default build is untouched;
  * flag-on reproduces them too - and additionally hashes to the SAME document,
    field for field.

The second line is the one to read. `pconfirm` could only claim scalar
reproduction because a refused confirmation charges `exactPairTests`
differently under it; this filter has no such escape hatch, because it changes
no verdict and carries no counter. So `allDigestsEqual` false would be a bug
report, not a tolerance to widen.
"""
import json
import sys

off = json.load(open(sys.argv[1]))
on = json.load(open(sys.argv[2]))
rows = []
for gate in sorted(set(off['gates']) & set(on['gates'])):
    a, b = off['gates'][gate], on['gates'][gate]
    rows.append({
        'gate': gate,
        'offHit': a['hit'], 'onHit': b['hit'],
        'digestsEqual': a['docDigest'] == b['docDigest'],
        'docDigest': a['docDigest'],
        'offPinned': a.get('raw', a.get('depths')),
        'onPinned': b.get('raw', b.get('depths')),
        'pinnedEqual': a.get('raw', a.get('depths')) == b.get('raw',
                                                              b.get('depths')),
        'offWallSeconds': a.get('wallSeconds'),
        'onWallSeconds': b.get('wallSeconds'),
    })
out = {
    'offBinary': off['binary'], 'onBinary': on['binary'],
    'allPassOff': off['ALL_PASS'], 'allPassOn': on['ALL_PASS'],
    'allDigestsEqual': all(r['digestsEqual'] for r in rows),
    'allPinnedEqual': all(r['pinnedEqual'] for r in rows),
    'gates': rows,
    'claim': 'flag-on is whole-document identical to flag-off on all four '
             'pinned gates; no field is permitted to differ',
}
json.dump(out, open(sys.argv[3], 'w'), indent=1)
print(json.dumps({k: v for k, v in out.items() if k != 'gates'}, indent=1))
for row in rows:
    print(f"  {row['gate']}: offHit={row['offHit']} onHit={row['onHit']} "
          f"digestsEqual={row['digestsEqual']} digest={row['docDigest'][:16]}")
