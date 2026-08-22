#!/usr/bin/env python3
"""Both arms priced on **operator time alone**, with the process out of it.

    slicetime.py MATCHEDDIR MATCHEDJSON BLOCKLABEL CONTROLWORK [OUT]

The wall column in `matched.py` carries the whole process: the request parse,
the parent load, the surrogate build. On the control arm that is 2-3 s before
its slice takes a single step, and on the block arm it is a fraction of a
second, so a wall comparison flatters the block by exactly the control's
startup. The fair wall statement compares the two operators:

* the block's own `elapsedMs`, which is the operator and nothing else;
* the slice's `repairMs + confirmationMs`, which is `witnessprice.py`'s own
  definition of the slice a proposal source is spent out of
  (`docs/experiments/sparse-rotation/` §3.1).

This is the axis on which the block does best, so it is the one to be most
careful about: it is reported here in full rather than left out because it did
not help the conclusion.
"""
import json
import os
import statistics
import sys


def slice_ms(path):
    doc = json.load(open(path))
    pop = ((doc.get('relaxedDiagnostics') or {})
           .get('coupledDynamicSeparator') or {}).get(
               'persistentVacancyPopulation') or {}
    schedule = pop.get('compressionSchedule') or {}
    return ((schedule.get('repairMs') or 0.0)
            + (schedule.get('confirmationMs') or 0.0)), schedule


def main():
    matched_dir, matched_json = sys.argv[1], sys.argv[2]
    block_label, control_work = sys.argv[3], sys.argv[4]
    out_path = sys.argv[5] if len(sys.argv) > 5 else None
    matched = json.load(open(matched_json))
    rows = []
    for cell in matched['cells']:
        seed = cell['seed']
        block = cell['arms'].get(block_label) or {}
        control = cell['arms'].get(f'm34:{control_work}') or {}
        path = f'{matched_dir}/seed{seed}-m34-{control_work}.json'
        if not os.path.exists(path) or 'deltaVsParentMm' not in block:
            continue
        control_ms, schedule = slice_ms(path)
        rows.append({
            'seed': seed,
            'blockDeltaMm': block['deltaVsParentMm'],
            'blockOperatorMs': block.get('operatorMs'),
            'controlDeltaMm': control.get('deltaVsParentMm'),
            'controlSliceMs': control_ms,
            'controlRepairMs': schedule.get('repairMs'),
            'controlConfirmationMs': schedule.get('confirmationMs'),
            'controlConfirmationsAttempted':
                schedule.get('confirmationsAttempted'),
            'controlStepsTaken': schedule.get('stepsTaken'),
            'blockMmPerOperatorSecond': (
                block['deltaVsParentMm'] / (block['operatorMs'] / 1e3)
                if block.get('operatorMs') else None),
            'controlMmPerSliceSecond': (
                control.get('deltaVsParentMm', 0.0) / (control_ms / 1e3)
                if control_ms else None),
        })
    ratios = [r['blockMmPerOperatorSecond'] / r['controlMmPerSliceSecond']
              for r in rows
              if r['blockMmPerOperatorSecond'] is not None
              and r['controlMmPerSliceSecond']]
    summary = {
        'blockArm': block_label,
        'controlArm': f'm34:{control_work}',
        'seeds': len(rows),
        'medianBlockOperatorMs': statistics.median(
            [r['blockOperatorMs'] for r in rows
             if r['blockOperatorMs'] is not None]),
        'medianControlSliceMs': statistics.median(
            [r['controlSliceMs'] for r in rows]),
        'medianBlockMmPerOperatorSecond': statistics.median(
            [r['blockMmPerOperatorSecond'] for r in rows
             if r['blockMmPerOperatorSecond'] is not None]),
        'medianControlMmPerSliceSecond': statistics.median(
            [r['controlMmPerSliceSecond'] for r in rows
             if r['controlMmPerSliceSecond'] is not None]),
        'medianPairedRatioBlockOverControl': (statistics.median(ratios)
                                              if ratios else None),
        'seedsBlockFasterPerMm': sum(1 for r in ratios if r > 1.0)
        if ratios else 0,
        'rows': rows,
    }
    print(json.dumps({k: v for k, v in summary.items() if k != 'rows'},
                     indent=1))
    for row in rows:
        print(f"  seed{row['seed']:>2}: block {row['blockDeltaMm']:.4f}mm in "
              f"{row['blockOperatorMs']:.0f}ms   control "
              f"{row['controlDeltaMm']:.4f}mm in "
              f"{row['controlSliceMs']:.0f}ms "
              f"({row['controlConfirmationsAttempted']} confirmations, "
              f"{row['controlStepsTaken']} steps)")
    if out_path:
        json.dump(summary, open(out_path, 'w'), indent=1)


if __name__ == '__main__':
    main()
