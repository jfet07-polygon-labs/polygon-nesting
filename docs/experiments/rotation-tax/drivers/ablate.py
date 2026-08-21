#!/usr/bin/env python3
"""The §2 ablation: what the tax fixes are worth, at equal work, armed.

    ablate.py OUTDIR OLDBIN NEWBIN PARENTSJSON ROUNDS [SEEDS] [DROP_MM] [ALLOW]

One pinned parent, one serial mode-34 slice, a fixed work cap, the operator
**armed on both sides**, and the only difference is the binary. That is the
measurement the wall battery cannot make: a from-request run at a wall budget
answers "did the fixes buy depth", which is §4's question, and confounds the
per-slice cost with every downstream decision the freed time changes. Here the
work is pinned, so the arms do the same search and the difference is the clock.

Paired and interleaved, arm order reversed on odd rounds, because the box is
shared. The within-arm spread is printed beside the ratio.

The second thing this driver checks is the one that licenses the first: the
fixes are meant to be **answer-preserving**, so at equal work the two binaries
should walk the same trajectory to the same layout. `fingerprint`,
`rawSourceDepthMm`, `processCandidateQueries` and `processExactPairTests` are
compared cell by cell and any mismatch is reported rather than averaged away.
"""
import hashlib
import json
import os
import statistics
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import runlib  # noqa: E402
import taxprobe  # noqa: E402
import workgate  # noqa: E402

EQUAL = ('fingerprint', 'rawSourceDepthMm', 'processCandidateQueries',
         'processExactPairTests', 'exactValid', 'contractValid')


def main():
    outdir, old_bin, new_bin, parents_json = sys.argv[1:5]
    rounds = int(sys.argv[5])
    wanted = ({int(s) for s in sys.argv[6].split(',')}
              if len(sys.argv) > 6 and sys.argv[6] else None)
    drop_mm = float(sys.argv[7]) if len(sys.argv) > 7 else workgate.DEFAULT_DROP_MM
    allowance = sys.argv[8] if len(sys.argv) > 8 else runlib.DEFAULT_ALLOWANCE
    parents = [row for row in json.load(open(parents_json))['rows']
               if wanted is None or row['seed'] in wanted]
    os.makedirs(outdir, exist_ok=True)
    result = {
        'old': old_bin, 'new': new_bin,
        'oldSha256': hashlib.sha256(open(old_bin, 'rb').read()).hexdigest(),
        'newSha256': hashlib.sha256(open(new_bin, 'rb').read()).hexdigest(),
        'rounds': rounds, 'dropMm': drop_mm, 'allowance': allowance,
        'workUnits': int(os.environ.get('WORKGATE_UNITS',
                                        workgate.DESIGN_SLICE_UNITS)),
        'cells': [], 'mismatches': [],
    }
    for round_index in range(rounds):
        order = (('old', old_bin), ('new', new_bin))
        if round_index % 2 == 1:
            order = tuple(reversed(order))
        for parent in parents:
            seed, parent_depth = parent['seed'], parent['rawDepthMm']
            cell = {'round': round_index, 'seed': seed, 'arms': {}}
            for label, binary in order:
                path = f'{outdir}/r{round_index}-seed{seed}-{label}.json'
                doc, wall, err = taxprobe.run_arm(
                    binary, seed, parent['fixture'], parent_depth - drop_mm,
                    True, path, allowance)
                cell['arms'][label] = ({'error': err[-600:]} if doc is None
                                       else workgate.row_for(doc, wall,
                                                             parent_depth))
            old, new = cell['arms']['old'], cell['arms']['new']
            for key in EQUAL:
                if old.get(key) != new.get(key):
                    result['mismatches'].append(
                        {'round': round_index, 'seed': seed, 'field': key,
                         'old': old.get(key), 'new': new.get(key)})
            if old.get('processWallSeconds') and new.get('processWallSeconds'):
                cell['ratio'] = (old['processWallSeconds']
                                 / new['processWallSeconds'])
            result['cells'].append(cell)
            print(f"r{round_index} seed{seed}: old="
                  f"{old.get('processWallSeconds')} new="
                  f"{new.get('processWallSeconds')} "
                  f"ratio={cell.get('ratio')}", flush=True)
            json.dump(result, open(f'{outdir}/ablate.json', 'w'), indent=1)

    ratios = [c['ratio'] for c in result['cells'] if 'ratio' in c]
    summary = {'pairedCells': len(ratios),
               'newFasterCells': sum(1 for r in ratios if r > 1.0),
               'medianSpeedup': statistics.median(ratios) if ratios else None,
               'minSpeedup': min(ratios) if ratios else None,
               'maxSpeedup': max(ratios) if ratios else None,
               'equalityMismatches': len(result['mismatches'])}
    for label in ('old', 'new'):
        by_seed = {}
        for cell in result['cells']:
            wall = cell['arms'][label].get('processWallSeconds')
            if wall:
                by_seed.setdefault(cell['seed'], []).append(wall)
        summary[f'{label}MedianWallBySeed'] = {
            seed: statistics.median(walls) for seed, walls in by_seed.items()}
        # Within-arm spread, per seed: the number a reader must see before the
        # between-arm ratio means anything on a shared box.
        summary[f'{label}RelativeSpreadBySeed'] = {
            seed: (max(walls) - min(walls)) / statistics.median(walls)
            for seed, walls in by_seed.items()}
    result['summary'] = summary
    json.dump(result, open(f'{outdir}/ablate.json', 'w'), indent=1)
    print(json.dumps(summary, indent=1))


if __name__ == '__main__':
    main()
