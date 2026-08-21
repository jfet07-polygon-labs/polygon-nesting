#!/usr/bin/env python3
"""What fraction of the validator's pairs the broad phase can prove clear.

    python3 census.py OUTDIR BINARY SEEDS [DROP_MM]

This is the number that decides whether a *pair-level* filter is the whole
answer or only the first half of it, and it is deliberately not measured from
the wall: it is a count, taken by a separate `O(n^2)` pass inside the validator
that runs only under `POLYGON_NESTING_CONTRACT_VALIDATOR_CENSUS`, so the loop
being described is never the loop being timed.

The census arrives on **stderr**, not in the result document. That is the whole
discipline of this feature: flag-on must leave the document byte-identical, so
the instrument that proves the filter works is not allowed to live in it.

Runs mode 34 from the twelve pinned 171-179 mm parents, one process per seed,
`pconfirm=0` so the count is the serial validator's own.
"""
import json
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import runlib  # noqa: E402

PARENTS = '/var/lib/t3/tmp/csched'
CENSUS = re.compile(
    r'contractValidatorCensus calls=(\d+) pairs=(\d+) provedClear=(\d+) '
    r'skipRate=([0-9.]+)')


def parents():
    """The twelve pinned parents, from the committed manifests."""
    rows = {}
    for manifest in (f'{PARENTS}/parents/parents.json',
                     f'{PARENTS}/parents-rest/parents.json'):
        if not os.path.exists(manifest):
            continue
        for row in json.load(open(manifest))['rows']:
            if 'fixture' in row:
                rows[row['seed']] = row
    return rows


def run(binary, seed, fixture, target, out_path, spec):
    args = [a.format(seed=seed) for a in runlib.ARGS]
    tail = ['34', fixture, f'{target:.17g}', '', runlib.DEFAULT_ALLOWANCE]
    command = [binary, runlib.REQUESTS['mixed-61']] + args + tail
    env = dict(os.environ)
    env.pop('POLYGON_NESTING_PROFILE', None)
    env['POLYGON_NESTING_COMPRESSION_SCHEDULE'] = spec
    env['POLYGON_NESTING_CONTRACT_VALIDATOR_CENSUS'] = '1'
    os.makedirs(os.path.dirname(out_path), exist_ok=True)
    with open(out_path, 'w') as handle:
        proc = subprocess.run(command, stdout=handle, stderr=subprocess.PIPE,
                              check=False, env=env)
    stderr = (proc.stderr or b'').decode()
    match = CENSUS.search(stderr)
    row = {'seed': seed, 'exitCode': proc.returncode}
    if match:
        row.update({
            'calls': int(match.group(1)),
            'pairs': int(match.group(2)),
            'provedClear': int(match.group(3)),
            'skipRate': float(match.group(4)),
        })
    else:
        row['stderrTail'] = stderr[-400:]
    try:
        doc = json.load(open(out_path))
        pop = ((doc.get('relaxedDiagnostics') or {})
               .get('coupledDynamicSeparator') or {}).get(
                   'persistentVacancyPopulation') or {}
        schedule = pop.get('compressionSchedule') or {}
        row['rawSourceDepthMm'] = pop.get('rawSourceDepthMm')
        row['confirmationsAccepted'] = schedule.get('confirmationsAccepted')
        row['confirmationsAttempted'] = schedule.get('confirmationsAttempted')
    except (json.JSONDecodeError, FileNotFoundError):
        pass
    return row


def main():
    outdir, binary = sys.argv[1], sys.argv[2]
    seeds = [int(s) for s in sys.argv[3].split(',')]
    drop_mm = float(sys.argv[4]) if len(sys.argv) > 4 else 1.5
    rows = parents()
    result = {'binary': binary, 'dropMm': drop_mm, 'rows': []}
    for seed in seeds:
        parent = rows[seed]
        target = parent['rawDepthMm'] - drop_mm
        result['rows'].append(run(
            binary, seed, parent['fixture'], target,
            f'{outdir}/census-seed{seed}.json',
            'past=0,rollback=0,lanes=1,pconfirm=0'))
        print(json.dumps(result['rows'][-1]))
    good = [r for r in result['rows'] if 'skipRate' in r]
    if good:
        pairs = sum(r['pairs'] for r in good)
        clear = sum(r['provedClear'] for r in good)
        result['total'] = {
            'seeds': len(good),
            'calls': sum(r['calls'] for r in good),
            'pairs': pairs,
            'provedClear': clear,
            'skipRate': clear / pairs,
            'perSeedSkipRate': sorted(r['skipRate'] for r in good),
        }
    os.makedirs(outdir, exist_ok=True)
    json.dump(result, open(f'{outdir}/census.json', 'w'), indent=1)
    print(json.dumps(result.get('total'), indent=1))


if __name__ == '__main__':
    main()
