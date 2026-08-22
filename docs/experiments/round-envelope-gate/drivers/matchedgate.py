#!/usr/bin/env python3
"""Sol review 12 §3.2's remaining kill: the round-envelope kernel against the
canonical miter authority, on the twelve pinned parents, at equal operator wall.

    matchedgate.py OUTDIR BINARY PARENTSJSON WORKS [ARMS] [DROP_MM] [ALLOWANCE]

`WORKS` is a comma-separated ladder of mode-34 work caps. `ARMS` is a subset of
`miter,union,exclusive`.

# The two arms are one binary and one environment variable

`matched.py`'s `run_m34` is the control, and what is verbatim is everything that
decides what gets measured: the `past=1,rollback=0,work=W,lanes=1,pconfirm=0`
spec string character for character, the pinned positional tail through the
shared `runlib.ARGS`, `POLYGON_NESTING_PROFILE=1`, the `target = parent - drop`
construction and its `.17g` formatting, the same `searchProfile` reader with
`processWorkUnits = candidateQueries + 5 * exactPairTests`, and the same
`rawSourceDepthMm` scoring with the parent as the floor.

The arm is the same command with `POLYGON_NESTING_ROUND_ENVELOPE_KERNEL` set.
Not a second binary: the kernel's own round measured that a build carrying the
feature *unarmed* reproduces all four pinned gates as whole documents, so the
binary is common mode and the variable is the only difference between the arms.

`rek` - the portfolio spec key - could not do this. It arms the v3 coordinator,
and `run_portfolio` runs from the request alone with no pinned parent anywhere;
Sol's kill is written on twelve pinned parents. The environment door exists for
that reason and is refused outright by a binary that cannot honour it.

# Why the ladder, and what "equal operator wall" means here

The engine is deterministic in the seed and the work cap, so a cell's depth
needs no replicas: two runs of one cell differ in wall and in nothing else. The
wall does *not* need to be equal by construction, then - it needs to be equal at
the point the two arms are compared, and the honest way to get there on a
polluted box is to measure each arm's own depth-against-wall curve and read both
arms at the same wall.

So every arm runs the same ladder of work caps, and three readings come out:

* **equal work** - cell against cell at the same `work=W`. The cheap reading,
  and the one that is immune to wall pollution entirely.
* **equal operator wall** - each arm's curve is interpolated to a common wall
  and the two are compared there. This is the reading Sol's kill is written in.
* **per-confirmation cost** - `confirmationMs / confirmationsAttempted`, the
  engine's own clock on its own confirmation, which is the quantity the
  <=1.25x clause is about.

Wall is reported twice: `processWallSeconds` (carries ~1.4 s of process startup
and parent load) and `elapsedMs` (the measured stream, which is what "operator
wall" means here). The interpolation uses `elapsedMs`.
"""
import hashlib
import json
import os
import statistics
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import runlib  # noqa: E402

# matched.py's spec string, character for character.
SPEC = 'past=1,rollback=0,work={work},lanes=1,pconfirm=0'
DEFAULT_DROP_MM = 1.0
DEFAULT_ARMS = 'miter,union'
KERNEL_ENV = 'POLYGON_NESTING_ROUND_ENVELOPE_KERNEL'
# The environment value each arm sets, and the `mode` label the run must then
# report back. `miter` sets nothing and must report nothing.
ARM_ENV = {'miter': None, 'union': '1', 'exclusive': '2'}
ARM_LABEL = {'miter': None, 'union': 'union', 'exclusive': 'exclusive'}
# Every round-scoped environment name this repository has ever armed an operator
# with, scrubbed so an inherited one cannot become an unlabelled arm.
ROUND_ENV = ('POLYGON_NESTING_CONTACT_BLOCK', 'POLYGON_NESTING_SE2_WITNESS',
             'POLYGON_NESTING_CONTINUOUS_ROTATION',
             'POLYGON_NESTING_SPARSE_ROTATION',
             'POLYGON_NESTING_COMPRESSION_SCHEDULE',
             'POLYGON_NESTING_ROUND_ENVELOPE_KERNEL')


def population(doc):
    return ((doc.get('relaxedDiagnostics') or {})
            .get('coupledDynamicSeparator') or {}).get(
                'persistentVacancyPopulation')


def run_cell(binary, arm, seed, fixture, target, work, out_path, allowance):
    """One mode-34 slice. `matched.py:run_m34` with the arm's variable set."""
    args = [a.format(seed=seed) for a in runlib.ARGS]
    command = ([binary, runlib.REQUESTS['mixed-61']] + args
               + ['34', fixture, f'{target:.17g}', '', allowance])
    env = dict(os.environ)
    for name in ROUND_ENV:
        env.pop(name, None)
    env['POLYGON_NESTING_PROFILE'] = '1'
    env['POLYGON_NESTING_COMPRESSION_SCHEDULE'] = SPEC.format(work=int(work))
    if ARM_ENV[arm] is not None:
        env[KERNEL_ENV] = ARM_ENV[arm]
    os.makedirs(os.path.dirname(out_path), exist_ok=True)
    started = time.monotonic()
    with open(out_path, 'w') as handle:
        proc = subprocess.run(command, stdout=handle, stderr=subprocess.PIPE,
                              check=False, env=env)
    wall = time.monotonic() - started
    row = {'arm': arm, 'workCap': int(work), 'exitCode': proc.returncode,
           'processWallSeconds': wall}
    try:
        doc = json.load(open(out_path))
    except json.JSONDecodeError:
        row['error'] = (proc.stderr or b'').decode()[-800:]
        return row
    # The arm must *say* it is the arm. A binary built without the feature
    # refuses the variable outright, so this can only fail on a driver bug -
    # which is exactly the bug worth failing on, because its symptom is a miter
    # run published under a round label.
    reported = (doc.get('roundEnvelopeKernel') or {}).get('mode')
    row['reportedKernelMode'] = reported
    row['armReportedCorrectly'] = (reported == ARM_LABEL[arm])
    profile = (doc.get('searchProfile') or {}).get('counters') or {}
    queries = profile.get('candidateQueries', 0)
    tests = profile.get('exactPairTests', 0)
    row['processCandidateQueries'] = queries
    row['processExactPairTests'] = tests
    row['processWorkUnits'] = queries + 5 * tests
    elapsed = doc.get('medianElapsedMs')
    row['elapsedMs'] = elapsed
    row['operatorWallSeconds'] = (elapsed / 1000.0
                                  if elapsed is not None else None)
    pop = population(doc)
    if pop is None:
        row['error'] = 'no population'
        return row
    row['exactValid'] = pop.get('exactValid')
    row['contractValid'] = pop.get('contractValid')
    row['rawSourceDepthMm'] = pop.get('rawSourceDepthMm')
    row['usedLongAxisDepthMm'] = doc.get('usedLongAxisDepthMm')
    row['fingerprint'] = pop.get('finalPlacementFingerprint')
    schedule = pop.get('compressionSchedule') or {}
    for key in ('workUnits', 'confirmationsAttempted', 'confirmationsAccepted',
                'confirmationsRefused', 'confirmationsSkippedInfeasible',
                'confirmationMs', 'stepsTaken', 'stepsPlanned', 'acceptedMoves',
                'exitCause', 'stepDigest', 'finalDepthMm', 'targetDepthMm',
                'microLegalizationsAttempted', 'microLegalizationsAccepted'):
        row['schedule_' + key[0].lower() + key[1:]] = schedule.get(key)
    attempted = schedule.get('confirmationsAttempted') or 0
    ms = schedule.get('confirmationMs')
    row['msPerConfirmation'] = (ms / attempted
                                if attempted and ms is not None else None)
    return row


def main():
    outdir, binary, parents_json = sys.argv[1], sys.argv[2], sys.argv[3]
    works = [int(w) for w in sys.argv[4].split(',')]
    arms = (sys.argv[5] if len(sys.argv) > 5 else DEFAULT_ARMS).split(',')
    drop_mm = float(sys.argv[6]) if len(sys.argv) > 6 else DEFAULT_DROP_MM
    allowance = sys.argv[7] if len(sys.argv) > 7 else runlib.DEFAULT_ALLOWANCE
    parents = json.load(open(parents_json))['rows']
    os.makedirs(outdir, exist_ok=True)
    result = {
        'binary': binary,
        'binarySha256': hashlib.sha256(open(binary, 'rb').read()).hexdigest(),
        'parents': parents_json,
        'works': works,
        'arms': arms,
        'dropMm': drop_mm,
        'allowance': allowance,
        'spec': SPEC,
        'cells': [],
    }
    for parent in parents:
        seed = parent['seed']
        parent_depth = parent['rawDepthMm']
        target = parent_depth - drop_mm
        cell = {'seed': seed, 'parentRawDepthMm': parent_depth,
                'targetDepthMm': target, 'arms': {}}
        # Interleaved: every budget is run on both arms back to back, so a
        # transient on this box lands on both arms rather than on one.
        for work in works:
            for arm in arms:
                row = run_cell(binary, arm, seed, parent['fixture'], target,
                               work, f'{outdir}/seed{seed}-{arm}-{work}.json',
                               allowance)
                if row.get('rawSourceDepthMm') is None:
                    row['rawSourceDepthMm'] = parent_depth
                row['deltaVsParentMm'] = parent_depth - row['rawSourceDepthMm']
                cell['arms'][f'{arm}:{work}'] = row
                print(f"seed{seed} {arm}:{work} "
                      f"depth={row['rawSourceDepthMm']:.4f} "
                      f"delta={row['deltaVsParentMm']:.4f}mm "
                      f"work={(row.get('processWorkUnits') or 0)/1e6:.2f}M "
                      f"opwall={row.get('operatorWallSeconds') or 0:.2f}s "
                      f"wall={row['processWallSeconds']:.2f}s "
                      f"conf={row.get('schedule_confirmationsAttempted')} "
                      f"armOk={row.get('armReportedCorrectly')}", flush=True)
        result['cells'].append(cell)
        json.dump(result, open(f'{outdir}/matchedgate.json', 'w'), indent=1)
    json.dump(result, open(f'{outdir}/matchedgate.json', 'w'), indent=1)
    print(json.dumps({'cells': len(result['cells']),
                      'armsReportedCorrectly': all(
                          row.get('armReportedCorrectly')
                          for c in result['cells']
                          for row in c['arms'].values())}, indent=1))


if __name__ == '__main__':
    main()
