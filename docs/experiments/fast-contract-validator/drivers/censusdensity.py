#!/usr/bin/env python3
"""The skip rate as a function of layout density, not at one density.

    python3 censusdensity.py OUTDIR BINARY

docs/experiments/fast-contract-validator/ §5 states the caveat this closes:

  > "The 96% is measured at one density. All twelve parents are mixed-61 at
  >  171-179 mm. The skip rate at the 155 mm record line, where the layout is
  >  much tighter, was not measured and could be materially lower."

The filter proves pairs *clear*, so the prediction is monotone: pack the same
pieces into less depth and fewer pairs are far enough apart to certify. That is
a falsifiable prediction and this is the falsification attempt. It walks the
campaign's own pinned mixed-61 layouts - the same 61 pieces on the same sheet -
from the 179.6 mm top of the census band down to the 155.264 mm record line, and
reports `skipRate` against `rawSourceDepthMm` for each.

The three other request fixtures are appended at their own pinned parents. Those
are not part of the density series - different pieces, different sheets' worth of
occupancy - and they are here because the skip rate is a property of a *layout*,
so a 17-piece and an 8-piece one are coverage the mixed-61-only census lacked.

Every row is one process with `POLYGON_NESTING_CONTRACT_VALIDATOR_CENSUS=1`. The
census is a separate `O(n^2)` pass that never touches the timed loop and reports
on stderr, so no row here is a wall claim and none is comparable to §3.2.

Rows whose `calls` is zero are reported rather than dropped: a schedule that
never reaches a confirmation never calls the validator, and on some fixtures
that is the finding.
"""
import hashlib
import json
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import runlib  # noqa: E402

CENSUS = re.compile(
    r'contractValidatorCensus calls=(\d+) pairs=(\d+) provedClear=(\d+) '
    r'skipRate=([0-9.]+)')

ROOT = runlib.ROOT
TRUE = (f'{ROOT}/docs/experiments/persistent-vacancy-descent/exact-contract/'
        'true-contract')
MIXED = f'{ROOT}/tests/fixtures/mixed-61/mixed61-request-exact-clearance.json'
EVIDENCE = f'{ROOT}/docs/experiments/fast-contract-validator/evidence'
PARENTS = '/var/lib/t3/tmp/csched'

# label, request, mode, parent, target, allowance, seed, nominal depth
LADDER = [
    ('record-155.264', MIXED, 34,
     f'{TRUE}/orientation-floor/pinned-fs-155.26442950833.json',
     '154.964', '0.0005', '5', 155.264),
    ('record-159.079', MIXED, 22,
     f'{TRUE}/finer-ladder/pinned-parent-159.079.json',
     '159.87876', '0.0005', '5', 159.079),
    ('record-159.092', MIXED, 22,
     f'{TRUE}/record-159.092/pinned-parent-159.092.json',
     '159.892624', '0.0005', '5', 159.092),
    ('record-164.038', MIXED, 22,
     f'{TRUE}/finer-ladder/pinned-fs-parent-164.0376.json',
     '164.837568', '0.0005', '5', 164.038),
    # The other three request fixtures, at their own pinned parents. These are
    # not a density series - they are different pieces on the same sheet - and
    # they are here because the skip rate is a property of the *layout*, so a
    # 17-piece and an 8-piece layout are the coverage the previous round's
    # mixed-61-only census did not have.
    ('shapes-17', f'{ROOT}/tests/fixtures/shapes-17/2000x2700-compact/'
     'request.json', 34, f'{EVIDENCE}/parents/shapes-17.json',
     '199.849', '0.002', '0', 200.349),
    ('triangle-20', f'{ROOT}/tests/fixtures/triangle-20/2000x2700-compact/'
     'request.json', 34, f'{EVIDENCE}/parents/triangle-20.json',
     '69.227', '0.002', '0', 70.727),
    ('small-8', f'{ROOT}/tests/vectors/core/'
     'thread-equality-mixed61-8-piece-request.json', 34,
     f'{EVIDENCE}/parents/small-8.json',
     '68.752', '0.002', '0', 70.252),
]


def mixed_band():
    """The twelve pinned 171-179 mm parents: the density §3.1 measured."""
    rows = []
    for manifest in (f'{PARENTS}/parents/parents.json',
                     f'{PARENTS}/parents-rest/parents.json'):
        if not os.path.exists(manifest):
            continue
        for row in json.load(open(manifest))['rows']:
            if 'fixture' not in row:
                continue
            depth = row['rawDepthMm']
            rows.append((f'band-seed{row["seed"]}', MIXED, 34, row['fixture'],
                         f'{depth - 1.5:.17g}', runlib.DEFAULT_ALLOWANCE,
                         str(row['seed']), depth))
    return rows


def run(binary, label, request, mode, parent, target, allowance, seed, outdir):
    args = [a.format(seed=seed) for a in runlib.ARGS]
    tail = [str(mode), parent, target, '', allowance]
    env = dict(os.environ)
    env.pop('POLYGON_NESTING_PROFILE', None)
    env['POLYGON_NESTING_CONTRACT_VALIDATOR_CENSUS'] = '1'
    env['POLYGON_NESTING_COMPRESSION_SCHEDULE'] = \
        'past=0,rollback=0,lanes=1,pconfirm=0'
    path = f'{outdir}/{label}.json'
    with open(path, 'w') as handle:
        proc = subprocess.run([binary, request] + args + tail, stdout=handle,
                              stderr=subprocess.PIPE, check=False, env=env)
    stderr = (proc.stderr or b'').decode()
    match = CENSUS.search(stderr)
    row = {'label': label, 'mode': mode, 'exitCode': proc.returncode,
           'parent': parent}
    if match:
        row.update({'calls': int(match.group(1)), 'pairs': int(match.group(2)),
                    'provedClear': int(match.group(3)),
                    'skipRate': float(match.group(4))})
    else:
        row['calls'] = 0
        row['stderrTail'] = stderr[-300:]
    try:
        doc = json.load(open(path))
        pop = ((doc.get('relaxedDiagnostics') or {})
               .get('coupledDynamicSeparator') or {}).get(
                   'persistentVacancyPopulation') or {}
        schedule = pop.get('compressionSchedule') or {}
        row['rawSourceDepthMm'] = pop.get('rawSourceDepthMm')
        row['confirmationsAccepted'] = schedule.get('confirmationsAccepted')
        row['confirmationsAttempted'] = schedule.get('confirmationsAttempted')
        row['confirmationsSkippedInfeasible'] = schedule.get(
            'confirmationsSkippedInfeasible')
    except (json.JSONDecodeError, FileNotFoundError):
        pass
    return row


def main():
    outdir, binary = sys.argv[1], sys.argv[2]
    os.makedirs(outdir, exist_ok=True)
    result = {
        'binary': binary,
        'binarySha256': hashlib.sha256(open(binary, 'rb').read()).hexdigest(),
        'note': 'census only; no row here is a wall claim',
        'rows': [],
    }
    for entry in LADDER + mixed_band():
        label, request, mode, parent, target, allowance, seed, nominal = entry
        if not os.path.exists(parent):
            result['rows'].append({'label': label, 'missing': parent})
            print(f'{label}: MISSING PARENT', file=sys.stderr)
            continue
        row = run(binary, label, request, mode, parent, target, allowance,
                  seed, outdir)
        row['nominalDepthMm'] = nominal
        result['rows'].append(row)
        print(f'{label}: depth={row.get("rawSourceDepthMm")} '
              f'calls={row.get("calls")} skip={row.get("skipRate")}',
              file=sys.stderr)
        json.dump(result, open(f'{outdir}/censusdensity.json', 'w'), indent=1)

    live = [r for r in result['rows'] if r.get('calls')]
    band = [r for r in live if r['label'].startswith('band-')]
    record = [r for r in live if r['label'].startswith('record-')]
    others = [r for r in live
              if not r['label'].startswith(('band-', 'record-'))]
    result['summary'] = {
        'rowsWithCalls': len(live),
        'rowsWithoutCalls': len(result['rows']) - len(live),
        'band171to179': aggregate(band),
        'recordLine155to164': aggregate(record),
        'otherFixtures': {r['label']: {'skipRate': r['skipRate'],
                                       'calls': r['calls'], 'pairs': r['pairs'],
                                       'depthMm': r.get('rawSourceDepthMm')}
                          for r in others},
        'byDepth': sorted(
            [{'label': r['label'], 'depthMm': r.get('nominalDepthMm'),
              'skipRate': r['skipRate'], 'calls': r['calls'],
              'pairs': r['pairs']} for r in live],
            key=lambda r: r['depthMm'] or 0),
    }
    json.dump(result, open(f'{outdir}/censusdensity.json', 'w'), indent=1)
    print(json.dumps(result['summary'], indent=1))


def aggregate(rows):
    if not rows:
        return None
    pairs = sum(r['pairs'] for r in rows)
    clear = sum(r['provedClear'] for r in rows)
    return {'rows': len(rows), 'calls': sum(r['calls'] for r in rows),
            'pairs': pairs, 'provedClear': clear, 'skipRate': clear / pairs,
            'perRowSkipRate': sorted(round(r['skipRate'], 5) for r in rows),
            'depthRangeMm': [min(r['nominalDepthMm'] for r in rows),
                             max(r['nominalDepthMm'] for r in rows)]}


if __name__ == '__main__':
    main()
