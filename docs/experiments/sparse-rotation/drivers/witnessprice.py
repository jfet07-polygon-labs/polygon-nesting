#!/usr/bin/env python3
"""What design C costs, priced against the slice it is spent out of.

    witnessprice.py OUTDIR BINARY PARENTSJSON SETTING[,SETTING...] [DROP] [ALLOW]

A `SETTING` is `trust:iterations:maxcalls`, exactly the form the `se2w` spec key
and `POLYGON_NESTING_SE2_WITNESS` take.

The question this driver exists to answer is **not** "does the witness find
depth". docs/experiments/se2-rigidity/ already answered that, on pinned parents,
with four programs and six trust radii: the best exactly-validated SE(2)
reduction on the two record parents is 0.039 mm and 0.030 mm, and SE(2) beats
translation in 5 of 24 cells and only at trust radii below the crossover.

The question is whether a *search* can afford to ask. A mode-34 slice at a
ten-second wall is 0.78 s whole (docs/experiments/rotation-tax/ §4.2) and one
certificate call was measured at up to a second. So the statistic here is
`se2WitnessMs` as a fraction of the slice's own `repairMs + confirmationMs`,
reported whether or not the witness bought anything - because a proposal source
that costs a slice and returns nothing is the finding, not a null result to be
dropped.

Every cell also runs the same parent with the witness **off**, so the depth
column is a paired difference and not an absolute number that a reader has to
compare against another table.
"""
import hashlib
import json
import os
import statistics
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import armgate  # noqa: E402
import runlib  # noqa: E402
import workgate  # noqa: E402

# Design B on the equivariant construction, which is what design C is bolted
# onto: the witness only fires when B's stall outlives a whole step.
BASE_ENV = dict(armgate.ARMS['sparseEq'])

WITNESS_KEYS = ('se2WitnessCalls', 'se2WitnessAccepted', 'se2WitnessMs',
                'se2WitnessBoughtMm', 'sparseRotationEpisodes',
                'sparseRotationSweeps')


def slice_of(doc):
    pop = workgate.population(doc) or {}
    return pop.get('compressionSchedule') or {}


def main():
    outdir, binary, parents_json = sys.argv[1], sys.argv[2], sys.argv[3]
    settings = sys.argv[4].split(',')
    drop_mm = float(sys.argv[5]) if len(sys.argv) > 5 \
        else workgate.DEFAULT_DROP_MM
    allowance = sys.argv[6] if len(sys.argv) > 6 else runlib.DEFAULT_ALLOWANCE
    parents = json.load(open(parents_json))['rows']
    os.makedirs(outdir, exist_ok=True)
    result = {
        'binary': binary,
        'binarySha256': hashlib.sha256(open(binary, 'rb').read()).hexdigest(),
        'workUnits': int(os.environ.get('WORKGATE_UNITS',
                                        workgate.DESIGN_SLICE_UNITS)),
        'dropMm': drop_mm, 'allowance': allowance,
        'settings': settings, 'baseEnv': BASE_ENV, 'cells': [],
    }
    arms = [('off', dict(BASE_ENV))]
    for setting in settings:
        env = dict(BASE_ENV)
        env['POLYGON_NESTING_SE2_WITNESS'] = setting
        arms.append((f'se2:{setting}', env))

    for parent in parents:
        seed = parent['seed']
        parent_depth = parent['rawDepthMm']
        target = parent_depth - drop_mm
        cell = {'seed': seed, 'parentRawDepthMm': parent_depth, 'arms': {}}
        for label, env in arms:
            tag = label.replace(':', '-').replace('.', 'p')
            path = f'{outdir}/seed{seed}-{tag}.json'
            doc, wall, err = armgate.run_arm(binary, seed, parent['fixture'],
                                             target, env, path, allowance)
            if doc is None:
                cell['arms'][label] = {'error': err}
                continue
            row = armgate.row_for(doc, wall, parent_depth)
            report = slice_of(doc)
            row['witness'] = {key: report.get(key) for key in WITNESS_KEYS}
            row['sliceMs'] = ((report.get('repairMs') or 0.0)
                              + (report.get('confirmationMs') or 0.0))
            witness_ms = row['witness']['se2WitnessMs'] or 0.0
            row['witnessShareOfSlice'] = (witness_ms / row['sliceMs']
                                          if row['sliceMs'] else None)
            cell['arms'][label] = row
        base = cell['arms']['off'].get('rawSourceDepthMm')
        for label, _ in arms[1:]:
            arm = cell['arms'][label].get('rawSourceDepthMm')
            if base is not None and arm is not None:
                cell['arms'][label]['deltaVsOffMm'] = arm - base
        print(f"seed{seed}: off={base} " + ' '.join(
            f"{label}={cell['arms'][label].get('rawSourceDepthMm')}"
            f"({cell['arms'][label].get('witness', {}).get('se2WitnessCalls')}"
            f" calls,"
            f"{cell['arms'][label].get('witness', {}).get('se2WitnessMs')}ms)"
            for label, _ in arms[1:]), flush=True)
        result['cells'].append(cell)
        json.dump(result, open(f'{outdir}/witnessprice.json', 'w'), indent=1)

    summary = {}
    for label, _ in arms:
        rows = [cell['arms'][label] for cell in result['cells']
                if 'rawSourceDepthMm' in cell['arms'].get(label, {})]
        if not rows:
            continue
        calls = sum((r['witness']['se2WitnessCalls'] or 0) for r in rows)
        accepted = sum((r['witness']['se2WitnessAccepted'] or 0) for r in rows)
        witness_ms = sum((r['witness']['se2WitnessMs'] or 0.0) for r in rows)
        deltas = [r['deltaVsOffMm'] for r in rows if 'deltaVsOffMm' in r]
        shares = [r['witnessShareOfSlice'] for r in rows
                  if r.get('witnessShareOfSlice') is not None]
        summary[label] = {
            'cells': len(rows),
            'calls': calls, 'accepted': accepted,
            'witnessMsTotal': witness_ms,
            'msPerCall': witness_ms / calls if calls else None,
            'medianShareOfSlice': statistics.median(shares) if shares else None,
            'maxShareOfSlice': max(shares) if shares else None,
            'boughtMmTotal': sum((r['witness']['se2WitnessBoughtMm'] or 0.0)
                                 for r in rows),
            'medianDeltaVsOffMm': statistics.median(deltas) if deltas else None,
            'betterCells': sum(1 for d in deltas if d < 0),
            'worseCells': sum(1 for d in deltas if d > 0),
            'medianWallSeconds': statistics.median(
                [r['processWallSeconds'] for r in rows]),
        }
    result['summary'] = summary
    json.dump(result, open(f'{outdir}/witnessprice.json', 'w'), indent=1)
    print(json.dumps(summary, indent=1))


if __name__ == '__main__':
    main()
