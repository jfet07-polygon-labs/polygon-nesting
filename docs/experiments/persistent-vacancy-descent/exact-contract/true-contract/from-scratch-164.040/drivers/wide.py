#!/usr/bin/env python3
"""Wide cascade, corrected for what the engine actually accepts.

Measured fact from cascade round 2: modes 22 and 26 both call
`validate_and_measure_placements` on the pinned parent before doing anything
(general_relaxed.rs 3540 in `run_alternation_fixpoint`, 3832 in
`run_ladder_compression`) and return `persistent vacancy parent validation: ...`
on the first overlapping pair. A perturbed state is overlapping BY
CONSTRUCTION, so PERTURB -> mode 26 and PERTURB -> mode 22 are refused at the
door, 48/48 runs, with `attempted=false`. Only mode 31 - whose whole contract is
"make an infeasible layout feasible" - accepts one.

So the working shape of the combination is:

    PERTURB -> mode 31  (the only tier that will take the perturbed state)
            -> pinned legal state
            -> mode 26 ladders / mode 22 alternation  (on the legal state)
            -> re-perturb

which is what this driver runs, at the record line's own ladder scale.
"""

import json
import os
import sys
import time

sys.path.insert(0, '/var/lib/t3/tmp/combo')
import base      # noqa: E402
import perturb   # noqa: E402
import sweep     # noqa: E402
import cascade   # noqa: E402

# Bound offsets below the incumbent's raw depth for mode 31.
EPS = (0.003, 0.006, 0.012, 0.02, 0.025, 0.035, 0.05, 0.075, 0.1, 0.15)
# Ladder drops for mode 26 on the legal incumbent, at the record line's scale
# (record-159.150 stepped on drops of 0.5 - 1.6 mm).
DROPS = (0.3, 0.55, 1.0, 1.6, 2.5, 4.0)
LADDER_SEEDS = (0, 1)
ALT_SEEDS = (0, 1, 2, 3)
WORKERS = 4
MAX_ROUNDS = 300


def log(message):
    print(message, flush=True)


def jobs_for(incumbent, cases, raw):
    # Legal incumbent: every tier is admissible.
    jobs = [(f'ctl-b{e}', 31, incumbent, f'{raw - e:.6f}', 0) for e in EPS]
    jobs += [(f'ctl-alt-s{s}', 22, incumbent, f'{raw + 0.8:.6f}', s) for s in ALT_SEEDS]
    jobs += [(f'ctl-lad-{d}-s{s}', 26, incumbent, f'{raw - d:.6f}', s)
             for d in DROPS for s in LADDER_SEEDS]
    # Perturbed states: mode 31 only - the other two refuse them.
    for tag, path, _ in cases:
        jobs += [(f'{tag}-b{e}', 31, path, f'{raw - e:.6f}', 0) for e in EPS]
    return jobs


def main():
    incumbent = sys.argv[1]
    work = sys.argv[2]
    os.makedirs(work, exist_ok=True)
    raw = cascade.raw_of(incumbent)
    log(f'wide cascade start {raw:.6f} from {incumbent}')
    trajectory = [(raw, 'start', incumbent)]
    for index in range(MAX_ROUNDS):
        outdir = f'{work}/round{index}'
        cases = perturb.build(incumbent, f'{outdir}/pert')
        jobs = jobs_for(incumbent, cases, raw)
        started = time.time()
        log(f'round {index} at {raw:.6f}: {len(jobs)} runs')
        results = sweep.sweep(jobs, f'{outdir}/runs', workers=WORKERS)
        wins = sorted((base.population(r)['rawSourceDepthMm'], base.published(r), t, r)
                      for t, r in results if base.published(r) is not None
                      and base.population(r)['rawSourceDepthMm'] < raw - 1e-9)
        log(f'  {len(wins)} improving publications of {len(results)} runs '
            f'({time.time() - started:.0f}s)')
        for w in wins[:10]:
            log(f'    win {w[0]:.6f} {w[2]}')
        taken = cascade.adopt(wins, raw, outdir, f'wide round {index}')
        if taken is None:
            if wins:
                log('  every win failed its replay')
            log(f'WIDE FIXPOINT at raw {raw:.6f} after {index} rounds')
            for tag, run_json in results:
                log(f'    {base.line(tag, run_json)}')
            break
        incumbent, raw, depth, tag = taken
        trajectory.append((raw, f'r{index} {tag}', incumbent))
        log(f'  ADOPT {tag}: raw {raw:.6f} (published {depth})')
    log('TRAJECTORY')
    for r, h, p in trajectory:
        log(f'  {r:.6f}  {h}  {p}')
    json.dump({'incumbent': incumbent, 'incumbentRaw': raw,
               'trajectory': [{'raw': r, 'via': h, 'path': p} for r, h, p in trajectory]},
              open(f'{work}/trajectory.json', 'w'), indent=1)
    log(f'FINAL {incumbent} raw={raw}')


if __name__ == '__main__':
    main()
