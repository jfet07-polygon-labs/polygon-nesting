#!/usr/bin/env python3
"""Joint mode-22 / mode-26 cascade loop to a fixpoint of both mechanisms.

The branch's established pattern: alternation (mode 22) and clamped-ladder
compression (mode 26, now carrying the global legalization tier) lock on
different things, so a state is only a fixpoint when neither moves it.
"""

import json
import os
import subprocess
import sys

sys.path.insert(0, '/var/lib/t3/tmp/mode31')
from run import ARGS, REQ  # noqa: E402
from pin import extract  # noqa: E402

BIN = '/var/lib/t3/tmp/mode31-bench'
OUT = sys.argv[2] if len(sys.argv) > 2 else '/var/lib/t3/tmp/mode31/joint'
DROPS = (0.3, 0.55, 1.0, 1.6)
SEEDS = (0, 1)
ALT_SEEDS = (0, 1, 2, 3)
MAX_ROUNDS = 20


def run(tag, mode, parent, target, seed):
    os.makedirs(OUT, exist_ok=True)
    path = f'{OUT}/{tag}.json'
    if not os.path.exists(path):
        argv = [BIN, REQ] + [a.format(clamp='0', seed=seed) for a in ARGS] + [
            str(mode), parent, f'{target:.6f}', '', '0.0005']
        with open(path, 'w') as out:
            subprocess.run(argv, stdout=out, stderr=subprocess.DEVNULL, check=False)
    try:
        with open(path) as handle:
            return json.load(handle)
    except Exception:
        return None


def published(run_json):
    if run_json is None:
        return None
    try:
        pop = (run_json['relaxedDiagnostics']['coupledDynamicSeparator']
               ['persistentVacancyPopulation'])
    except (KeyError, TypeError):
        return None
    if not pop.get('exactValid') or not pop.get('contractValid'):
        return None
    return pop.get('independentDepthMm')


def main():
    parent = sys.argv[1]
    with open(parent) as handle:
        incumbent = json.load(handle)['independentDepthMm']
    print(f'start {incumbent} from {parent}', flush=True)
    for index in range(MAX_ROUNDS):
        best = None
        arms = ([(f'r{index}-lad{drop}-s{seed}', 26, incumbent - drop, seed)
                 for drop in DROPS for seed in SEEDS]
                + [(f'r{index}-alt-s{seed}', 22, incumbent + 0.8, seed)
                   for seed in ALT_SEEDS])
        for tag, mode, target, seed in arms:
            depth = published(run(tag, mode, parent, target, seed))
            mark = ''
            if depth is not None and depth < incumbent - 1e-9:
                mark = ' <-- improves'
                if best is None or depth < best[0]:
                    best = (depth, tag)
            print(f'  round {index} mode {mode} target {target:.3f} seed {seed}: '
                  f'{depth}{mark}', flush=True)
        if best is None:
            print(f'FIXPOINT at {incumbent} after {index} rounds', flush=True)
            return
        depth, tag = best
        parent = f'{OUT}/pinned-{depth:.3f}.json'
        extract(f'{OUT}/{tag}.json', parent,
                f'joint mode-22/mode-26 cascade with the global legalization tier, '
                f'from {incumbent}')
        incumbent = depth
        print(f'round {index}: incumbent -> {incumbent}', flush=True)


if __name__ == '__main__':
    main()
