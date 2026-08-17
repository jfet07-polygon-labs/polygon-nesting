#!/usr/bin/env python3
"""Phase B: the mode-31 descent cascade with perturbation escalation.

Each iteration asks bounded global legalization for a state strictly below the
incumbent. The direct route runs mode 31 on the incumbent itself at a ladder of
targets, largest step first. When every direct step refuses, the PERTURB
escalation fires: the k deepest pieces by true transformed max-Y are nudged d mm
into the packed body and the infeasible state is handed to mode 31 at the same
targets, which is the mechanism combination under test.

Every accepted state is re-pinned from the run's own published placements, so
the next iteration's parent is a fixture the harness re-derives and re-validates
on load.
"""

import json
import os
import sys
import time

import lib

OUT = '/var/lib/t3/tmp/combo/descent'
STEPS = [0.15, 0.10, 0.06, 0.04, 0.02, 0.01, 0.005]
CELLS = [(k, d) for k in (2, 3, 4, 6) for d in (1.0, 2.0, 3.5)]
BUDGET_S = float(os.environ.get('BUDGET_S', '3000'))


def probe(tag, fixture, target):
    result = lib.run(tag, 31, fixture, f'{target + lib.BOUND_OFFSET_MM:.6f}', 0, OUT)
    pop = lib.population(result) or {}
    return result, pop


def main():
    os.makedirs(OUT, exist_ok=True)
    started = time.time()
    current_fixture = lib.PARENT
    current_placements = json.load(open(lib.PARENT))['placements']
    current_raw = lib.depth_mm(current_placements)
    trajectory = [{'raw': current_raw, 'via': 'parent', 'fixture': current_fixture}]
    print(f'start {current_raw!r}')
    iteration = 0
    while time.time() - started < BUDGET_S:
        iteration += 1
        moved = False
        # Direct route: mode 31 on the incumbent.
        for step in STEPS:
            target = current_raw - step
            tag = f'i{iteration:03d}-direct-s{step}'
            run_json, pop = probe(tag, current_fixture, target)
            ok = pop.get('exactValid') and pop.get('rawSourceDepthMm') is not None \
                and pop['rawSourceDepthMm'] < current_raw - 1e-9
            print(f'  {tag}: target={target:.6f} exactValid={pop.get("exactValid")} '
                  f'raw={pop.get("rawSourceDepthMm")} '
                  f'{(pop.get("failureReason") or "")[:60]}')
            sys.stdout.flush()
            if ok:
                current_raw = pop['rawSourceDepthMm']
                current_fixture = lib.pin(
                    run_json, f'{OUT}/state-{iteration:03d}-{current_raw:.6f}.json',
                    f'mode-31 direct descent step {step} to {current_raw:.6f}')
                current_placements = json.load(open(current_fixture))['placements']
                trajectory.append({'raw': current_raw, 'via': f'mode31 direct step {step}',
                                   'fixture': current_fixture})
                json.dump(trajectory, open(f'{OUT}/trajectory.json', 'w'), indent=1)
                print(f'  ACCEPT {current_raw:.6f} via direct step {step}')
                moved = True
                break
        if moved:
            continue
        # PERTURB escalation.
        best = None
        for k, d in CELLS:
            perturbed = lib.nudge(current_placements, k, d)
            fixture = f'{OUT}/i{iteration:03d}-nudge-k{k}-d{d}.json'
            lib.write_fixture(fixture, f'pv-combo nudge k{k} d{d}', perturbed,
                              reported_depth_mm=current_raw)
            for step in STEPS:
                target = current_raw - step
                tag = f'i{iteration:03d}-k{k}-d{d}-s{step}'
                run_json, pop = probe(tag, fixture, target)
                ok = pop.get('exactValid') and pop.get('rawSourceDepthMm') is not None \
                    and pop['rawSourceDepthMm'] < current_raw - 1e-9
                if ok:
                    print(f'  {tag}: raw={pop["rawSourceDepthMm"]:.6f} ACCEPTABLE')
                    if best is None or pop['rawSourceDepthMm'] < best[0]:
                        best = (pop['rawSourceDepthMm'], run_json, f'nudge k{k} d{d} step {step}')
                    break
            sys.stdout.flush()
        if best is None:
            print(f'FIXPOINT at {current_raw!r} after iteration {iteration}: '
                  'no direct step and no perturbation cell moved it')
            break
        current_raw, run_json, via = best
        current_fixture = lib.pin(
            run_json, f'{OUT}/state-{iteration:03d}-{current_raw:.6f}.json',
            f'mode-31 perturbation descent via {via} to {current_raw:.6f}')
        current_placements = json.load(open(current_fixture))['placements']
        trajectory.append({'raw': current_raw, 'via': f'mode31 {via}',
                           'fixture': current_fixture})
        print(f'  ACCEPT {current_raw:.6f} via {via}')
        json.dump(trajectory, open(f'{OUT}/trajectory.json', 'w'), indent=1)
    json.dump(trajectory, open(f'{OUT}/trajectory.json', 'w'), indent=1)
    print(f'\nBEST {current_raw!r} at {current_fixture}')
    print(f'steps: {len(trajectory) - 1}, elapsed {time.time() - started:.0f}s')


if __name__ == '__main__':
    main()
