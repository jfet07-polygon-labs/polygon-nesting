#!/usr/bin/env python3
"""The §1 decomposition, on an isolated mode-34 slice at equal work.

    taxprobe.py OUTDIR BINARY PARENTSJSON [DROP_MM] [ALLOWANCE] [SEEDS]

`workgate.py`'s replay - one pinned parent, one serial mode-34 slice, a fixed
work cap, both arms of `POLYGON_NESTING_CONTINUOUS_ROTATION` on one binary -
with the `rotation-tax-census` line read off stderr beside it.

# Why the decomposition is taken here and not from a from-request run

A ten-second from-request run is the wrong instrument for this question twice
over. Its mode-34 slices are a *variable* fraction of the budget, so an armed
arm and an unarmed arm do not run the same number of them and the census totals
are not comparable; and the census build is slow enough that the coordinator's
phase structure moves under it, which is the instrument deciding what it
measures. A replay at a fixed work cap has neither problem: both arms get the
same number of proxy questions by construction, one lane, one slice, so every
counter below is per-slice and the two arms' counters differ only by what the
operator did.

The census build is still an instrument and never a wall claim. `sliceSeconds`
is printed for shape only.
"""
import hashlib
import json
import os
import re
import statistics
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import runlib  # noqa: E402
import workgate  # noqa: E402

CENSUS = re.compile(r'rotationTaxCensus (.*)')


def census_of(stderr):
    match = CENSUS.search(stderr or '')
    if not match:
        return {}
    out = {}
    for field in match.group(1).split():
        key, _, value = field.partition('=')
        out[key] = int(value)
    return out


def run_arm(binary, seed, fixture, target, armed, out_path, allowance):
    args = [a.format(seed=seed) for a in runlib.ARGS]
    tail = ['34', fixture, f'{target:.17g}', '', allowance]
    command = [binary, runlib.REQUESTS['mixed-61']] + args + tail
    env = dict(os.environ)
    env['POLYGON_NESTING_PROFILE'] = '1'
    env['POLYGON_NESTING_COMPRESSION_SCHEDULE'] = workgate.SPEC.format(
        work=int(os.environ.get('WORKGATE_UNITS',
                                workgate.DESIGN_SLICE_UNITS)))
    if armed:
        env['POLYGON_NESTING_CONTINUOUS_ROTATION'] = '1'
    else:
        env.pop('POLYGON_NESTING_CONTINUOUS_ROTATION', None)
    os.makedirs(os.path.dirname(out_path), exist_ok=True)
    started = time.monotonic()
    with open(out_path, 'w') as handle:
        proc = subprocess.run(command, stdout=handle, stderr=subprocess.PIPE,
                              check=False, env=env)
    wall = time.monotonic() - started
    stderr = (proc.stderr or b'').decode()
    try:
        return json.load(open(out_path)), wall, stderr
    except json.JSONDecodeError:
        return None, wall, stderr


def main():
    outdir, binary, parents_json = sys.argv[1], sys.argv[2], sys.argv[3]
    drop_mm = float(sys.argv[4]) if len(sys.argv) > 4 else workgate.DEFAULT_DROP_MM
    allowance = sys.argv[5] if len(sys.argv) > 5 else runlib.DEFAULT_ALLOWANCE
    wanted = ({int(s) for s in sys.argv[6].split(',')}
              if len(sys.argv) > 6 else None)
    parents = [row for row in json.load(open(parents_json))['rows']
               if wanted is None or row['seed'] in wanted]
    os.makedirs(outdir, exist_ok=True)
    result = {
        'binary': binary,
        'binarySha256': hashlib.sha256(open(binary, 'rb').read()).hexdigest(),
        'workUnits': int(os.environ.get('WORKGATE_UNITS',
                                        workgate.DESIGN_SLICE_UNITS)),
        'dropMm': drop_mm, 'allowance': allowance, 'parents': parents_json,
        'cells': [],
    }
    for parent in parents:
        seed, parent_depth = parent['seed'], parent['rawDepthMm']
        cell = {'seed': seed, 'parentRawDepthMm': parent_depth, 'arms': {}}
        for label, armed in (('base', False), ('crot', True)):
            path = f'{outdir}/seed{seed}-{label}.json'
            doc, wall, err = run_arm(binary, seed, parent['fixture'],
                                     parent_depth - drop_mm, armed, path,
                                     allowance)
            row = ({'error': err[-600:]} if doc is None
                   else workgate.row_for(doc, wall, parent_depth))
            row['census'] = census_of(err)
            cell['arms'][label] = row
        cell['deltaMm'] = (cell['arms']['crot'].get('rawSourceDepthMm', 0)
                           - cell['arms']['base'].get('rawSourceDepthMm', 0))
        result['cells'].append(cell)
        base_wall = cell['arms']['base'].get('processWallSeconds')
        crot_wall = cell['arms']['crot'].get('processWallSeconds')
        print(f"seed{seed}: base={base_wall}s crot={crot_wall}s "
              f"delta={cell['deltaMm']:+.4f}mm", flush=True)
        json.dump(result, open(f'{outdir}/taxprobe.json', 'w'), indent=1)

    keys = set()
    for cell in result['cells']:
        for arm in cell['arms'].values():
            keys |= set(arm.get('census') or {})
    summary = {'cells': len(result['cells'])}
    for label in ('base', 'crot'):
        walls = [c['arms'][label]['processWallSeconds'] for c in result['cells']
                 if 'processWallSeconds' in c['arms'][label]]
        summary[f'{label}WallSecondsMedian'] = statistics.median(walls) \
            if walls else None
        summary[f'{label}Census'] = {
            key: sum((c['arms'][label].get('census') or {}).get(key, 0)
                     for c in result['cells']) for key in sorted(keys)}
        summary[f'{label}Rotation'] = {
            key: sum((c['arms'][label].get('rotation') or {}).get(key) or 0
                     for c in result['cells'])
            for key in workgate.ROTATION_KEYS[1:]}
    result['summary'] = summary
    json.dump(result, open(f'{outdir}/taxprobe.json', 'w'), indent=1)
    print(json.dumps(summary, indent=1))


if __name__ == '__main__':
    main()
