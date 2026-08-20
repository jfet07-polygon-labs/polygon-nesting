#!/usr/bin/env python3
"""The equal-WORK matched-arm gate: the parallel schedule against the serial
one, from the same pinned parent at the same seed under the same cap.

    python3 workgate.py PARENTSDIR OUTDIR BINARY ARMS [DROP_MM]

This is the gate Grok's action 2 names as measurement (b), and its whole point
is that it is denominated in **work**, not wall. The fan-out charges every
worker it dispatches - winners and losers - so at a fixed work cap the parallel
arm takes fewer depth steps than the serial one and has to pay for that with
better steps. If it cannot, the lever is a wall-only lever and must be sold as
one.

Both arms run on the *same* binary, the armed one, and differ only in the spec:
`lanes=1` is the shipped serial schedule and `lanes=8` the fan-out. That
removes the build from the comparison, which matters here because the two arms
are being compared on quality rather than on a document digest.

The statistic per cell is the raw source depth of the best exact-valid
publication, with the parent as the floor for both arms - the contract both
modes already publish under - so an arm that finds nothing scores its parent
rather than being dropped.
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

# The anatomy's design slice, the arm the compression-schedule round found most
# efficient per unit of work (1.013 mm per M units, 6.0x the ladder).
DESIGN_SLICE_UNITS = 3_341_379
# One measured mode-26 rung.
RUNG_WORK_UNITS = 33_413_789
DEFAULT_DROP_MM = 0.3

ARMS = {
    # The control: the shipped schedule, one lane, serial confirmation.
    'serial': 'past=1,rollback=0,work={work},lanes=1,pconfirm=0',
    # The repair fan-out alone.
    'lanes8': 'past=1,rollback=0,work={work},lanes=8,pconfirm=0',
    'lanes4': 'past=1,rollback=0,work={work},lanes=4,pconfirm=0',
    # The confirmation lever alone. It is semantics-preserving, so this arm is
    # the one that must reproduce `serial` exactly - it is a control on the
    # measurement, not a quality arm.
    'pconfirm': 'past=1,rollback=0,work={work},lanes=1,pconfirm=1',
    # Both levers.
    'both': 'past=1,rollback=0,work={work},lanes=8,pconfirm=1',
}


def run_arm(binary, seed, fixture, target, spec, out_path):
    args = [a.format(seed=seed) for a in runlib.ARGS]
    tail = ['34', fixture, f'{target:.17g}', '', runlib.DEFAULT_ALLOWANCE]
    command = [binary, runlib.REQUESTS['mixed-61']] + args + tail
    env = dict(os.environ)
    env['POLYGON_NESTING_PROFILE'] = '1'
    env['POLYGON_NESTING_COMPRESSION_SCHEDULE'] = spec
    os.makedirs(os.path.dirname(out_path), exist_ok=True)
    started = time.monotonic()
    with open(out_path, 'w') as handle:
        proc = subprocess.run(command, stdout=handle, stderr=subprocess.PIPE,
                              check=False, env=env)
    wall = time.monotonic() - started
    try:
        return json.load(open(out_path)), wall, ''
    except json.JSONDecodeError:
        return None, wall, (proc.stderr or b'').decode()[-800:]


def population(doc):
    return ((doc.get('relaxedDiagnostics') or {})
            .get('coupledDynamicSeparator') or {}).get(
                'persistentVacancyPopulation')


def row_for(doc, wall, parent_depth):
    row = {'processWallSeconds': wall}
    profile = (doc.get('searchProfile') or {}).get('counters') or {}
    row['processCandidateQueries'] = profile.get('candidateQueries', 0)
    row['processExactPairTests'] = profile.get('exactPairTests', 0)
    row['processWorkUnits'] = (row['processCandidateQueries']
                               + 5 * row['processExactPairTests'])
    pop = population(doc)
    if pop is None:
        row['error'] = 'no population'
        return row
    row['exactValid'] = pop.get('exactValid')
    row['contractValid'] = pop.get('contractValid')
    raw = pop.get('rawSourceDepthMm')
    # The parent is the floor of the statistic, for both arms.
    row['rawSourceDepthMm'] = raw if raw is not None else parent_depth
    row['deltaMm'] = parent_depth - row['rawSourceDepthMm']
    row['fingerprint'] = pop.get('finalPlacementFingerprint')
    schedule = pop.get('compressionSchedule')
    if schedule:
        row['schedule'] = {k: v for k, v in schedule.items() if k != 'steps'}
    return row


def main():
    parents_dir = sys.argv[1]
    outdir = sys.argv[2]
    binary = sys.argv[3]
    arms = sys.argv[4].split(',')
    drop_mm = float(sys.argv[5]) if len(sys.argv) > 5 else DEFAULT_DROP_MM
    work = int(os.environ.get('WORKGATE_UNITS', DESIGN_SLICE_UNITS))
    parents = json.load(open(f'{parents_dir}/parents.json'))['rows']
    os.makedirs(outdir, exist_ok=True)
    result = {
        'binary': binary,
        'binarySha256': hashlib.sha256(open(binary, 'rb').read()).hexdigest(),
        'workUnits': work,
        'dropMm': drop_mm,
        'arms': {arm: ARMS[arm].format(work=work) for arm in arms},
        'cells': [],
    }
    for parent in parents:
        seed = parent['seed']
        parent_depth = parent['rawDepthMm']
        target = parent_depth - drop_mm
        cell = {'seed': seed, 'parentRawDepthMm': parent_depth, 'arms': {}}
        for arm in arms:
            spec = ARMS[arm].format(work=work)
            path = f'{outdir}/seed{seed}-{arm}.json'
            doc, wall, err = run_arm(binary, seed, parent['fixture'], target,
                                     spec, path)
            cell['arms'][arm] = ({'error': err} if doc is None
                                 else row_for(doc, wall, parent_depth))
        result['cells'].append(cell)
        json.dump(result, open(f'{outdir}/workgate.json', 'w'), indent=1)
    result['summary'] = summarise(result, arms)
    json.dump(result, open(f'{outdir}/workgate.json', 'w'), indent=1)
    print(json.dumps(result['summary'], indent=1))


def summarise(result, arms):
    out = {}
    for arm in arms:
        deltas = [c['arms'][arm]['deltaMm'] for c in result['cells']
                  if 'deltaMm' in c['arms'][arm]]
        works = [c['arms'][arm]['schedule']['workUnits']
                 for c in result['cells']
                 if c['arms'][arm].get('schedule')]
        walls = [c['arms'][arm]['processWallSeconds'] for c in result['cells']]
        out[arm] = {
            'cells': len(deltas),
            'publishes': sum(1 for d in deltas if d > 0),
            'medianDeltaMm': statistics.median(deltas) if deltas else None,
            'meanDeltaMm': statistics.fmean(deltas) if deltas else None,
            'bestDeltaMm': max(deltas, default=None),
            'medianScheduleWorkUnits': statistics.median(works) if works else None,
            'medianProcessWallSeconds': statistics.median(walls) if walls else None,
        }
    control = arms[0]
    for arm in arms[1:]:
        paired = [(c['arms'][arm]['deltaMm'] - c['arms'][control]['deltaMm'])
                  for c in result['cells']
                  if 'deltaMm' in c['arms'][arm]
                  and 'deltaMm' in c['arms'][control]]
        out[f'{arm}-minus-{control}'] = {
            'cells': len(paired),
            'medianAdvantageMm': statistics.median(paired) if paired else None,
            'meanAdvantageMm': statistics.fmean(paired) if paired else None,
            'wins': sum(1 for p in paired if p > 1e-12),
            'ties': sum(1 for p in paired if abs(p) <= 1e-12),
            'losses': sum(1 for p in paired if p < -1e-12),
            'perCell': paired,
        }
    return out


if __name__ == '__main__':
    main()
