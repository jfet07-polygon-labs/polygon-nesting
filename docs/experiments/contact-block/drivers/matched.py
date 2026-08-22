#!/usr/bin/env python3
"""Sol review 10 §3's gate: the block operator against the shipping m34/m22
continuation, from the same pinned parent, at **equal work**.

    matched.py OUTDIR BINARY PARENTSJSON BLOCKSPECS M34WORKS [DROP_MM] [ALLOW]

`BLOCKSPECS` is a comma-separated list of `POLYGON_NESTING_CONTACT_BLOCK`
strings with `,` written as `;` (the argument separator is already taken).
`M34WORKS` is a comma-separated list of work caps for the schedule spec.

# The equal-work axis

Three axes are reported, because the arms do not spend work in the same shape
and any one of them alone can be argued with:

* **`processWorkUnits = candidateQueries + 5 * exactPairTests`**, the
  portfolio's own meter, measured on the process rather than declared. This
  became available to the block arm only with the gate correction: the operator
  now validates through `validate_and_measure_placements`, whose pair phase goes
  through `search::kernel::exact` and therefore increments
  `Counter::ExactPairTests`. Under the retracted contract-only gate the same
  column read **exactly zero**, because `validate_publication` alone does not
  reach that kernel - which is worth keeping in mind for any future in-search
  wiring, since an operator invisible to the work meter would spend seconds
  without spending budget and a naive equal-work gate would call that a win.
* **whole-layout exact validations** - the block's `validations` against the
  slice's `confirmationsAttempted`. Both are calls to the same composite check
  on the same 61 pieces, so one of each costs the same thing.
* **wall seconds** - the currency the ten-second contract is written in. It
  carries the control's 2-3 s of process startup, so `slicetime.py` reports the
  operator-only version beside it.

Each arm is run at several budgets so the comparison is read off a curve rather
than off one cell that happened to land where the author wanted it.

# The two arms

* `block`: the diagnostic door. It reads the pinned parent, runs the operator,
  and returns; no search runs, so its whole work counter is the operator's.
* `m34`: `workgate.py`'s arm verbatim - one serial mode-34 slice from the same
  pinned parent at `past=1,rollback=0,work=W,lanes=1,pconfirm=0`, scored on the
  raw source depth of the best exact-valid publication with the parent as the
  floor.

Both arms carry `POLYGON_NESTING_PROFILE=1`, because the x-axis is a counter.
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

SPEC = 'past=1,rollback=0,work={work},lanes=1,pconfirm=0'
DEFAULT_DROP_MM = 0.3


def population(doc):
    return ((doc.get('relaxedDiagnostics') or {})
            .get('coupledDynamicSeparator') or {}).get(
                'persistentVacancyPopulation')


def run_m34(binary, seed, fixture, target, work, out_path, allowance):
    args = [a.format(seed=seed) for a in runlib.ARGS]
    command = ([binary, runlib.REQUESTS['mixed-61']] + args
               + ['34', fixture, f'{target:.17g}', '', allowance])
    env = dict(os.environ)
    env['POLYGON_NESTING_PROFILE'] = '1'
    env['POLYGON_NESTING_COMPRESSION_SCHEDULE'] = SPEC.format(work=int(work))
    for name in ('POLYGON_NESTING_CONTACT_BLOCK', 'POLYGON_NESTING_SE2_WITNESS',
                 'POLYGON_NESTING_CONTINUOUS_ROTATION',
                 'POLYGON_NESTING_SPARSE_ROTATION'):
        env.pop(name, None)
    os.makedirs(os.path.dirname(out_path), exist_ok=True)
    started = time.monotonic()
    with open(out_path, 'w') as handle:
        proc = subprocess.run(command, stdout=handle, stderr=subprocess.PIPE,
                              check=False, env=env)
    wall = time.monotonic() - started
    try:
        doc = json.load(open(out_path))
    except json.JSONDecodeError:
        return {'error': (proc.stderr or b'').decode()[-800:]}, wall
    profile = (doc.get('searchProfile') or {}).get('counters') or {}
    queries = profile.get('candidateQueries', 0)
    tests = profile.get('exactPairTests', 0)
    row = {
        'processCandidateQueries': queries,
        'processExactPairTests': tests,
        'processWorkUnits': queries + 5 * tests,
        'processWallSeconds': wall,
    }
    pop = population(doc)
    if pop is None:
        row['error'] = 'no population'
        return row, wall
    row['exactValid'] = pop.get('exactValid')
    row['contractValid'] = pop.get('contractValid')
    row['rawSourceDepthMm'] = pop.get('rawSourceDepthMm')
    row['fingerprint'] = pop.get('finalPlacementFingerprint')
    schedule = pop.get('compressionSchedule') or {}
    row['scheduleWorkUnits'] = schedule.get('workUnits')
    row['confirmationsAttempted'] = schedule.get('confirmationsAttempted')
    row['confirmationsAccepted'] = schedule.get('confirmationsAccepted')
    row['stepsTaken'] = schedule.get('stepsTaken')
    return row, wall


def run_block(binary, seed, fixture, spec, out_path, allowance):
    doc, wall, err, code = runlib.probe(
        binary, 'mixed-61', seed, fixture,
        {'POLYGON_NESTING_CONTACT_BLOCK': spec,
         'POLYGON_NESTING_PROFILE': '1'},
        out_path, allowance=allowance, timeout=3600)
    if doc is None:
        return {'error': err[-800:], 'exitCode': code}, wall
    rounds = doc.get('rounds') or []
    refusals = {}
    for entry in rounds:
        key = entry.get('refusal') or 'moved'
        refusals[key] = refusals.get(key, 0) + 1
    headrooms = [e['headroomMm'] for e in rounds
                 if e.get('headroomMm') is not None
                 and e['headroomMm'] != float('inf')]
    fulls = [e['fullStepExactValid'] for e in rounds if e.get('rows')]
    return {
        'processCandidateQueries': doc.get('processCandidateQueries'),
        'processExactPairTests': doc.get('processExactPairTests'),
        'processWorkUnits': doc.get('processWorkUnits'),
        'processWallSeconds': wall,
        'rawSourceDepthMm': doc.get('finalDepthMm'),
        'parentDepthMm': doc.get('parentDepthMm'),
        'deltaMm': doc.get('deltaMm'),
        'rounds': len(rounds),
        'roundsAccepted': doc.get('roundsAccepted'),
        'solves': doc.get('solves'),
        'validations': doc.get('validations'),
        'operatorMs': doc.get('elapsedMs'),
        'refusals': refusals,
        'medianHeadroomMm': statistics.median(headrooms) if headrooms else None,
        'fullStepExactValidRate': (sum(1 for f in fulls if f) / len(fulls)
                                   if fulls else None),
        'medianBlockSize': statistics.median(
            [len(e['block']) for e in rounds]) if rounds else None,
    }, wall


def main():
    outdir, binary, parents_json = sys.argv[1], sys.argv[2], sys.argv[3]
    block_specs = [s.replace(';', ',') for s in sys.argv[4].split(',')]
    m34_works = [int(w) for w in sys.argv[5].split(',')]
    drop_mm = float(sys.argv[6]) if len(sys.argv) > 6 else DEFAULT_DROP_MM
    allowance = sys.argv[7] if len(sys.argv) > 7 else runlib.DEFAULT_ALLOWANCE
    parents = json.load(open(parents_json))['rows']
    os.makedirs(outdir, exist_ok=True)
    result = {
        'binary': binary,
        'binarySha256': hashlib.sha256(open(binary, 'rb').read()).hexdigest(),
        'parents': parents_json,
        'blockSpecs': block_specs,
        'm34Works': m34_works,
        'dropMm': drop_mm,
        'allowance': allowance,
        'cells': [],
    }
    for parent in parents:
        seed = parent['seed']
        parent_depth = parent['rawDepthMm']
        target = parent_depth - drop_mm
        cell = {'seed': seed, 'parentRawDepthMm': parent_depth, 'arms': {}}
        for spec in block_specs:
            tag = 'block-' + spec.replace('=', '').replace(',', '-') \
                                  .replace('.', 'p')
            row, _ = run_block(binary, seed, parent['fixture'], spec,
                               f'{outdir}/seed{seed}-{tag}.json', allowance)
            row.setdefault('rawSourceDepthMm', parent_depth)
            cell['arms'][f'block:{spec}'] = row
        for work in m34_works:
            row, _ = run_m34(binary, seed, parent['fixture'], target, work,
                             f'{outdir}/seed{seed}-m34-{work}.json', allowance)
            if row.get('rawSourceDepthMm') is None:
                row['rawSourceDepthMm'] = parent_depth
            cell['arms'][f'm34:{work}'] = row
        for label, row in cell['arms'].items():
            if 'rawSourceDepthMm' in row:
                row['deltaVsParentMm'] = parent_depth - row['rawSourceDepthMm']
        print(f'seed{seed} parent={parent_depth:.4f} ' + ' '.join(
            f"{label}=({row.get('deltaVsParentMm', 0):.4f}mm,"
            f"{(row.get('processWorkUnits') or 0)/1e6:.2f}Mw,"
            f"{row.get('processWallSeconds', 0):.1f}s)"
            for label, row in cell['arms'].items()), flush=True)
        result['cells'].append(cell)
        json.dump(result, open(f'{outdir}/matched.json', 'w'), indent=1)

    labels = list(result['cells'][0]['arms']) if result['cells'] else []
    summary = {}
    for label in labels:
        rows = [cell['arms'][label] for cell in result['cells']
                if 'deltaVsParentMm' in cell['arms'].get(label, {})]
        if not rows:
            continue
        deltas = [r['deltaVsParentMm'] for r in rows]
        works = [(r.get('processWorkUnits') or 0) for r in rows]
        walls = [r.get('processWallSeconds', 0) for r in rows]
        # The commensurable cost: whole-layout `validate_publication` calls.
        # `validations` on the block arm, `confirmationsAttempted` on the slice.
        exacts = [(r.get('validations') if r.get('validations') is not None
                   else r.get('confirmationsAttempted')) or 0 for r in rows]
        per_work = [d / w * 1e6 for d, w in zip(deltas, works) if w > 0]
        per_exact = [d / e * 1e3 for d, e in zip(deltas, exacts) if e > 0]
        per_second = [d / w for d, w in zip(deltas, walls) if w > 0]
        summary[label] = {
            'cells': len(rows),
            'medianDeltaMm': statistics.median(deltas),
            'cellsMoved': sum(1 for d in deltas if d > 0),
            'medianWorkUnits': statistics.median(works),
            'medianWallSeconds': statistics.median(walls),
            'medianExactValidations': statistics.median(exacts),
            'medianMmPerMegaWork': (statistics.median(per_work)
                                    if per_work else None),
            'medianMmPerKiloExactValidation': (statistics.median(per_exact)
                                               if per_exact else None),
            'medianMmPerSecond': (statistics.median(per_second)
                                  if per_second else None),
        }
    result['summary'] = summary
    json.dump(result, open(f'{outdir}/matched.json', 'w'), indent=1)
    print(json.dumps(summary, indent=1))


if __name__ == '__main__':
    main()
