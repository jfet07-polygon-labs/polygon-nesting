#!/usr/bin/env python3
"""The matched-arm quality gate: the compression schedule against one short
legacy mode-26 ladder, from the same parent at the same seed.

    python3 gate.py PARENTSDIR OUTDIR SCHEDULE_BINARY GATE_BINARY [ARMS] [DROP]

Every cell is one pinned parent fixture (see `parents.py`) plus one relaxed
seed, and every arm in a cell descends from *that file*, so "the same parent"
is a property of the input rather than of a re-run. The statistic is the raw
source depth of the arm's exact-valid publication, with the parent as the
floor - which is the contract both modes already publish under, so an arm that
finds nothing reports its parent rather than a failure.

The arms:

  `m26`   one legacy short mode-26 ladder to `parent - DROP`. This is the
          control the opportunity ledger's arm C measured at 174-179 mm
          parents, minus the coordinator-level mode-31 rung that followed it
          there - that rung is a *second*, outer legalizer and not part of the
          operator under comparison.
  `sched` the compression schedule at the same allowance, 33,413,789 work
          units, which is one measured mode-26 rung (32,246,564 candidate
          queries + 5 x 233,445 exact pair tests). It is asked for the same
          drop and allowed to continue past it, because the control also
          overshoots its own requested bound - the ledger measured rung 1
          publishing 4.25 mm below a 0.174 mm request - so stopping the
          schedule at the request while the control runs past it would not be
          a matched arm.
  `sched10` the same schedule at 3,341,379 units: 10% of a rung, the middle of
          the 5.9-11.7% band the anatomy's 0.5-1.0 s design slice works out to.
          This is the *cost* arm, not the quality arm.

Work is read two ways and both are reported. The schedule counts its own spend
deterministically (lane candidate queries plus 5 per derived exact pair test),
and every run is additionally executed with the process profile armed so the
whole-process counter is available; the mode's own work is that counter minus
the identical mode-0 preamble every arm pays, which is measured once per cell.
"""
import hashlib
import json
import os
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import runlib  # noqa: E402

# One measured mode-26 rung, from the rung anatomy: 32,246,564 candidate
# queries + 5 x 233,445 exact pair tests.
RUNG_WORK_UNITS = 33_413_789
# The anatomy's design slice: 0.5-1.0 s is 5.9-11.7% of a rung.
DESIGN_SLICE_UNITS = 3_341_379
# The anatomy's shortest sampled ladder drop, and the ledger's arm C.
DEFAULT_DROP_MM = 0.3

# Every schedule arm names its rollback explicitly, so the driver measures what
# it says it measures whatever the engine's default happens to be. The `-noroll`
# pair is the rollback's own A/B: the anatomy's central complaint about mode 26
# is that 85.4% of its arms abort on a rollback and burn 75.5% of the wall, so a
# schedule that never gives its frontier back is the control that says whether
# this port's rollback is buying anything.
ARMS = {
    'm26': {'mode': 26, 'schedule': None},
    'sched': {'mode': 34,
              'schedule': f'past=1,rollback=32,work={RUNG_WORK_UNITS}'},
    'sched10': {'mode': 34,
                'schedule': f'past=1,rollback=32,work={DESIGN_SLICE_UNITS}'},
    'sched-noroll': {'mode': 34,
                     'schedule': f'past=1,rollback=0,work={RUNG_WORK_UNITS}'},
    'sched10-noroll': {'mode': 34,
                       'schedule':
                           f'past=1,rollback=0,work={DESIGN_SLICE_UNITS}'},
    # The preamble every arm pays: the same process, the same coupled
    # separator arms, no operator. Subtracting it is what turns a whole-process
    # counter into the operator's own spend.
    'preamble': {'mode': 0, 'schedule': None},
}


def run_arm(binary, seed, fixture, target, arm, out_path):
    args = [a.format(seed=seed) for a in runlib.ARGS]
    spec = ARMS[arm]
    mode = spec['mode']
    tail = [str(mode),
            fixture if mode else '',
            f'{target:.17g}' if mode else '',
            '',
            runlib.DEFAULT_ALLOWANCE]
    command = [binary, runlib.REQUESTS['mixed-61']] + args + tail
    env = dict(os.environ)
    env['POLYGON_NESTING_PROFILE'] = '1'
    env.pop('POLYGON_NESTING_COMPRESSION_SCHEDULE', None)
    if spec['schedule']:
        env['POLYGON_NESTING_COMPRESSION_SCHEDULE'] = spec['schedule']
    started = time.monotonic()
    os.makedirs(os.path.dirname(out_path), exist_ok=True)
    with open(out_path, 'w') as handle:
        proc = subprocess.run(command, stdout=handle, stderr=subprocess.PIPE,
                              check=False, env=env)
    wall = time.monotonic() - started
    try:
        doc = json.load(open(out_path))
    except json.JSONDecodeError:
        return None, wall, (proc.stderr or b'').decode()[-800:]
    return doc, wall, (proc.stderr or b'').decode()[-800:]


def counters(doc):
    profile = doc.get('searchProfile') or {}
    found = profile.get('counters') or {}
    queries = found.get('candidateQueries', 0)
    exact = found.get('exactPairTests', 0)
    return queries, exact, queries + 5 * exact


def population(doc):
    return ((doc.get('relaxedDiagnostics') or {})
            .get('coupledDynamicSeparator') or {}).get(
                'persistentVacancyPopulation')


def main():
    parents_dir = sys.argv[1]
    outdir = sys.argv[2]
    schedule_binary = sys.argv[3]
    gate_binary = sys.argv[4]
    arms = (sys.argv[5] if len(sys.argv) > 5 else 'preamble,m26,sched,sched10').split(',')
    drop_mm = float(sys.argv[6]) if len(sys.argv) > 6 else DEFAULT_DROP_MM
    parents = json.load(open(f'{parents_dir}/parents.json'))
    os.makedirs(outdir, exist_ok=True)
    result = {
        'parentsDir': parents_dir,
        'dropMm': drop_mm,
        'rungWorkUnits': RUNG_WORK_UNITS,
        'designSliceUnits': DESIGN_SLICE_UNITS,
        'allowance': runlib.DEFAULT_ALLOWANCE,
        'scheduleBinarySha256':
            hashlib.sha256(open(schedule_binary, 'rb').read()).hexdigest(),
        'gateBinarySha256':
            hashlib.sha256(open(gate_binary, 'rb').read()).hexdigest(),
        'cells': [],
    }
    for parent in parents['rows']:
        if 'error' in parent:
            continue
        seed = parent['seed']
        fixture = parent['fixture']
        parent_depth = parent['rawDepthMm']
        target = parent_depth - drop_mm
        cell = {'seed': seed, 'fixture': fixture,
                'parentRawDepthMm': parent_depth,
                'targetMm': target, 'arms': {}}
        for arm in arms:
            # `m26` and the preamble run on the pinned default-feature gate
            # binary; the schedule arms need the build that carries it.
            binary = schedule_binary if ARMS[arm]['mode'] == 34 else gate_binary
            path = f'{outdir}/seed{seed}-{arm}.json'
            doc, wall, err = run_arm(binary, seed, fixture, target, arm, path)
            if doc is None:
                cell['arms'][arm] = {'error': err}
                continue
            queries, exact, work = counters(doc)
            row = {'processWallSeconds': wall,
                   'processCandidateQueries': queries,
                   'processExactPairTests': exact,
                   'processWorkUnits': work}
            pop = population(doc)
            if pop is not None:
                row.update({
                    'attempted': pop.get('attempted'),
                    'exactValid': pop.get('exactValid'),
                    'contractValid': pop.get('contractValid'),
                    'rawSourceDepthMm': pop.get('rawSourceDepthMm'),
                    'independentDepthMm': pop.get('independentDepthMm'),
                    'parentIndependentDepthMm':
                        pop.get('parentIndependentDepthMm'),
                    'finalPlacementFingerprint':
                        pop.get('finalPlacementFingerprint'),
                    'failureReason': pop.get('failureReason'),
                })
                ladder = pop.get('ladderCompression')
                if ladder:
                    row['ladder'] = {k: ladder.get(k) for k in (
                        'parentDepthMm', 'finalBoundMm', 'stepMm',
                        'stepsPlanned', 'stepsRun', 'publishedStep',
                        'publishedBoundMm')}
                schedule = pop.get('compressionSchedule')
                if schedule:
                    steps = schedule.get('steps') or []
                    row['schedule'] = {k: v for k, v in schedule.items()
                                       if k != 'steps'}
                    row['scheduleCurve'] = [
                        {'step': s['step'], 'depthMm': s['depthMm'],
                         'rawDepthMm': s['rawDepthMm']}
                        for s in steps if s.get('rawDepthMm') is not None]
                    row['scheduleResidue'] = {
                        'stepsWithViolations': sum(
                            1 for s in steps
                            if s['boundaryViolationsBefore'] > 0),
                        'maxViolationsAfterStep': max(
                            (s['boundaryViolationsBefore'] for s in steps),
                            default=0),
                        'stepsFeasibleAfterRepair': sum(
                            1 for s in steps if s['proxyFeasible']),
                        'steps': len(steps),
                    }
            cell['arms'][arm] = row
        result['cells'].append(cell)
        json.dump(result, open(f'{outdir}/gate.json', 'w'), indent=1)
    print(json.dumps(result, indent=1))


if __name__ == '__main__':
    main()
