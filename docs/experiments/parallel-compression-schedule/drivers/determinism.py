#!/usr/bin/env python3
"""The hard gate: two processes, work-budget mode, byte-identical documents.

    python3 determinism.py OUTDIR BINARY REPEATS

Grok's action 2 makes this a gate rather than a nicety: "if the parallel
schedule cannot be made deterministic in work mode, deliver the obstacle
analysis instead of a nondeterministic feature". So it is measured the way the
requirement is worded - across *processes*, under a **work** cap, on the armed
arms - and the statistic is a whole-document digest with the wall-clock fields
removed, not a scalar.

The digest is the campaign's repaired `doc_digest`: it drops `elapsedMs` and
everything the benchmark computes from it, the binary's own hash, and the
worktree status. Everything else is compared, including every one of the
schedule's step rows and the fan-out's lane-win histogram.
"""
import hashlib
import json
import os
import subprocess
import sys
import time

DRIVERS = ('/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/'
           'wf_960e7225-201-2/docs/experiments/parallel-compression-schedule/'
           'drivers')
sys.path.insert(0, DRIVERS)
import lib  # noqa: E402
import runlib  # noqa: E402

PARENTS = '/var/lib/t3/tmp/pl34/parents'
DESIGN_SLICE_UNITS = 3_341_379
ARMS = {
    'serial': f'past=1,rollback=0,work={DESIGN_SLICE_UNITS},lanes=1,pconfirm=0',
    'lanes8': f'past=1,rollback=0,work={DESIGN_SLICE_UNITS},lanes=8,pconfirm=0',
    'pconfirm': f'past=1,rollback=0,work={DESIGN_SLICE_UNITS},lanes=1,pconfirm=1',
    'both': f'past=1,rollback=0,work={DESIGN_SLICE_UNITS},lanes=8,pconfirm=1',
}


def run(binary, seed, fixture, target, spec, path):
    args = [a.format(seed=seed) for a in runlib.ARGS]
    tail = ['34', fixture, f'{target:.17g}', '', runlib.DEFAULT_ALLOWANCE]
    env = dict(os.environ)
    env.pop('POLYGON_NESTING_PROFILE', None)
    env['POLYGON_NESTING_COMPRESSION_SCHEDULE'] = spec
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, 'w') as handle:
        subprocess.run([binary, runlib.REQUESTS['mixed-61']] + args + tail,
                       stdout=handle, stderr=subprocess.DEVNULL, check=False,
                       env=env)
    return json.load(open(path))


def main():
    outdir, binary, repeats = sys.argv[1], sys.argv[2], int(sys.argv[3])
    parents = json.load(open(f'{PARENTS}/parents.json'))['rows'][:3]
    result = {'binary': binary,
              'binarySha256': hashlib.sha256(open(binary, 'rb').read()).hexdigest(),
              'repeats': repeats, 'budget': 'work', 'arms': ARMS, 'cells': []}
    for arm, spec in ARMS.items():
        for parent in parents:
            target = parent['rawDepthMm'] - 0.3
            digests, depths, fingerprints, walls = [], [], [], []
            for run_index in range(repeats):
                started = time.monotonic()
                doc = run(binary, parent['seed'], parent['fixture'], target,
                          spec, f'{outdir}/{arm}-s{parent["seed"]}-r{run_index}.json')
                walls.append(time.monotonic() - started)
                digests.append(lib.doc_digest(doc))
                pop = ((doc.get('relaxedDiagnostics') or {})
                       .get('coupledDynamicSeparator') or {}).get(
                           'persistentVacancyPopulation') or {}
                depths.append(pop.get('rawSourceDepthMm'))
                fingerprints.append(pop.get('finalPlacementFingerprint'))
            result['cells'].append({
                'arm': arm, 'seed': parent['seed'],
                'processes': repeats,
                'distinctDocDigests': len(set(digests)),
                'docDigest': digests[0],
                'distinctDepths': len(set(depths)),
                'rawSourceDepthMm': depths[0],
                'distinctFingerprints': len(set(fingerprints)),
                'reproducible': len(set(digests)) == 1,
                'processWallSeconds': walls,
            })
            json.dump(result, open(f'{outdir}/determinism.json', 'w'), indent=1)
    result['ALL_REPRODUCIBLE'] = all(c['reproducible'] for c in result['cells'])
    # The second half of the claim: the two levers are semantics-preserving
    # (`pconfirm`) or not (`lanes8`), and the digests say which.
    by_key = {(c['arm'], c['seed']): c for c in result['cells']}
    result['crossArm'] = [{
        'seed': seed,
        'pconfirmMatchesSerialDigest':
            by_key[('pconfirm', seed)]['docDigest']
            == by_key[('serial', seed)]['docDigest'],
        'pconfirmMatchesSerialDepth':
            by_key[('pconfirm', seed)]['rawSourceDepthMm']
            == by_key[('serial', seed)]['rawSourceDepthMm'],
        'lanes8MatchesSerialDepth':
            by_key[('lanes8', seed)]['rawSourceDepthMm']
            == by_key[('serial', seed)]['rawSourceDepthMm'],
    } for seed in sorted({c['seed'] for c in result['cells']})]
    json.dump(result, open(f'{outdir}/determinism.json', 'w'), indent=1)
    print(json.dumps({'ALL_REPRODUCIBLE': result['ALL_REPRODUCIBLE'],
                      'cells': [{k: c[k] for k in
                                 ('arm', 'seed', 'processes',
                                  'distinctDocDigests', 'distinctDepths',
                                  'reproducible')}
                                for c in result['cells']],
                      'crossArm': result['crossArm']}, indent=1))


if __name__ == '__main__':
    main()
