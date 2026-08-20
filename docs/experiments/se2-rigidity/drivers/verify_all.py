#!/usr/bin/env python3
"""Run the independent witness check over every cell of the sweep.

    python3 verify_all.py [certdir] [outfile]

One row per (parent, trust radius, program, motion): the certificate's own
delta beside an out-of-engine recomputation of it, plus the containment and
pair-clearance margins the engine's `validate_publication` claims to have
checked. `AGREES` is the point of the table - if the engine and this script
ever disagreed about a depth, neither number could be trusted.
"""
import json
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import certify  # noqa: E402  (for PARENTS and TRUST)

certdir = sys.argv[1] if len(sys.argv) > 1 else '/var/lib/t3/tmp/se2cert'
outfile = sys.argv[2] if len(sys.argv) > 2 else f'{certdir}/verify.json'

rows = []
for label, path, _depth, _allowance in certify.PARENTS:
    for trust in certify.TRUST:
        cert = f'{certdir}/cert-{label}-t{trust}.json'
        if not os.path.exists(cert):
            continue
        for program in ('depthOnly', 'stripCoupled'):
            for motion in ('translationOnly', 'se2'):
                proc = subprocess.run(
                    [sys.executable, f'{HERE}/verify_witness.py', path, cert,
                     program, motion],
                    capture_output=True, check=True)
                got = json.loads(proc.stdout)
                got.update({'parent': label, 'trustMm': trust})
                got['AGREES'] = (
                    got.get('CALIBRATED')
                    and abs(got.get('independentDeltaMm', 1)
                            - got.get('engineDeltaMm', 0)) < 1e-9)
                rows.append(got)
                print(f"{label} t={trust:<6} {program:13} {motion:16} "
                      f"delta={got.get('engineDeltaMm')} "
                      f"indep={got.get('independentDeltaMm')} "
                      f"agrees={got['AGREES']} "
                      f"contain={got.get('CONTAINMENT_OK')} "
                      f"pair={got.get('PAIR_CLEARANCE_OK')} "
                      f"minPair={got.get('movedWorstPairDistanceMm')}",
                      flush=True)

json.dump(rows, open(outfile, 'w'), indent=1)
agree = all(r['AGREES'] for r in rows)
contain = all(r.get('CONTAINMENT_OK') for r in rows)
pair = all(r.get('PAIR_CLEARANCE_OK') for r in rows)
print(f'\nALL_AGREE={agree} ALL_CONTAINED={contain} ALL_PAIR_OK={pair} '
      f'rows={len(rows)}')
print(f'wrote {outfile}')
