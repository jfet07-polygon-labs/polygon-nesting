#!/usr/bin/env python3
"""Phase C: joint cascade to a fixpoint of all four arms.

Arms per round, all against the current incumbent fixture:
  * mode 31 direct, at a ladder of depth targets;
  * mode 31 on PERTURBED copies of the incumbent (the combination under test);
  * mode 26 clamped-sheet ladders at incumbent - {0.3, 0.55, 1.0, 1.6}, seeds 0/1;
  * mode 22 alternation at incumbent + 0.8, seeds 0..3.

The round adopts the deepest exact-valid publication from any arm and repeats.
A round in which no arm publishes below the incumbent is the fixpoint.

usage: phase_c.py <parent-fixture> <outdir> [max-rounds]
"""

import json
import os
import sys
import time

import lib

DROPS = (0.3, 0.55, 1.0, 1.6)
SEEDS = (0, 1)
ALT_SEEDS = (0, 1, 2, 3)
M31_STEPS = (0.10, 0.06, 0.04, 0.02, 0.01, 0.005)
CELLS = [(k, d) for k in (2, 3, 4, 6) for d in (1.0, 2.0, 3.5)]


def published(run_json):
    pop = lib.population(run_json)
    if pop is None or not pop.get('exactValid') or not pop.get('contractValid'):
        return None, pop
    return pop.get('rawSourceDepthMm'), pop


def main():
    parent = sys.argv[1]
    out = sys.argv[2]
    max_rounds = int(sys.argv[3]) if len(sys.argv) > 3 else 8
    os.makedirs(out, exist_ok=True)
    incumbent = lib.depth_mm(json.load(open(parent))['placements'])
    print(f'start {incumbent!r} from {parent}', flush=True)
    trajectory = [{'raw': incumbent, 'via': 'parent', 'fixture': parent}]
    for index in range(max_rounds):
        best = None
        placements = json.load(open(parent))['placements']

        def consider(tag, run_json, label):
            nonlocal best
            depth, pop = published(run_json)
            mark = ''
            if depth is not None and depth < incumbent - 1e-9:
                mark = ' <-- improves'
                if best is None or depth < best[0]:
                    best = (depth, run_json, label)
            print(f'  r{index} {tag}: exactValid={(pop or {}).get("exactValid")} '
                  f'raw={depth}{mark} '
                  f'{((pop or {}).get("failureReason") or "")[:70]}', flush=True)

        # mode 31 direct
        for step in M31_STEPS:
            target = incumbent - step
            tag = f'r{index}-m31-direct-s{step}'
            consider(tag, lib.run(tag, 31, parent, f'{target + lib.BOUND_OFFSET_MM:.6f}',
                                  0, out), f'mode31 direct step {step}')
        # mode 22 alternation
        for seed in ALT_SEEDS:
            tag = f'r{index}-m22-s{seed}'
            consider(tag, lib.run(tag, 22, parent, f'{incumbent + 0.8:.6f}', seed, out),
                     f'mode22 seed {seed}')
        # mode 26 ladders
        for drop in DROPS:
            for seed in SEEDS:
                tag = f'r{index}-m26-{drop}-s{seed}'
                consider(tag, lib.run(tag, 26, parent, f'{incumbent - drop:.6f}',
                                      seed, out), f'mode26 drop {drop} seed {seed}')
        # mode 31 on perturbed copies: the combination under test
        for k, d in CELLS:
            fixture = f'{out}/r{index}-nudge-k{k}-d{d}.json'
            lib.write_fixture(fixture, f'pv-combo nudge k{k} d{d}',
                              lib.nudge(placements, k, d), reported_depth_mm=incumbent)
            for step in M31_STEPS:
                target = incumbent - step
                tag = f'r{index}-m31-k{k}-d{d}-s{step}'
                consider(tag, lib.run(tag, 31, fixture,
                                      f'{target + lib.BOUND_OFFSET_MM:.6f}', 0, out),
                         f'mode31 nudge k{k} d{d} step {step}')

        if best is None:
            print(f'FIXPOINT at {incumbent!r} after {index + 1} rounds', flush=True)
            break
        incumbent, run_json, label = best
        parent = f'{out}/pinned-{incumbent:.6f}.json'
        lib.pin(run_json, parent, f'pv-combo joint cascade via {label}')
        trajectory.append({'raw': incumbent, 'via': label, 'fixture': parent})
        json.dump(trajectory, open(f'{out}/trajectory.json', 'w'), indent=1)
        print(f'round {index}: incumbent -> {incumbent!r} via {label}', flush=True)
    json.dump(trajectory, open(f'{out}/trajectory.json', 'w'), indent=1)
    print(f'BEST {incumbent!r} at {parent}', flush=True)


if __name__ == '__main__':
    main()
