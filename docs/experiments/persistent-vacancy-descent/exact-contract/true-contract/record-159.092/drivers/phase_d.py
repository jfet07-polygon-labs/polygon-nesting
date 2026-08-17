#!/usr/bin/env python3
"""Phase D: widened perturbation sweep, to find where the nudge->mode-31 route
stops being a null. The assigned grid (k in {2,3,4,6} x d in {1.0,2.0,3.5}) is
included; everything else is supplementary characterisation of the mechanism.

usage: phase_d.py <parent-fixture> <outdir>
"""

import json
import os
import sys

import lib

KS = (2, 3, 4, 6, 8, 10, 12, 16)
DS = (0.05, 0.1, 0.2, 0.5, 1.0, 2.0, 3.5, 6.0)
STEPS = ('self', 0.08, 0.04, 0.02, 0.01)


def main():
    parent = sys.argv[1]
    out = sys.argv[2]
    os.makedirs(out, exist_ok=True)
    placements = json.load(open(parent))['placements']
    incumbent = lib.depth_mm(placements)
    print(f'incumbent {incumbent!r}', flush=True)
    rows = []
    for k in KS:
        for d in DS:
            perturbed = lib.nudge(placements, k, d)
            fixture = f'{out}/nudge-k{k}-d{d}.json'
            perturbed_depth = lib.write_fixture(
                fixture, f'pv-combo wide nudge k{k} d{d}', perturbed,
                reported_depth_mm=incumbent)
            best = None
            for step in STEPS:
                # 'self' bounds the program at the perturbed layout's own
                # frontier: the nudge did the compressing, and mode 31 is only
                # asked to legalize it without letting anything relax back.
                target = (perturbed_depth if step == 'self'
                          else incumbent - step)
                tag = f'k{k}-d{d}-s{step}'
                run_json = lib.run(tag, 31, fixture,
                                   f'{target + lib.BOUND_OFFSET_MM:.6f}', 0, out)
                pop = lib.population(run_json) or {}
                raw = pop.get('rawSourceDepthMm') if pop.get('exactValid') else None
                if raw is not None and raw < incumbent - 1e-9 and (
                        best is None or raw < best[0]):
                    best = (raw, step, tag)
                rows.append({'k': k, 'd': d, 'step': step, 'target': target,
                             'exactValid': pop.get('exactValid'), 'raw': raw,
                             'failure': (pop.get('failureReason') or '')[:80]})
            print(f'k={k:>2} d={d:<5} perturbedDepth={perturbed_depth:.6f} '
                  f'best={best}', flush=True)
    json.dump(rows, open(f'{out}/rows.json', 'w'), indent=1)
    wins = [r for r in rows if r['raw'] is not None]
    print(f'publications below incumbent: {len(wins)}')
    for win in sorted(wins, key=lambda r: r['raw'])[:10]:
        print('  ', win)


if __name__ == '__main__':
    main()
