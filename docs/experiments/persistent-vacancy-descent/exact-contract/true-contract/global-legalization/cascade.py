#!/usr/bin/env python3
"""Cascade the mode-26 ladder with the global tier armed.

From a pinned parent, run ladders at a small grid of relative bounds and seeds;
take the deepest exact-valid publication strictly below the incumbent, pin it,
and repeat until a whole round produces nothing. Fail-closed: a run that
crashes or fails to validate is skipped, never accepted.
"""

import json
import os
import subprocess
import sys

sys.path.insert(0, '/var/lib/t3/tmp/mode31')
from run import ARGS, REQ  # noqa: E402
from pin import extract  # noqa: E402

BIN = '/var/lib/t3/tmp/mode31-bench'
OUT = '/var/lib/t3/tmp/mode31/cascade'
# Relative rung depths below the incumbent, and the seeds each is drawn at.
DROPS = (0.55, 1.0, 0.3)
SEEDS = (0, 1)
MAX_ROUNDS = 12


def run(tag, parent, bound, seed):
    os.makedirs(OUT, exist_ok=True)
    path = f'{OUT}/{tag}.json'
    if not os.path.exists(path):
        argv = [BIN, REQ] + [a.format(clamp='0', seed=seed) for a in ARGS] + [
            '26', parent, f'{bound:.6f}', '', '0.0005']
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
    incumbent_path = sys.argv[1]
    with open(incumbent_path) as handle:
        incumbent = json.load(handle)['independentDepthMm']
    print(f'start incumbent {incumbent} from {incumbent_path}', flush=True)
    for round_index in range(MAX_ROUNDS):
        best = None
        for drop in DROPS:
            for seed in SEEDS:
                bound = incumbent - drop
                tag = f'r{round_index}-d{drop}-s{seed}'
                depth = published(run(tag, incumbent_path, bound, seed))
                mark = ''
                if depth is not None and depth < incumbent - 1e-9:
                    mark = ' <-- improves'
                    if best is None or depth < best[0]:
                        best = (depth, tag)
                print(f'  round {round_index} bound {bound:.3f} seed {seed}: '
                      f'published {depth}{mark}', flush=True)
        if best is None:
            print(f'round {round_index}: no improvement, cascade stops at {incumbent}',
                  flush=True)
            return
        depth, tag = best
        incumbent_path = f'{OUT}/pinned-{depth:.3f}.json'
        extract(f'{OUT}/{tag}.json', incumbent_path,
                f'mode 26 ladder with the global legalization tier, from {incumbent}')
        incumbent = depth
        print(f'round {round_index}: incumbent -> {incumbent}', flush=True)


if __name__ == '__main__':
    main()
