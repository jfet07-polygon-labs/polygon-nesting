#!/usr/bin/env python3
"""Sol review 10 §3's gate, read off `matched.json` as a per-seed verdict.

    verdict.py MATCHEDJSON BLOCKLABEL CONTROLLABEL [OUT]

The gate has two clauses and this driver reports both, separately, because they
can disagree and a single headline that hides the disagreement is not a verdict:

* **">= 2/3 seeds moved"** - on how many of the twelve parents does the block
  arm end strictly shallower than the control arm, paired on the seed;
* **"net mm/work improvement"** - is the block's depth per unit of cost better
  than the control's.

"Cost" is the honest part. `processWorkUnits` is zero on the block arm, measured
(see `matched.py`), so the two commensurable axes are wall seconds and
whole-layout exact validations. The wall column carries the control arm's
process startup, which is 2-3 s of parent load and surrogate build before its
slice does anything, so it flatters the block; the exact-validation column does
not, because both arms are counted only for calls they make. Both are printed.
"""
import json
import statistics
import sys


def cost_of(row):
    """Whole-layout `validate_publication` calls, whichever arm this is."""
    if row.get('validations') is not None:
        return row['validations']
    return row.get('confirmationsAttempted') or 0


def main():
    matched = json.load(open(sys.argv[1]))
    block_label, control_label = sys.argv[2], sys.argv[3]
    out_path = sys.argv[4] if len(sys.argv) > 4 else None
    rows = []
    for cell in matched['cells']:
        block = cell['arms'].get(block_label) or {}
        control = cell['arms'].get(control_label) or {}
        if 'deltaVsParentMm' not in block or 'deltaVsParentMm' not in control:
            continue
        rows.append({
            'seed': cell['seed'],
            'parentMm': cell['parentRawDepthMm'],
            'blockDepthMm': block['rawSourceDepthMm'],
            'controlDepthMm': control['rawSourceDepthMm'],
            # Negative means the block is shallower, i.e. the block won.
            'blockMinusControlMm': (block['rawSourceDepthMm']
                                    - control['rawSourceDepthMm']),
            'blockDeltaMm': block['deltaVsParentMm'],
            'controlDeltaMm': control['deltaVsParentMm'],
            'blockExact': cost_of(block),
            'controlExact': cost_of(control),
            'blockWall': block.get('processWallSeconds'),
            'controlWall': control.get('processWallSeconds'),
            'blockOperatorMs': block.get('operatorMs'),
            'blockRefusals': block.get('refusals'),
            'blockHeadroomMm': block.get('medianHeadroomMm'),
        })
    diffs = [r['blockMinusControlMm'] for r in rows]
    block_per_exact = [r['blockDeltaMm'] / r['blockExact'] * 1e3
                       for r in rows if r['blockExact']]
    control_per_exact = [r['controlDeltaMm'] / r['controlExact'] * 1e3
                         for r in rows if r['controlExact']]
    paired_ratio = [
        (r['blockDeltaMm'] / r['blockExact'])
        / (r['controlDeltaMm'] / r['controlExact'])
        for r in rows if r['blockExact'] and r['controlExact']
        and r['controlDeltaMm'] > 0]
    verdict = {
        'blockArm': block_label,
        'controlArm': control_label,
        'seeds': len(rows),
        'seedsBlockShallower': sum(1 for d in diffs if d < 0),
        'seedsControlShallower': sum(1 for d in diffs if d > 0),
        'seedsEqual': sum(1 for d in diffs if d == 0),
        'clauseOneTwoThirdsSeedsMoved': sum(1 for d in diffs if d < 0)
                                        >= (2 * len(rows) + 2) // 3,
        'medianBlockMinusControlMm': (statistics.median(diffs)
                                      if diffs else None),
        'medianBlockDeltaMm': statistics.median(
            [r['blockDeltaMm'] for r in rows]) if rows else None,
        'medianControlDeltaMm': statistics.median(
            [r['controlDeltaMm'] for r in rows]) if rows else None,
        'medianBlockMmPerKiloExact': (statistics.median(block_per_exact)
                                      if block_per_exact else None),
        'medianControlMmPerKiloExact': (statistics.median(control_per_exact)
                                        if control_per_exact else None),
        'medianPairedEfficiencyRatioBlockOverControl':
            statistics.median(paired_ratio) if paired_ratio else None,
        'medianBlockExactValidations': statistics.median(
            [r['blockExact'] for r in rows]) if rows else None,
        'medianControlExactValidations': statistics.median(
            [r['controlExact'] for r in rows]) if rows else None,
        'medianBlockOperatorMs': statistics.median(
            [r['blockOperatorMs'] for r in rows
             if r['blockOperatorMs'] is not None]) if rows else None,
        'medianBlockWallSeconds': statistics.median(
            [r['blockWall'] for r in rows if r['blockWall']]) if rows else None,
        'medianControlWallSeconds': statistics.median(
            [r['controlWall'] for r in rows
             if r['controlWall']]) if rows else None,
        'rows': rows,
    }
    verdict['clauseTwoNetMmPerWorkImprovement'] = bool(
        verdict['medianPairedEfficiencyRatioBlockOverControl'] is not None
        and verdict['medianPairedEfficiencyRatioBlockOverControl'] > 1.0)
    verdict['GATE_PASS'] = bool(verdict['clauseOneTwoThirdsSeedsMoved']
                                and verdict['clauseTwoNetMmPerWorkImprovement'])
    print(json.dumps({k: v for k, v in verdict.items() if k != 'rows'},
                     indent=1))
    for row in rows:
        print(f"  seed{row['seed']:>2}: parent={row['parentMm']:.4f} "
              f"block={row['blockDepthMm']:.4f}({row['blockExact']}) "
              f"control={row['controlDepthMm']:.4f}({row['controlExact']}) "
              f"blockMinusControl={row['blockMinusControlMm']:+.4f}")
    if out_path:
        json.dump(verdict, open(out_path, 'w'), indent=1)


if __name__ == '__main__':
    main()
