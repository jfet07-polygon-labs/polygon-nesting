#!/usr/bin/env python3
"""Sol's cheap A/B: accepted witness -> child frontier -> one m34 batch.

    witnessab.py OUTDIR BINARY PARENTSJSON WITNESS [DROP_MM] [ALLOWANCE]

`WITNESS` is `trust:iterations:maxcalls`, the `POLYGON_NESTING_SE2_WITNESS`
form. Three arms per parent, on one binary, at the **same work cap**, from the
same pinned parent:

    off      no witness at all - the schedule's own slice, the floor
    publish  witness on, `adopt = 0` - design C exactly as the sparse-rotation
             round shipped it: an accepted witness updates `publishedDepthMm`
             and `publishedPlacements` and nothing else
    adopt    witness on, `adopt = 1` - the accepted witness additionally becomes
             `confirmed_state` and the live frontier, so every later step,
             sweep and confirmation in the slice descends from it

The question is the one Sol review 8 §"Design C e verdetto null" says the
sparse-rotation round could not answer: its 0/12 measured that a one-shot
publication is later dominated, not that `witness -> m34` fails to compose,
because "C aggiorna solo `published_depth_mm/placements`; non aggiorna `state`,
`confirmed_state`, floor o archive". The `adopt` arm is that missing wire, and
the statistic is **descendant publications**: parents whose final published
depth is strictly better under `adopt` than under `publish`.

Sol's own stopping rule, quoted: *"Se resta 0/12 descendant publications, taglio
witness/m33 dalla produzione."* This driver reports exactly that fraction.
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

# The compression-schedule anatomy's design slice, in the schedule's own
# currency, and the same cap `sparse-rotation/drivers/workgate.py` used - so a
# cell here is the same slice that round priced the witness inside.
DESIGN_SLICE_UNITS = 3_341_379
DEFAULT_DROP_MM = 0.3
SPEC = 'past=1,rollback=0,work={work},lanes=1,pconfirm=0'

# Design B on the equivariant construction, which is what design C is bolted
# onto: the witness only fires when B's stall outlives a whole step.
BASE_ENV = {
    'POLYGON_NESTING_CONTINUOUS_ROTATION': '1',
    'POLYGON_NESTING_SPARSE_ROTATION': '1',
    'POLYGON_NESTING_ROTATION_EQUIVARIANT': '1',
}

WITNESS_KEYS = ('se2WitnessCalls', 'se2WitnessAccepted', 'se2WitnessAdoptions',
                'se2WitnessMs', 'se2WitnessBoughtMm',
                'sparseRotationEpisodes', 'sparseRotationSweeps',
                'sparseRotationRungsProposed', 'sparseRotationRungWinners',
                'sparseRotationCommittedMoves',
                'sparseRotationCommittedEpisodes',
                'rotationAcceptedMoves', 'acceptedMoves',
                'stepsTaken', 'confirmationsAttempted',
                'confirmationsAccepted', 'finalDepthMm')


def population(doc):
    return ((doc.get('relaxedDiagnostics') or {})
            .get('coupledDynamicSeparator') or {}).get(
                'persistentVacancyPopulation')


def run_arm(binary, seed, fixture, target, env_extra, out_path, allowance):
    args = [a.format(seed=seed) for a in runlib.ARGS]
    tail = ['34', fixture, f'{target:.17g}', '', allowance]
    command = [binary, runlib.REQUESTS['mixed-61']] + args + tail
    env = dict(os.environ)
    env['POLYGON_NESTING_PROFILE'] = '1'
    env['POLYGON_NESTING_COMPRESSION_SCHEDULE'] = SPEC.format(
        work=int(os.environ.get('WITNESSAB_UNITS', DESIGN_SLICE_UNITS)))
    env.update(BASE_ENV)
    env.pop('POLYGON_NESTING_SE2_WITNESS', None)
    env.update(env_extra)
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
    raw = pop.get('rawSourceDepthMm')
    row['rawSourceDepthMm'] = raw if raw is not None else parent_depth
    row['deltaMm'] = parent_depth - row['rawSourceDepthMm']
    row['fingerprint'] = pop.get('finalPlacementFingerprint')
    schedule = pop.get('compressionSchedule') or {}
    row['slice'] = {k: schedule.get(k) for k in WITNESS_KEYS}
    return row


def main():
    outdir, binary, parents_json, witness = sys.argv[1:5]
    drop_mm = float(sys.argv[5]) if len(sys.argv) > 5 else DEFAULT_DROP_MM
    allowance = sys.argv[6] if len(sys.argv) > 6 else runlib.DEFAULT_ALLOWANCE
    parents = json.load(open(parents_json))['rows']
    os.makedirs(outdir, exist_ok=True)
    arms = [
        ('off', {}),
        ('publish', {'POLYGON_NESTING_SE2_WITNESS': f'{witness}:0'}),
        ('adopt', {'POLYGON_NESTING_SE2_WITNESS': f'{witness}:1'}),
    ]
    result = {
        'binary': binary,
        'binarySha256': hashlib.sha256(open(binary, 'rb').read()).hexdigest(),
        'workUnits': int(os.environ.get('WITNESSAB_UNITS',
                                        DESIGN_SLICE_UNITS)),
        'dropMm': drop_mm, 'allowance': allowance, 'witness': witness,
        'baseEnv': BASE_ENV,
        'arms': {label: env for label, env in arms},
        'cells': [],
    }
    for parent in parents:
        seed = parent['seed']
        parent_depth = parent['rawDepthMm']
        target = parent_depth - drop_mm
        cell = {'seed': seed, 'parentRawDepthMm': parent_depth, 'arms': {}}
        for label, env_extra in arms:
            path = f'{outdir}/seed{seed}-{label}.json'
            doc, wall, err = run_arm(binary, seed, parent['fixture'], target,
                                     env_extra, path, allowance)
            cell['arms'][label] = ({'error': err} if doc is None
                                   else row_for(doc, wall, parent_depth))
        depths = {k: v.get('rawSourceDepthMm') for k, v in cell['arms'].items()}
        cell['depths'] = depths
        if depths.get('adopt') is not None and depths.get('publish') is not None:
            cell['adoptMinusPublishMm'] = depths['adopt'] - depths['publish']
            # A descendant publication: the adopted witness carried the slice
            # to a strictly better final depth than the one-shot publication
            # did. This is the 0/12 Sol's stopping rule is stated over.
            cell['descendantPublication'] = cell['adoptMinusPublishMm'] < 0.0
        if depths.get('publish') is not None and depths.get('off') is not None:
            cell['publishMinusOffMm'] = depths['publish'] - depths['off']
        print(f'seed {seed}: parent={parent_depth:.4f} '
              f'off={depths.get("off")} publish={depths.get("publish")} '
              f'adopt={depths.get("adopt")} '
              f'descendant={cell.get("descendantPublication")}', flush=True)
        result['cells'].append(cell)

    graded = [c for c in result['cells'] if 'descendantPublication' in c]
    deltas = [c['adoptMinusPublishMm'] for c in graded]
    accepted = sum((c['arms'].get('publish', {}).get('slice') or {})
                   .get('se2WitnessAccepted') or 0 for c in result['cells'])
    adoptions = sum((c['arms'].get('adopt', {}).get('slice') or {})
                    .get('se2WitnessAdoptions') or 0 for c in result['cells'])
    accepted_adopt = sum((c['arms'].get('adopt', {}).get('slice') or {})
                         .get('se2WitnessAccepted') or 0
                         for c in result['cells'])
    result['summary'] = {
        'cells': len(result['cells']),
        'graded': len(graded),
        'descendantPublications': sum(
            1 for c in graded if c['descendantPublication']),
        'medianAdoptMinusPublishMm': (statistics.median(deltas)
                                      if deltas else None),
        'adoptBetter': sum(1 for d in deltas if d < 0),
        'adoptWorse': sum(1 for d in deltas if d > 0),
        'tied': sum(1 for d in deltas if d == 0),
        'witnessAcceptedPublishArm': accepted,
        'witnessAcceptedAdoptArm': accepted_adopt,
        'witnessAdoptionsAdoptArm': adoptions,
    }
    json.dump(result, open(f'{outdir}/witnessab.json', 'w'), indent=1)
    print(json.dumps(result['summary'], indent=1))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
