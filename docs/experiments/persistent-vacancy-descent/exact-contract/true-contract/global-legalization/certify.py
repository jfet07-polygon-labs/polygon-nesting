#!/usr/bin/env python3
"""Certify a pinned state: replay validation plus the established fixpoint arms."""

import json
import os
import subprocess
import sys

sys.path.insert(0, '/var/lib/t3/tmp/mode31')
from run import ARGS, REQ  # noqa: E402

BIN = '/var/lib/t3/tmp/mode31-bench'
OUT = '/var/lib/t3/tmp/mode31/certify'


def run(tag, mode, parent, target, seed):
    os.makedirs(OUT, exist_ok=True)
    path = f'{OUT}/{tag}.json'
    if not os.path.exists(path):
        argv = [BIN, REQ] + [a.format(clamp='0', seed=seed) for a in ARGS] + [
            str(mode), parent, str(target), '', '0.0005']
        with open(path, 'w') as out:
            subprocess.run(argv, stdout=out, stderr=subprocess.DEVNULL, check=False)
    with open(path) as handle:
        return json.load(handle)


def pop(run_json):
    return (run_json['relaxedDiagnostics']['coupledDynamicSeparator']
            .get('persistentVacancyPopulation'))


def main():
    parent = sys.argv[1]
    with open(parent) as handle:
        fixture = json.load(handle)
    depth = fixture['independentDepthMm']
    print(f'certifying {parent} at {depth}')

    for mode, tag in ((27, 'replay27'), (30, 'replay30')):
        p = pop(run(f'{tag}', mode, parent, '0', 0))
        diag = p.get('microLegalization') or p.get('globalLegalization')
        print(f'  mode {mode}: exactValid={p["exactValid"]} '
              f'contractValid={p["contractValid"]} '
              f'depth={p["independentDepthMm"]} raw={p["rawSourceDepthMm"]} '
              f'pairs={diag.get("violatingPairsBefore")} '
              f'boundary={diag.get("boundaryPiecesBefore")}')

    print('  mode 22 alternation:')
    for seed in (0, 1, 2, 3):
        p = pop(run(f'alt-s{seed}', 22, parent, f'{depth + 0.8:.6f}', seed))
        print(f'    seed {seed}: exactValid={p["exactValid"]} '
              f'published={p.get("independentDepthMm")}')

    print('  mode 26 ladders (global tier armed):')
    for drop in (0.3, 0.55, 1.0):
        for seed in (0, 1):
            p = pop(run(f'lad-{drop}-s{seed}', 26, parent, f'{depth - drop:.6f}', seed))
            print(f'    bound {depth - drop:.3f} seed {seed}: '
                  f'exactValid={p["exactValid"]} published={p.get("independentDepthMm")}')

    print('  mode 28 / 29 local repair tiers:')
    for mode in (28, 29):
        p = pop(run(f'local{mode}', mode, parent, f'{depth:.6f}', 0))
        print(f'    mode {mode}: exactValid={p["exactValid"]} '
              f'published={p.get("independentDepthMm")} '
              f'{(p.get("failureReason") or "")[:70]}')


if __name__ == '__main__':
    main()
