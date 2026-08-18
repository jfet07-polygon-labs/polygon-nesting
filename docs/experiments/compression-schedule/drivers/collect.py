#!/usr/bin/env python3
"""Copies the summarised evidence out of the scratch run directory.

    python3 collect.py RUNDIR EVIDENCEDIR

The per-run benchmark documents are large - a schedule arm records one row per
depth step and the equal-work arms take 15,000 to 36,000 of them - so the
evidence directory carries the summaries, the curves and the gate documents
rather than the raw stream. Every table in the README is computed from what is
copied here.
"""
import json
import os
import shutil
import sys


def main():
    rundir, evidence = sys.argv[1], sys.argv[2]
    os.makedirs(evidence, exist_ok=True)
    for name in ('gate-summary.json', 'gate-curve.json', 'parents.json',
                 'records.json', 'gates-base.json', 'gates-after.json',
                 'gates-armed.json', 'gates-docdiff.json',
                 'gates-docdiff-armed.json'):
        source = f'{rundir}/{name}'
        if os.path.exists(source):
            shutil.copy(source, f'{evidence}/{name}')
            print('copied', name)
    # The gate document itself, minus the per-step curves that make it large.
    gate_path = f'{rundir}/gate.json'
    if os.path.exists(gate_path):
        gate = json.load(open(gate_path))
        for cell in gate['cells']:
            for arm in cell['arms'].values():
                arm.pop('scheduleCurve', None)
        json.dump(gate, open(f'{evidence}/gate.json', 'w'), indent=1)
        print('copied gate.json (curves stripped)')


if __name__ == '__main__':
    main()
