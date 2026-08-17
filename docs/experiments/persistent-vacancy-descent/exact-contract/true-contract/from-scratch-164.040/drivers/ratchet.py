#!/usr/bin/env python3
"""Inner loop: mode-31 bounded global legalization ratchet.

From a pinned state, run mode 31 at a ladder of bounds below the state's own
raw depth. Adopt the deepest exact-valid AND contract-valid publication that is
strictly below the incumbent, pin it, repeat. Fail-closed: only a publication
the engine itself validated is ever adopted.
"""

import json
import os
import sys

sys.path.insert(0, '/var/lib/t3/tmp/combo')
import base  # noqa: E402

EPS = (0.006, 0.012, 0.025, 0.05, 0.1, 0.2, 0.4)
MAX_ROUNDS = 200


def raw_depth(path):
    return base.depth_mm(json.load(open(path))['placements'])


def ratchet(path, outdir, label, log=print, eps=EPS, max_rounds=MAX_ROUNDS):
    os.makedirs(outdir, exist_ok=True)
    best_path, best_raw = path, raw_depth(path)
    for index in range(max_rounds):
        moved = False
        for step in eps:
            bound = best_raw - step
            tag = f'{label}-r{index}-e{step}'
            out = base.run(tag, 31, best_path, f'{bound:.6f}', 0, outdir)
            depth = base.published(out)
            if depth is None:
                continue
            pop = base.population(out)
            new_raw = pop['rawSourceDepthMm']
            if new_raw < best_raw - 1e-9:
                pinned = f'{outdir}/pin-{new_raw:.6f}.json'
                base.pin(out, pinned, f'mode-31 ratchet {label} round {index} bound {bound:.6f}')
                log(f'  {label} round {index} bound {bound:.4f}: '
                    f'{best_raw:.6f} -> {new_raw:.6f} (published {depth})')
                best_path, best_raw = pinned, new_raw
                moved = True
                break
        if not moved:
            log(f'  {label} ratchet fixpoint at raw {best_raw:.6f} after {index} rounds')
            return best_path, best_raw
    log(f'  {label} ratchet hit round cap at raw {best_raw:.6f}')
    return best_path, best_raw


if __name__ == '__main__':
    start = sys.argv[1]
    outdir = sys.argv[2]
    label = sys.argv[3] if len(sys.argv) > 3 else 'ratchet'
    path, raw = ratchet(start, outdir, label,
                        log=lambda m: print(m, flush=True))
    print(f'RESULT {path} raw={raw}')
