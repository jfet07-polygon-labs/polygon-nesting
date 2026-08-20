#!/usr/bin/env python3
"""The A/B/C campaign Sol review 5 section 3 asks for.

    python3 campaign.py BINARY OUTDIR [ARMS] [WORK] [DROP_MM]

Fifteen parents - the twelve compression-schedule port parents (171.61-179.62
mm) plus the three true-contract pins (156.9188, 156.091, 155.422) - each run
through three arms of mode 34 at equal work:

  `grid`        A: StructuredGrid, today's default. `POLYGON_NESTING_
                CURRENT_POSE_OVERLAY` unset.
  `overlay`     B: StructuredGrid + CurrentPoseOverlay.
                `POLYGON_NESTING_CURRENT_POSE_OVERLAY=1`.
  `directional` C: CurrentAssignment + DirectionalPenetration, the existing
                other engine, for reference. `relaxed-pressure-model` (CLI
                slot 33) set to `directional` instead of `structured`; every
                other argument, including the work budget, held fixed.

Equal work is the schedule's own currency (candidate queries + 5 x exact pair
tests, `work_cap_queries`), not wall-clock: the schedule stops itself at the
same budget on every arm regardless of engine, which is what makes A vs B vs C
a work-budget comparison rather than a wall-clock race on a shared box. Every
run is additionally executed with the process profile armed so
`processWallSeconds` and `searchProfile` are both available for queries/s.

Per arm, per parent, this records exactly what section 2 of the task asks
for: entry loss (`parentBoundaryViolations`/`parentCollisionPairs`/
`parentProxyFeasible`), queries/s, confirmations, publications (an
exact-valid state strictly shallower than the parent), and best raw depth
reached.
"""
import hashlib
import json
import os
import subprocess
import sys
import time

ROOT = '/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_b7992967-b13-2'
REQUEST = f'{ROOT}/tests/fixtures/mixed-61/mixed61-request-exact-clearance.json'
TRUE = (f'{ROOT}/docs/experiments/persistent-vacancy-descent/exact-contract/'
        'true-contract/record-line-cascade')

# Byte-for-byte `compression-schedule/drivers/runlib.py`'s ARGS: the pinned
# CLI tail every compression-schedule run in this repository uses. Slot 25 is
# the relaxed seed (fixed at 5, matching the gate's own replay seed); slot 33
# is the relaxed pressure model, which arm `directional` overrides.
ARGS = ('1 4 0 0 0 0 1 0 0 1 1 0 16 4 8 0 0 5 5 24 8 40 10 10 5 5 '
        '0.005 0.001 1 6 0 0 0 {pressure} 0 10 1 0 0 0 0').split()
DEFAULT_ALLOWANCE = '0.002'
DEFAULT_DROP_MM = 0.3
# The anatomy's design slice - 0.5-1.0 s of a mode-26 rung - reused here as
# the equal-work budget so this campaign's own claim transfers to the
# ten-second envelope the binding priority names, rather than an offline
# record-chasing budget.
DESIGN_SLICE_UNITS = 3_341_379

# The schedule configuration every arm runs, spelled out rather than defaulted.
#
# Sol review 6 §2.3 rejected the v5 round's campaign because it ran
# `rollback=32`: the compression-schedule port had *already certified* that
# arming the rollback costs a median 11.75 mm of published depth (12/12 publish
# without it, 8/12 with it), and `CompressionScheduleSettings::default()` sets
# `rollback_after_steps = 0` for that reason. The paired arms made the *entry*
# measurement survive, but the downstream 12/15-vs-9/15 claim was measured on a
# configuration that does not ship and does not transfer to one that does.
#
# These are coordinator v4's own settings (`docs/experiments/coordinator-v4/
# README.md` §1.1: six repair sweeps per step, a confirmation every fourth
# step, `micro_legalize` on a refused confirmation, one canonical grid unit per
# step, and `rollback_after_steps = 0`), which are also the schedule's
# defaults. Written out in full so a reader can see the whole configuration
# instead of inferring it from what is absent.
SCHEDULE_V4 = 'sweeps=6,confirm=4,rollback=0,repair=micro,step=1,past=1'

ARMS = {
    'grid': {'pressure': 'structured', 'overlay': False},
    'overlay': {'pressure': 'structured', 'overlay': True},
    'directional': {'pressure': 'directional', 'overlay': False},
}

PORT_PARENTS = [
    {'name': 'port-seed0', 'fixture': '/var/lib/t3/tmp/csched/parents/parent-seed0.json', 'depthMm': 174.20812003998896},
    {'name': 'port-seed1', 'fixture': '/var/lib/t3/tmp/csched/parents/parent-seed1.json', 'depthMm': 176.05599999999998},
    {'name': 'port-seed2', 'fixture': '/var/lib/t3/tmp/csched/parents/parent-seed2.json', 'depthMm': 179.006},
    {'name': 'port-seed3', 'fixture': '/var/lib/t3/tmp/csched/parents2/parent-seed3.json', 'depthMm': 176.061},
    {'name': 'port-seed4', 'fixture': '/var/lib/t3/tmp/csched/parents2/parent-seed4.json', 'depthMm': 171.64953207726535},
    {'name': 'port-seed5', 'fixture': '/var/lib/t3/tmp/csched/parents2/parent-seed5.json', 'depthMm': 179.05182605364416},
    {'name': 'port-seed6', 'fixture': '/var/lib/t3/tmp/csched/parents2/parent-seed6.json', 'depthMm': 179.6200102363703},
    {'name': 'port-seed7', 'fixture': '/var/lib/t3/tmp/csched/parents2/parent-seed7.json', 'depthMm': 179.52233303152792},
    {'name': 'port-seed8', 'fixture': '/var/lib/t3/tmp/csched/parents2/parent-seed8.json', 'depthMm': 178.93200000000002},
    {'name': 'port-seed9', 'fixture': '/var/lib/t3/tmp/csched/parents2/parent-seed9.json', 'depthMm': 174.96558182288433},
    {'name': 'port-seed10', 'fixture': '/var/lib/t3/tmp/csched/parents2/parent-seed10.json', 'depthMm': 176.3622237458826},
    {'name': 'port-seed11', 'fixture': '/var/lib/t3/tmp/csched/parents2/parent-seed11.json', 'depthMm': 171.6141235046606},
    # The true-contract pins replay at the record lineage's own allowance,
    # 0.0005, not the from-request 0.002 the twelve port parents use - see
    # `docs/sol-review-5-se2-and-pose-freedom.md`'s task framing and
    # `constructor-inner-certificate/drivers/lib.py`'s g2-g4.
    {'name': 'true-156.9188', 'fixture': f'{TRUE}/pinned-fs-156.9188.json', 'depthMm': 156.9188, 'allowance': '0.0005'},
    {'name': 'true-156.091', 'fixture': f'{TRUE}/pinned-fs-156.0914.json', 'depthMm': 156.0914, 'allowance': '0.0005'},
    {'name': 'true-155.422', 'fixture': f'{TRUE}/pinned-fs-155.4223.json', 'depthMm': 155.4223, 'allowance': '0.0005'},
]


def run_arm(binary, parent, arm, work, drop_mm, out_path):
    spec = ARMS[arm]
    args = [a.format(pressure=spec['pressure']) for a in ARGS]
    target = parent['depthMm'] - drop_mm
    allowance = parent.get('allowance', DEFAULT_ALLOWANCE)
    tail = ['34', parent['fixture'], f'{target:.17g}', '', allowance]
    command = [binary, REQUEST] + args + tail
    env = dict(os.environ)
    env['POLYGON_NESTING_PROFILE'] = '1'
    env['POLYGON_NESTING_COMPRESSION_SCHEDULE'] = f'{SCHEDULE_V4},work={work}'
    env.pop('POLYGON_NESTING_CURRENT_POSE_OVERLAY', None)
    # Never armed on a measured arm: the classification runs an exact-tier
    # bisection per pair, so a run carrying it is a diagnostic, not a
    # measurement. `classify.py` arms it on its own runs.
    env.pop('POLYGON_NESTING_CURRENT_POSE_OVERLAY_CLASSIFY', None)
    if spec['overlay']:
        env['POLYGON_NESTING_CURRENT_POSE_OVERLAY'] = '1'
    started = time.monotonic()
    os.makedirs(os.path.dirname(out_path), exist_ok=True)
    with open(out_path, 'w') as handle:
        proc = subprocess.run(command, stdout=handle, stderr=subprocess.PIPE,
                              check=False, env=env)
    wall = time.monotonic() - started
    try:
        doc = json.load(open(out_path))
    except json.JSONDecodeError:
        return None, wall, (proc.stderr or b'').decode()[-1200:]
    return doc, wall, (proc.stderr or b'').decode()[-1200:]


def population(doc):
    return ((doc.get('relaxedDiagnostics') or {})
            .get('coupledDynamicSeparator') or {}).get(
                'persistentVacancyPopulation')


def counters(doc):
    profile = doc.get('searchProfile') or {}
    found = profile.get('counters') or {}
    queries = found.get('candidateQueries', 0)
    exact = found.get('exactPairTests', 0)
    return queries, exact, queries + 5 * exact


def summarize(parent, arm, doc, wall):
    pop = population(doc)
    row = {'parent': parent['name'], 'arm': arm, 'processWallSeconds': wall}
    if pop is None:
        row['error'] = 'no persistentVacancyPopulation in output'
        return row
    queries, exact, work = counters(doc)
    row.update({
        'attempted': pop.get('attempted'),
        'exactValid': pop.get('exactValid'),
        'failureReason': pop.get('failureReason'),
        'parentIndependentDepthMm': pop.get('parentIndependentDepthMm'),
        'rawSourceDepthMm': pop.get('rawSourceDepthMm'),
        'processCandidateQueries': queries,
        'processExactPairTests': exact,
        'processWorkUnits': work,
        'processQueriesPerSecond': (queries / wall) if wall > 0 else None,
    })
    schedule = pop.get('compressionSchedule')
    if not schedule:
        row['error'] = 'no compressionSchedule in output'
        return row
    steps = schedule.get('steps') or []
    raw_depths = [s['rawDepthMm'] for s in steps if s.get('rawDepthMm') is not None]
    published = pop.get('rawSourceDepthMm')
    row.update({
        'entryBoundaryViolations': schedule.get('parentBoundaryViolations'),
        'entryCollisionPairs': schedule.get('parentCollisionPairs'),
        'entryProxyFeasible': schedule.get('parentProxyFeasible'),
        'currentPoseOverlay': schedule.get('currentPoseOverlay'),
        # Two counts, not one (Sol review 6 §2.4): entries are catalogue keys
        # `(geometry_class, angle, mirror)` and collapse duplicates; off-grid
        # pieces are placements, and are what the snap would have damaged.
        'currentPoseOverlayEntries': schedule.get('currentPoseOverlayEntries'),
        'currentPoseOverlayOffGridPieces':
            schedule.get('currentPoseOverlayOffGridPieces'),
        'scheduleCandidateQueries': schedule.get('candidateQueries'),
        'scheduleExactPairTests': schedule.get('exactPairTests'),
        'scheduleWorkUnits': schedule.get('workUnits'),
        'scheduleQueriesPerSecond':
            (schedule.get('candidateQueries', 0) / wall) if wall > 0 else None,
        'confirmationsAttempted': schedule.get('confirmationsAttempted'),
        'confirmationsAccepted': schedule.get('confirmationsAccepted'),
        'confirmationsRefused': schedule.get('confirmationsRefused'),
        'rollbacks': schedule.get('rollbacks'),
        'exitCause': schedule.get('exitCause'),
        'confirmationMs': schedule.get('confirmationMs'),
        'repairMs': schedule.get('repairMs'),
        'stepsPlanned': schedule.get('stepsPlanned'),
        'stepsTaken': schedule.get('stepsTaken'),
        'bestRawDepthMm': min(raw_depths) if raw_depths else None,
        'published':
            published is not None and pop.get('parentIndependentDepthMm') is not None
            and published < pop.get('parentIndependentDepthMm'),
    })
    return row


def main():
    binary = sys.argv[1]
    outdir = sys.argv[2]
    arms = (sys.argv[3] if len(sys.argv) > 3 else 'grid,overlay,directional').split(',')
    work = int(sys.argv[4]) if len(sys.argv) > 4 else DESIGN_SLICE_UNITS
    drop_mm = float(sys.argv[5]) if len(sys.argv) > 5 else DEFAULT_DROP_MM
    os.makedirs(outdir, exist_ok=True)
    result = {
        'binary': binary,
        'binarySha256': hashlib.sha256(open(binary, 'rb').read()).hexdigest(),
        'workUnits': work,
        'dropMm': drop_mm,
        'allowance': DEFAULT_ALLOWANCE,
        'schedule': f'{SCHEDULE_V4},work={work}',
        'rows': [],
    }
    for parent in PORT_PARENTS:
        for arm in arms:
            path = f"{outdir}/{parent['name']}-{arm}.json"
            doc, wall, err = run_arm(binary, parent, arm, work, drop_mm, path)
            if doc is None:
                row = {'parent': parent['name'], 'arm': arm, 'error': err}
            else:
                row = summarize(parent, arm, doc, wall)
            result['rows'].append(row)
            print(json.dumps(row))
            json.dump(result, open(f'{outdir}/campaign.json', 'w'), indent=1)
    json.dump(result, open(f'{outdir}/campaign.json', 'w'), indent=1)


if __name__ == '__main__':
    main()
