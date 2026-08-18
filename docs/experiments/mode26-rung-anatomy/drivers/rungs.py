#!/usr/bin/env python3
"""Runs a set of mode-26 ladders under the `search-profiling` build.

    python3 rungs.py <binary> <outdir> [tag ...]

Each ladder is one process: the pinned parent, a final bound at
`parent_raw - drop`, and the pinned `'' 0.0005` CLI tail. The profiling build
fills the mode-26 diagnostics with a per-arm, per-rung and per-ladder wall-clock
anatomy plus the phase and counter deltas of each region; this driver writes the
raw benchmark document per ladder and a flat row table beside it.

Profiling costs about 4.5% of a deep-operator stream, so nothing here is a
wall-clock *claim*: it is a decomposition. The paired A/B is the wall-clock
claim, and this experiment does not make one.
"""
import json
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import lib  # noqa: E402

TRUE = (f'{lib.ROOT}/docs/experiments/persistent-vacancy-descent/exact-contract/'
        'true-contract')
LINEAR_PARENT = f'{TRUE}/finer-ladder/pinned-parent-159.079.json'
LINEAR_RAW = 159.07876040364795
FS_PARENT = f'{TRUE}/finer-ladder/pinned-fs-parent-164.0376.json'
FS_RAW = 164.0375677990678

# tag, parent, parent raw depth, drop (mm), seed
PLAN = []
for drop in (0.3, 0.55, 1.0):
    for seed in (0, 1):
        PLAN.append((f'lin-d{drop}-s{seed}', LINEAR_PARENT, LINEAR_RAW, drop, seed))
for drop in (0.3, 1.0):
    PLAN.append((f'fs-d{drop}-s0', FS_PARENT, FS_RAW, drop, 0))


def ladder_of(doc):
    pop = (doc.get('relaxedDiagnostics', {})
              .get('coupledDynamicSeparator', {})
              .get('persistentVacancyPopulation'))
    if pop is None:
        return None, None
    return pop, pop.get('ladderCompression')


def main():
    binary, outdir = sys.argv[1], sys.argv[2]
    wanted = set(sys.argv[3:])
    os.makedirs(outdir, exist_ok=True)
    rows = []
    for tag, parent, raw, drop, seed in PLAN:
        if wanted and tag not in wanted:
            continue
        target = f'{raw - drop:.6f}'
        started = time.time()
        doc, wall, stderr = lib.run(
            binary, tag, 26, parent, target, '0.0005', outdir,
            env={'POLYGON_NESTING_PROFILE': '1'}, seed=str(seed))
        pop, ladder = ladder_of(doc)
        row = {
            'tag': tag, 'parent': os.path.basename(parent), 'parentRaw': raw,
            'dropMm': drop, 'targetMm': float(target), 'seed': seed,
            'processWallSeconds': wall,
            'engineSeconds': lib.engine_seconds(doc),
            'publishedRaw': (pop or {}).get('rawSourceDepthMm'),
            'independentDepthMm': (pop or {}).get('independentDepthMm'),
            'exactValid': (pop or {}).get('exactValid'),
            'contractValid': (pop or {}).get('contractValid'),
            'failureReason': (pop or {}).get('failureReason'),
            'stepsPlanned': (ladder or {}).get('stepsPlanned'),
            'stepsRun': (ladder or {}).get('stepsRun'),
            'stepMm': (ladder or {}).get('stepMm'),
            'publishedStep': (ladder or {}).get('publishedStep'),
            'ladderWallMs': ((ladder or {}).get('anatomy') or {}).get('wallMs'),
            'searchProfile': doc.get('searchProfile'),
            'stderrTail': stderr[-400:] if stderr else '',
            'elapsed': time.time() - started,
        }
        rows.append(row)
        print(json.dumps({k: v for k, v in row.items()
                          if k not in ('searchProfile', 'stderrTail')}),
              flush=True)
    json.dump(rows, open(f'{outdir}/rows.json', 'w'), indent=1)


if __name__ == '__main__':
    main()
