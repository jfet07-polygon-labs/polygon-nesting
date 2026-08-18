#!/usr/bin/env python3
"""Re-confirms every publication the A/B/C/D battery made, and writes the
confirmations as one evidence document.

    python3 collect.py evidence/abc-equalwork-mixed61.json OUT.json [BINARY]

Every arm that moved the incumbent gets its layout replayed through mode 27 in
a separate process from the default-feature gate binary. An arm that published
nothing is recorded as such rather than skipped: `0 of 3` is a result.
"""
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import runlib  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))


def main():
    battery = json.load(open(sys.argv[1]))
    out_path = sys.argv[2]
    binary = sys.argv[3] if len(sys.argv) > 3 \
        else '/var/lib/t3/tmp/ledger-gate-final'
    result = {'battery': sys.argv[1], 'confirmBinary': binary,
              'confirmations': []}
    for row in battery['rows']:
        seed, arm = row['seed'], row['arm']
        entry = {'seed': seed, 'arm': arm,
                 'deltaRawMm': row.get('deltaRawMm'),
                 'publications': row.get('probePublications')}
        if not row.get('probePublications'):
            entry['confirmed'] = None
            entry['note'] = 'the arm published nothing; nothing to confirm'
            result['confirmations'].append(entry)
            continue
        run_path = f'{runlib.OUT}/abc/{battery["tag"]}-seed{seed}-{arm}.json'
        outdir = f'{runlib.OUT}/confirm/{battery["tag"]}-seed{seed}-{arm}'
        proc = subprocess.run(
            ['python3', f'{HERE}/confirm.py', run_path, outdir, binary],
            capture_output=True, check=False)
        try:
            report = json.loads(proc.stdout.decode())
        except json.JSONDecodeError:
            entry['error'] = proc.stderr.decode()[-800:]
            result['confirmations'].append(entry)
            continue
        mode27 = report['mode27']
        micro = mode27.get('microLegalization') or {}
        entry.update({
            'rawSourceDepthMm': mode27.get('rawSourceDepthMm'),
            'exactValid': mode27.get('exactValid'),
            'contractValid': mode27.get('contractValid'),
            'fingerprintUnchanged': report.get('fingerprintUnchanged'),
            'violatingPairsBefore': micro.get('violatingPairsBefore'),
            'collisionPairsBefore': micro.get('collisionPairsBefore'),
            'movedPieces': micro.get('movedPieces'),
            'confirmed': bool(mode27.get('exactValid')
                              and mode27.get('contractValid')
                              and report.get('fingerprintUnchanged')
                              and micro.get('violatingPairsBefore') == 0
                              and micro.get('movedPieces') == 0),
        })
        result['confirmations'].append(entry)
    result['ALL_CONFIRMED'] = all(
        c['confirmed'] for c in result['confirmations']
        if c['confirmed'] is not None)
    json.dump(result, open(out_path, 'w'), indent=1)
    print(json.dumps(result, indent=1))


if __name__ == '__main__':
    main()
