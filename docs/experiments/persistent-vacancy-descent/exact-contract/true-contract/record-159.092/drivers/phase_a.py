#!/usr/bin/env python3
"""Phase A: PERTURB x mode-31 (bounded global legalization).

For every (k, d) cell the k deepest pieces by true transformed max-Y are nudged
`d` mm into the packed body, the resulting infeasible state is written as a
parent fixture, and mode 31 is run against it at a ladder of depth bounds. The
k=0 row is the matched control: the same bounds on the unperturbed record.
"""

import json
import os
import sys

import lib

OUT = '/var/lib/t3/tmp/combo/phase-a'
PARENT_DEPTH = None
TARGETS = [159.10, 159.00, 158.50, 158.00, 157.00, 156.00, 155.00]


def main():
    global PARENT_DEPTH
    os.makedirs(OUT, exist_ok=True)
    placements = json.load(open(lib.PARENT))['placements']
    PARENT_DEPTH = lib.depth_mm(placements)
    print(f'parent raw depth {PARENT_DEPTH!r}')
    rows = []
    cells = [(0, 0.0)] + [(k, d) for k in (2, 3, 4, 6) for d in (1.0, 2.0, 3.5)]
    for k, d in cells:
        if k == 0:
            fixture, perturbed_depth = lib.PARENT, PARENT_DEPTH
            tag = 'control'
        else:
            perturbed = lib.nudge(placements, k, d)
            tag = f'k{k}-d{d}'
            fixture = f'{OUT}/{tag}.json'
            perturbed_depth = lib.write_fixture(
                fixture, f'pv-combo nudge {tag}', perturbed,
                reported_depth_mm=PARENT_DEPTH)
        targets = [round(perturbed_depth, 6)] + TARGETS if k else TARGETS
        print(f'--- {tag}: perturbed raw depth {perturbed_depth:.6f}')
        for target in targets:
            bound = target + lib.BOUND_OFFSET_MM
            run_tag = f'{tag}-t{target}'
            result = lib.run(run_tag, 31, fixture, f'{bound:.6f}', 0, OUT)
            print('  ' + lib.line(run_tag, result))
            pop = lib.population(result) or {}
            rows.append({
                'k': k, 'd': d, 'target': target, 'bound': bound,
                'exactValid': pop.get('exactValid'),
                'depth': pop.get('independentDepthMm'),
                'raw': pop.get('rawSourceDepthMm'),
                'failure': pop.get('failureReason'),
                'global': pop.get('globalLegalization'),
                'run': f'{OUT}/{run_tag}.json',
            })
            sys.stdout.flush()
    json.dump(rows, open(f'{OUT}/rows.json', 'w'), indent=1)
    wins = [r for r in rows if r['exactValid'] and r['raw'] is not None
            and r['raw'] < PARENT_DEPTH - 1e-9]
    print(f'\nPUBLICATIONS BELOW PARENT: {len(wins)}')
    for win in sorted(wins, key=lambda r: r['raw']):
        print(f"  k={win['k']} d={win['d']} target={win['target']} "
              f"raw={win['raw']} run={win['run']}")


if __name__ == '__main__':
    main()
