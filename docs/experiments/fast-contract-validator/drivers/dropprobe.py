#!/usr/bin/env python3
"""Finds a descent drop that makes a fixture's schedule actually confirm.

    python3 dropprobe.py OUTDIR BINARY LABEL DROP[,DROP...]

A per-confirmation wall needs confirmations. On mixed-61 the campaign's standard
1.5 mm drop produces plenty; on shapes-17 and the eight-piece request it
produces **none** - the schedule descends the full 1.5 mm with
`confirmationsSkippedInfeasible` equal to `stepsTaken` and
`confirmationsAttempted` at zero, because the proxy never calls the reduced
layout feasible and the exact validator is therefore never reached.

That is a property of the fixture, not a failure: a 1.5 mm drop is a large ask
on a 17-piece layout whose pieces are big relative to the sheet. This probe
sweeps the drop and reports where confirmations start, so the wall battery can
be run at a depth the fixture can actually reach rather than at a number
inherited from a different fixture.

It prints one row per drop and makes no claim of its own.
"""
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import runlib  # noqa: E402
import wallfixtures  # noqa: E402


def main():
    outdir, binary, label = sys.argv[1], sys.argv[2], sys.argv[3]
    drops = [float(d) for d in sys.argv[4].split(',')]
    row = wallfixtures.FIXTURES[label]
    depth = wallfixtures.parent_depth(row['parent'])
    os.makedirs(outdir, exist_ok=True)
    out = {'label': label, 'parentDepthMm': depth, 'binary': binary,
           'rows': []}
    for drop in drops:
        target = depth - drop
        path = f'{outdir}/{label}-drop{drop:g}.json'
        result = wallfixtures.run_arm(
            binary, row['request'], row['parent'], target, row['allowance'],
            'past=0,rollback=0,lanes=1,pconfirm=0', path)
        try:
            doc = json.load(open(path))
            pop = ((doc.get('relaxedDiagnostics') or {})
                   .get('coupledDynamicSeparator') or {}).get(
                       'persistentVacancyPopulation') or {}
            schedule = pop.get('compressionSchedule') or {}
        except (json.JSONDecodeError, FileNotFoundError):
            schedule = {}
        out['rows'].append({
            'dropMm': drop,
            'targetDepthMm': target,
            'confirmationsAttempted': schedule.get('confirmationsAttempted'),
            'confirmationsAccepted': schedule.get('confirmationsAccepted'),
            'confirmationsSkippedInfeasible': schedule.get(
                'confirmationsSkippedInfeasible'),
            'stepsTaken': schedule.get('stepsTaken'),
            'exitCause': schedule.get('exitCause'),
            'finalDepthMm': schedule.get('finalDepthMm'),
            'rawSourceDepthMm': pop.get('rawSourceDepthMm') if schedule else None,
            'perConfirmationMs': result.get('perConfirmationMs'),
            'processWallSeconds': result.get('processWallSeconds'),
        })
        print(json.dumps(out['rows'][-1]), file=sys.stderr)
    json.dump(out, open(f'{outdir}/dropprobe-{label}.json', 'w'), indent=1)
    print(json.dumps(out, indent=1))


if __name__ == '__main__':
    main()
