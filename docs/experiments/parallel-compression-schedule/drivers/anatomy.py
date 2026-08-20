#!/usr/bin/env python3
"""The m34 slice's own wall anatomy, and the occupancy it achieves.

    python3 anatomy.py BINARY OUTDIR SPEC [SEEDS] [LABEL]

One mode-34 arm per seed from a pinned parent, run under `/usr/bin/time` so the
process's CPU-seconds are measured rather than inferred. Three things come out
that decide whether an intra-arm parallel schedule can pay:

  * `repairMs` / `confirmationMs` - the schedule's own decomposition of its
    slice, reported by the engine. The repair half is what eight workers could
    share; the confirmation half is one whole-layout exact validation at a time
    and is Amdahl's serial term unless it is parallelised too.
  * the per-step work distribution - `sweepsRun` and `candidateQueries` per
    step. A step that repairs nothing has nothing to spread over eight
    workers, and the median step is the one that decides the multiplier.
  * process occupancy - CPU-seconds / wall-seconds over the whole process,
    which is the measurement `compression-schedule` README section 6.3 asserts
    ("the schedule is one lane; the mode-26 pipeline is eight") without
    putting a number on it. A mode-26 arm is run in the same shape as the
    control so the two occupancies are read off the same instrument.
"""
import json
import os
import re
import statistics
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import runlib  # noqa: E402

PARENTS = '/var/lib/t3/tmp/csched/parents'
# GNU time, wherever this host keeps it. It is the instrument for occupancy:
# `Percent of CPU` over the whole process is CPU-seconds / wall-seconds, which
# is the average number of cores the arm actually kept busy.
TIME_BIN = next((p for p in ('/usr/bin/time', '/run/current-system/sw/bin/time')
                 if os.path.exists(p)), 'time')


def run_arm(binary, seed, fixture, target, mode, spec, out_path, profile=True):
    args = [a.format(seed=seed) for a in runlib.ARGS]
    tail = [str(mode), fixture, f'{target:.17g}', '', runlib.DEFAULT_ALLOWANCE]
    command = [binary, runlib.REQUESTS['mixed-61']] + args + tail
    env = dict(os.environ)
    if profile:
        env['POLYGON_NESTING_PROFILE'] = '1'
    env.pop('POLYGON_NESTING_COMPRESSION_SCHEDULE', None)
    if spec:
        env['POLYGON_NESTING_COMPRESSION_SCHEDULE'] = spec
    os.makedirs(os.path.dirname(out_path), exist_ok=True)
    timed = [TIME_BIN, '-v'] + command
    started = time.monotonic()
    with open(out_path, 'w') as handle:
        proc = subprocess.run(timed, stdout=handle, stderr=subprocess.PIPE,
                              check=False, env=env)
    wall = time.monotonic() - started
    stderr = (proc.stderr or b'').decode()
    usage = {}
    for key, field in (('User time', 'userSeconds'),
                       ('System time', 'systemSeconds'),
                       ('Percent of CPU', 'cpuPercent'),
                       ('Elapsed (wall clock) time', 'timeWall'),
                       ('Maximum resident set size', 'maxRssKb')):
        match = re.search(rf'{re.escape(key)}[^:]*:\s*([0-9.:%]+)', stderr)
        if match:
            usage[field] = match.group(1)
    try:
        doc = json.load(open(out_path))
    except json.JSONDecodeError:
        return None, wall, usage, stderr[-800:]
    return doc, wall, usage, stderr[-800:]


def population(doc):
    return ((doc.get('relaxedDiagnostics') or {})
            .get('coupledDynamicSeparator') or {}).get(
                'persistentVacancyPopulation')


def summarise(doc, wall, usage):
    row = {'processWallSeconds': wall, 'usage': usage}
    cpu = usage.get('cpuPercent', '').rstrip('%')
    try:
        row['occupancyLanes'] = float(cpu) / 100.0
    except ValueError:
        row['occupancyLanes'] = None
    profile = (doc.get('searchProfile') or {}).get('counters') or {}
    row['processCandidateQueries'] = profile.get('candidateQueries', 0)
    row['processExactPairTests'] = profile.get('exactPairTests', 0)
    row['processWorkUnits'] = (row['processCandidateQueries']
                               + 5 * row['processExactPairTests'])
    pop = population(doc)
    if pop is None:
        return row
    row['rawSourceDepthMm'] = pop.get('rawSourceDepthMm')
    row['parentIndependentDepthMm'] = pop.get('parentIndependentDepthMm')
    row['exactValid'] = pop.get('exactValid')
    schedule = pop.get('compressionSchedule')
    if not schedule:
        return row
    steps = schedule.get('steps') or []
    row['schedule'] = {k: v for k, v in schedule.items() if k != 'steps'}
    queries = [s['candidateQueries'] for s in steps]
    sweeps = [s['sweepsRun'] for s in steps]
    pairs = [s['collisionPairsBefore'] for s in steps]
    row['stepAnatomy'] = {
        'steps': len(steps),
        'stepsWithZeroSweeps': sum(1 for s in sweeps if s == 0),
        'stepsWithZeroQueries': sum(1 for q in queries if q == 0),
        'medianQueriesPerStep': statistics.median(queries) if queries else 0,
        'meanQueriesPerStep': statistics.fmean(queries) if queries else 0,
        'maxQueriesPerStep': max(queries, default=0),
        'medianSweepsPerStep': statistics.median(sweeps) if sweeps else 0,
        'meanSweepsPerStep': statistics.fmean(sweeps) if sweeps else 0,
        'medianCollisionPairsBefore': statistics.median(pairs) if pairs else 0,
        'meanCollisionPairsBefore': statistics.fmean(pairs) if pairs else 0,
        'maxCollisionPairsBefore': max(pairs, default=0),
        # Where the queries actually are: the share of all candidate queries
        # spent in the top decile of steps by query count. A schedule whose
        # work is concentrated in a few hard steps parallelises differently
        # from one that spreads it evenly.
        'queryShareTopDecile': (
            sum(sorted(queries, reverse=True)[:max(1, len(queries) // 10)])
            / sum(queries)) if sum(queries) else 0,
    }
    total_ms = (schedule.get('repairMs') or 0) + (
        schedule.get('confirmationMs') or 0)
    row['sliceMs'] = total_ms
    row['repairShare'] = ((schedule.get('repairMs') or 0) / total_ms
                          if total_ms else None)
    row['confirmationShare'] = ((schedule.get('confirmationMs') or 0)
                                / total_ms if total_ms else None)
    return row


def main():
    binary = sys.argv[1]
    outdir = sys.argv[2]
    spec = sys.argv[3]
    seeds = [int(s) for s in (sys.argv[4] if len(sys.argv) > 4
                              else '0,1,2').split(',')]
    label = sys.argv[5] if len(sys.argv) > 5 else 'm34'
    mode = int(os.environ.get('ANATOMY_MODE', '34'))
    drop = float(os.environ.get('ANATOMY_DROP_MM', '0.3'))
    # Occupancy is a clock measurement, so it is taken on an unprofiled build
    # path: `POLYGON_NESTING_PROFILE=1` adds spans to the hottest loops in the
    # engine and would be measuring the instrument.
    profile = os.environ.get('ANATOMY_PROFILE', '1') != '0'
    parents = json.load(open(f'{PARENTS}/parents.json'))['rows']
    by_seed = {p['seed']: p for p in parents}
    result = {'binary': binary, 'spec': spec, 'mode': mode, 'label': label,
              'dropMm': drop, 'rows': []}
    for seed in seeds:
        parent = by_seed[seed]
        target = parent['rawDepthMm'] - drop
        path = f'{outdir}/{label}-seed{seed}.json'
        doc, wall, usage, err = run_arm(
            binary, seed, parent['fixture'], target, mode, spec, path,
            profile=profile)
        if doc is None:
            result['rows'].append({'seed': seed, 'error': err})
            continue
        row = summarise(doc, wall, usage)
        row['seed'] = seed
        row['parentRawDepthMm'] = parent['rawDepthMm']
        result['rows'].append(row)
        os.makedirs(outdir, exist_ok=True)
        json.dump(result, open(f'{outdir}/anatomy-{label}.json', 'w'), indent=1)
    print(json.dumps(result, indent=1))


if __name__ == '__main__':
    main()
