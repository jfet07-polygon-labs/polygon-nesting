#!/usr/bin/env python3
"""The Sparrow re-import, through the full armed publication path.

    sparrowcheck.py BATTERYJSON OUT.json

Gate A established that the 150.16451 mm Sparrow layout is accepted by the
material contract and refused by the composite's miter envelope. Deliverable 3
asks the next question - whether the *armed* path publishes it - and it asks it
of the wire point rather than of the kernel: `wired_verdicts` calls
`validate_and_measure_placements`, which is the same function a mode-34 slice
and the coordinator's own publication both call.

This is a **legality** answer and nothing more. Whether a search can reach that
layout in ten seconds is what deliverables 1 and 2 measure, and they measure it
separately for the reason Grok review 7 §3 gives: legalising a pose does not
make it appear.

The allowance matters and is reported per row rather than collapsed. The
composite's collision expansion is `total_padding/2 + margin + allowance`, so
the shipping 0.002 mm search-offset allowance asks for a **2.502 mm** disc where
the contract asks for 2.500 - a tax that is nothing to do with the join.
"""
import json
import sys


def main():
    battery = json.load(open(sys.argv[1]))
    out_path = sys.argv[2]
    section = battery['population3SparrowDifferential']
    rows = []
    for row in section['rows']:
        rows.append({
            'searchOffsetAllowanceMm': row['searchOffsetAllowanceMm'],
            'expansionMm': row['expansionMm'],
            'contractOnlyAccepts': row.get('contractOnlyAccepts'),
            'compositeMiterAccepts':
                bool((row.get('compositeMiterVerdict') or {}).get('accepted')),
            'compositeRoundAccepts':
                bool((row.get('compositeRoundVerdict') or {}).get('accepted')),
            'compositeUnionAccepts':
                bool((row.get('compositeUnionVerdict') or {}).get('accepted')),
            'kernelPairFailureCount': row.get('kernelPairFailureCount'),
            'kernelBoundaryFailureCount': row.get('kernelBoundaryFailureCount'),
            'kernelRefusedPairIndices': row.get('kernelRefusedPairIndices'),
            'miterMessage': (row.get('compositeMiterVerdict') or {})
            .get('message'),
            'unionMessage': (row.get('compositeUnionVerdict') or {})
            .get('message'),
        })
    publishable = [r for r in rows if r['compositeUnionAccepts']]
    result = {
        'poses': section.get('poses'),
        'rows': rows,
        'publishableUnderArmedAuthorityAtAllowancesMm':
            [r['searchOffsetAllowanceMm'] for r in publishable],
        'publishableAtShippingAllowance': any(
            r['compositeUnionAccepts']
            and r['searchOffsetAllowanceMm'] == 0.002 for r in rows),
        'publishableAtZeroAllowance': any(
            r['compositeUnionAccepts']
            and r['searchOffsetAllowanceMm'] == 0.0 for r in rows),
        'publishableUnderMiterAtAnyAllowance': any(
            r['compositeMiterAccepts'] for r in rows),
        'contractAcceptsAtEveryAllowance': all(
            r['contractOnlyAccepts'] for r in rows),
    }
    json.dump(result, open(out_path, 'w'), indent=1)
    print(json.dumps({k: v for k, v in result.items()
                      if k not in ('rows', 'poses')}, indent=1))


if __name__ == '__main__':
    main()
