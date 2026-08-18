#!/usr/bin/env python3
"""The two record-lineage parents, for contrast only.

    python3 records.py OUTDIR SCHEDULE_BINARY GATE_BINARY [DROP]

The record lineage is a different search envelope - allowance `0.0005`, not the
`0.002` the coordinator band uses - so nothing here is comparable to the gate's
numbers, and it is not run as part of the gate. It is here because the mode-26
rung anatomy sampled its cost at exactly these two parents and measured zero
publishing arms in 171, and because the schedule reports one thing that round
could not: what the *proxy tier* thinks of a record-lineage parent before
anything moves.

Both parents' rotations are continuous (61 distinct angles on the 159.079
parent), and the structured surrogate backend the relaxed lane runs on can only
represent a pose on its 2.5-degree grid, so `initialize_complete_state` snaps
them on the way in. That snap is upstream of both modes and is not something the
schedule introduces - but it is what these two rows measure.
"""
import hashlib
import json
import os
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import runlib  # noqa: E402

RECORD_ALLOWANCE = '0.0005'
TRUE = (f'{runlib.ROOT}/docs/experiments/persistent-vacancy-descent/'
        'exact-contract/true-contract/finer-ladder')
PARENTS = [
    ('lin-159.079', f'{TRUE}/pinned-parent-159.079.json', 159.07876040364795),
    ('fs-164.038', f'{TRUE}/pinned-fs-parent-164.0376.json', 164.0375677990678),
]
RUNG_WORK_UNITS = 33_413_789


def run(binary, mode, parent, target, seed, schedule_spec, out_path):
    args = [a.format(seed=seed) for a in runlib.ARGS]
    command = ([binary, runlib.REQUESTS['mixed-61']] + args
               + [str(mode), parent, f'{target:.17g}', '', RECORD_ALLOWANCE])
    env = dict(os.environ)
    env['POLYGON_NESTING_PROFILE'] = '1'
    env.pop('POLYGON_NESTING_COMPRESSION_SCHEDULE', None)
    if schedule_spec:
        env['POLYGON_NESTING_COMPRESSION_SCHEDULE'] = schedule_spec
    started = time.monotonic()
    os.makedirs(os.path.dirname(out_path), exist_ok=True)
    with open(out_path, 'w') as handle:
        subprocess.run(command, stdout=handle, stderr=subprocess.DEVNULL,
                       check=False, env=env)
    wall = time.monotonic() - started
    try:
        return json.load(open(out_path)), wall
    except json.JSONDecodeError:
        return None, wall


def main():
    outdir = sys.argv[1]
    schedule_binary = sys.argv[2]
    gate_binary = sys.argv[3]
    drop = float(sys.argv[4]) if len(sys.argv) > 4 else 0.3
    result = {'allowance': RECORD_ALLOWANCE, 'dropMm': drop,
              'scheduleBinarySha256':
                  hashlib.sha256(open(schedule_binary, 'rb').read()).hexdigest(),
              'rows': []}
    arms = [
        ('m26', 26, gate_binary, None),
        ('sched', 34, schedule_binary, f'past=1,work={RUNG_WORK_UNITS}'),
    ]
    for tag, fixture, depth in PARENTS:
        for arm, mode, binary, spec in arms:
            path = f'{outdir}/{tag}-{arm}.json'
            doc, wall = run(binary, mode, fixture, depth - drop, 5, spec, path)
            if doc is None:
                result['rows'].append({'parent': tag, 'arm': arm,
                                       'error': 'no document'})
                continue
            pop = ((doc.get('relaxedDiagnostics') or {})
                   .get('coupledDynamicSeparator') or {}).get(
                       'persistentVacancyPopulation') or {}
            profile = (doc.get('searchProfile') or {}).get('counters') or {}
            schedule = dict(pop.get('compressionSchedule') or {})
            schedule.pop('steps', None)
            result['rows'].append({
                'parent': tag,
                'parentRawDepthMm': depth,
                'arm': arm,
                'exactValid': pop.get('exactValid'),
                'contractValid': pop.get('contractValid'),
                'rawSourceDepthMm': pop.get('rawSourceDepthMm'),
                'deltaMm': (depth - pop['rawSourceDepthMm'])
                if pop.get('rawSourceDepthMm') is not None else None,
                'processWallSeconds': wall,
                'processWorkUnits': profile.get('candidateQueries', 0)
                + 5 * profile.get('exactPairTests', 0),
                'schedule': schedule or None,
            })
    print(json.dumps(result, indent=1))
    os.makedirs(outdir, exist_ok=True)
    json.dump(result, open(f'{outdir}/records.json', 'w'), indent=1)


if __name__ == '__main__':
    main()
