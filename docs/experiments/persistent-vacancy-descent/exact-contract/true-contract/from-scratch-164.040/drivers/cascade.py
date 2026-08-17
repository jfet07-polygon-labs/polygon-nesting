#!/usr/bin/env python3
"""The full PERTURB -> {mode 31, mode 26, mode 22} cascade, to a fixpoint.

Each round:
  1. builds the k x d binding-stack nudge grid from the incumbent;
  2. runs the CHEAP arm - mode 31 bounded global legalization - on the
     unperturbed incumbent (control) and on every perturbed state, over a
     ladder of bounds below the incumbent's raw depth;
  3. adopts the deepest exact-valid AND contract-valid publication strictly
     below the incumbent, after a mode-27 replay confirms it independently;
  4. when the cheap arm stops moving, escalates to the EXPENSIVE arms - mode 26
     clamped-sheet ladders and mode 22 alternation - on the control and on the
     perturbed states, and adopts from those the same way;
  5. stops when a full escalated round produces nothing.

Fail-closed everywhere: a run that crashes, fails to validate, or fails its
replay is logged and skipped, never adopted.
"""

import json
import os
import sys
import time

sys.path.insert(0, '/var/lib/t3/tmp/combo')
import base      # noqa: E402
import perturb   # noqa: E402
import sweep     # noqa: E402

WORK = '/var/lib/t3/tmp/combo/cascade'
EPS = (0.006, 0.012, 0.025, 0.05, 0.1)
DROPS = (0.3, 1.0)
LADDER_SEEDS = (0, 1)
ALT_SEEDS = (0, 1, 2, 3)
WORKERS = 4
MAX_ROUNDS = 400


def log(message):
    print(message, flush=True)


def raw_of(path):
    return base.depth_mm(json.load(open(path))['placements'])


def adopt(wins, incumbent_raw, outdir, label):
    """Takes the deepest win that survives an independent mode-27 replay."""
    for raw, depth, tag, run_json in wins:
        if raw >= incumbent_raw - 1e-9:
            continue
        candidate = f'{outdir}/cand-{tag}-{raw:.6f}.json'
        base.pin(run_json, candidate, f'{label} via {tag}')
        replay = base.run(f'replay-{tag}', 27, candidate, '0', 0, f'{outdir}/replay')
        pop = base.population(replay)
        if not (pop and pop.get('exactValid') and pop.get('contractValid')):
            log(f'    REJECTED {tag}: mode-27 replay did not confirm '
                f'({base.line("replay", replay)})')
            continue
        if abs(pop['rawSourceDepthMm'] - raw) > 1e-9:
            log(f'    REJECTED {tag}: replay measured {pop["rawSourceDepthMm"]} != {raw}')
            continue
        return candidate, raw, depth, tag
    return None


def cheap_jobs(incumbent, cases, incumbent_raw):
    jobs = [(f'ctl-b{e}', 31, incumbent, f'{incumbent_raw - e:.6f}', 0) for e in EPS]
    for tag, path, _ in cases:
        jobs += [(f'{tag}-b{e}', 31, path, f'{incumbent_raw - e:.6f}', 0) for e in EPS]
    return jobs


def expensive_jobs(incumbent, cases, incumbent_raw):
    jobs = [(f'ctl-alt-s{s}', 22, incumbent, f'{incumbent_raw + 0.8:.6f}', s)
            for s in ALT_SEEDS]
    jobs += [(f'ctl-lad-{d}-s{s}', 26, incumbent, f'{incumbent_raw - d:.6f}', s)
             for d in DROPS for s in LADDER_SEEDS]
    for tag, path, _ in cases:
        jobs += [(f'{tag}-alt-s{s}', 22, path, f'{incumbent_raw + 0.8:.6f}', s)
                 for s in (0, 1)]
        jobs += [(f'{tag}-lad-{d}-s0', 26, path, f'{incumbent_raw - d:.6f}', 0)
                 for d in DROPS]
    return jobs


def main():
    incumbent = sys.argv[1]
    work = sys.argv[2] if len(sys.argv) > 2 else WORK
    os.makedirs(work, exist_ok=True)
    incumbent_raw = raw_of(incumbent)
    log(f'cascade start {incumbent_raw:.6f} from {incumbent}')
    trajectory = [(incumbent_raw, 'parent', incumbent)]
    escalated_and_stuck = False

    for index in range(MAX_ROUNDS):
        outdir = f'{work}/round{index}'
        cases = perturb.build(incumbent, f'{outdir}/pert')
        started = time.time()

        log(f'round {index}: cheap arm (mode 31) at {incumbent_raw:.6f}')
        results = sweep.sweep(cheap_jobs(incumbent, cases, incumbent_raw),
                              f'{outdir}/m31', workers=WORKERS)
        wins = sorted((base.population(r)['rawSourceDepthMm'], base.published(r), t, r)
                      for t, r in results if base.published(r) is not None
                      and base.population(r)['rawSourceDepthMm'] < incumbent_raw - 1e-9)
        taken = adopt(wins, incumbent_raw, outdir, f'cascade round {index} mode 31')
        if taken is None and wins:
            log(f'    all {len(wins)} cheap wins failed replay')
        if taken is not None:
            incumbent, incumbent_raw, depth, tag = taken
            trajectory.append((incumbent_raw, f'r{index} m31 {tag}', incumbent))
            log(f'  ADOPT {tag}: raw {incumbent_raw:.6f} (published {depth}) '
                f'[{len(wins)} wins, {time.time() - started:.0f}s]')
            escalated_and_stuck = False
            continue

        log(f'  cheap arm fixpoint at {incumbent_raw:.6f}; escalating')
        results = sweep.sweep(expensive_jobs(incumbent, cases, incumbent_raw),
                              f'{outdir}/heavy', workers=WORKERS)
        wins = sorted((base.population(r)['rawSourceDepthMm'], base.published(r), t, r)
                      for t, r in results if base.published(r) is not None
                      and base.population(r)['rawSourceDepthMm'] < incumbent_raw - 1e-9)
        for t, r in results:
            log(f'    {base.line(t, r)}')
        taken = adopt(wins, incumbent_raw, outdir, f'cascade round {index} heavy')
        if taken is not None:
            incumbent, incumbent_raw, depth, tag = taken
            trajectory.append((incumbent_raw, f'r{index} heavy {tag}', incumbent))
            log(f'  ADOPT {tag}: raw {incumbent_raw:.6f} (published {depth}) '
                f'[{len(wins)} wins, {time.time() - started:.0f}s]')
            escalated_and_stuck = False
            continue

        log(f'FIXPOINT at raw {incumbent_raw:.6f} after {index} rounds')
        escalated_and_stuck = True
        break

    log('TRAJECTORY')
    for raw, how, path in trajectory:
        log(f'  {raw:.6f}  {how}  {path}')
    json.dump({'fixpoint': escalated_and_stuck, 'incumbent': incumbent,
               'incumbentRaw': incumbent_raw,
               'trajectory': [{'raw': r, 'via': h, 'path': p} for r, h, p in trajectory]},
              open(f'{work}/trajectory.json', 'w'), indent=1)
    log(f'FINAL {incumbent} raw={incumbent_raw}')


if __name__ == '__main__':
    main()
