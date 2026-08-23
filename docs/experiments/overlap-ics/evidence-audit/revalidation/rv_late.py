#!/usr/bin/env python3
"""The three late publications, named, with the committed numbers they carry."""
import json
import os
import sys

RAW = os.environ.get('ICS_RAW', '/var/lib/t3/tmp/overlapics/rerun')
CASES = [('3', 3.000, 1), ('10', 10.000, 3), ('30', 30.000, 8)]


def main():
    out = []
    for budget, limit, seed in CASES:
        with open(f'{RAW}/wall-{budget}s-seed{seed}.json') as handle:
            doc = json.load(handle)
        pubs = doc['outcome']['publications']
        wall = doc['wall']
        cf = doc['constructor'].get('placementFingerprint')
        low = wall['constructorSeconds']
        up = wall['totalSeconds'] - wall['searchSeconds']
        rows = []
        for row in pubs:
            rows.append(dict(row, requestLower=low + row['wallSeconds'],
                             requestUpper=up + row['wallSeconds'],
                             strict=row['placementFingerprint'] != cf))
        strict = [r for r in rows if r['strict']]
        best_old = min(r['publishedRawDepthMm'] for r in strict)
        winner = min((r for r in strict
                      if r['publishedRawDepthMm'] == best_old),
                     key=lambda r: r['requestLower'])
        late_low = [r for r in rows if r['requestLower'] > limit]
        late_up = [r for r in rows if r['requestUpper'] > limit]
        keep = ('ordinal', 'phase', 'publishedRawDepthMm', 'targetDepthMm',
                'wallSeconds', 'requestLower', 'requestUpper',
                'improvedIncumbent', 'repairRows', 'strict')
        entry = {
            'budget': budget, 'seed': seed, 'limitSeconds': limit,
            'constructorSeconds': low, 'outsideLoopSeconds': up,
            'totalSeconds': wall['totalSeconds'],
            'searchSeconds': wall['searchSeconds'],
            'committedBestStrictChildMm': best_old,
            'committedBestPublication': {k: winner[k] for k in keep},
            'committedBestIsLateUnderLowerBound':
                winner['requestLower'] > limit,
            'committedBestIsLateUnderUpperBound':
                winner['requestUpper'] > limit,
            'overrunSecondsLower': winner['requestLower'] - limit,
            'overrunSecondsUpper': winner['requestUpper'] - limit,
            'latePublicationsLowerBound':
                [{k: r[k] for k in keep} for r in late_low],
            'latePublicationsUpperBound':
                [{k: r[k] for k in keep} for r in late_up],
            'bestAfterExcludingLateLower':
                min((r['publishedRawDepthMm'] for r in strict
                     if r['requestLower'] <= limit), default=None),
            'bestAfterExcludingLateUpper':
                min((r['publishedRawDepthMm'] for r in strict
                     if r['requestUpper'] <= limit), default=None),
            'lastPublicationOrdinal': pubs[-1]['ordinal'],
            'incumbentRawSourceDepthMm':
                doc['outcome']['incumbent']['rawSourceDepthMm'],
        }
        entry['deltaMmLower'] = (entry['bestAfterExcludingLateLower']
                                 - best_old)
        entry['deltaMmUpper'] = (entry['bestAfterExcludingLateUpper']
                                 - best_old)
        out.append(entry)
    text = json.dumps(out, indent=1, sort_keys=True)
    if len(sys.argv) > 1:
        with open(sys.argv[1], 'w') as handle:
            handle.write(text + '\n')
    print(text)


if __name__ == '__main__':
    main()
