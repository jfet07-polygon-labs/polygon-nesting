#!/usr/bin/env python3
"""One parent, one spec, the whole round document printed.

    smoke.py BINARY FIXTURE SEED SPEC [OUTPATH]

The per-round table this prints is the operator's whole decomposition on a
single cell, and it is the first thing to look at when a spec's numbers on the
twelve-parent table look wrong.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import runlib  # noqa: E402

ROUND_KEYS = ('round', 'trustRadiusMm', 'parentDepthMm', 'setter', 'block',
              'depthBandPieces', 'seedRank', 'headroomMm', 'rows', 'deltaRows',
              'modelLowerMm', 'modelUpperMm', 'fullStepExactValid', 'scale',
              'validations', 'validatedDeltaMm', 'maxAbsDthetaDeg',
              'maxAbsTranslationMm', 'refusal')


def main():
    binary, fixture, seed, spec = sys.argv[1:5]
    out_path = sys.argv[5] if len(sys.argv) > 5 \
        else '/var/lib/t3/tmp/cblock/out/smoke.json'
    doc, wall, err, code = runlib.probe(
        binary, 'mixed-61', int(seed), fixture,
        {'POLYGON_NESTING_CONTACT_BLOCK': spec}, out_path, timeout=3600)
    print(f'exit={code} wall={wall:.2f}s')
    if doc is None:
        print('STDERR:', err[-2000:])
        raise SystemExit(1)
    head = {k: v for k, v in doc.items() if k != 'rounds'}
    print(json.dumps(head, indent=1))
    for entry in doc['rounds']:
        print(json.dumps({k: entry.get(k) for k in ROUND_KEYS}))
        print('   familes:', entry.get('rowsByFamily'))
        print('   edges:', [(e['first'], e['second'], round(e['slackMm'], 4),
                             e['gate']) for e in entry['edges']][:12])
        if entry.get('fullStepRejection'):
            print('   rejection:', entry['fullStepRejection'][:200])


if __name__ == '__main__':
    main()
