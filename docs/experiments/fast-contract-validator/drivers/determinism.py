#!/usr/bin/env python3
"""Two gates in one run: determinism across processes, and flag-off == flag-on.

    python3 determinism.py OUTDIR OFF_BINARY ON_BINARY REPEATS

The campaign's hard gate is determinism in **work-budget** mode across two
processes. This feature adds a second, stronger claim that is checked in the
same place because it is measured with the same instrument: the broad phase
changes no verdict, so a flag-on document must equal the flag-off document
*exactly*, not merely reproduce its scalars.

That equality is the whole design. Unlike `pconfirm`, which is semantics-
preserving on accepted confirmations but charges a refused one differently, this
filter is semantics-preserving on **every** input: the loop's only consumer reads
a threshold verdict, and a skip is a proof of that verdict. So there is no field
that is allowed to differ - which is also why the broad phase carries no counter
in the document and reports its census on stderr instead.

The digest is the campaign's repaired `doc_digest` from `gatelib`: it drops
`elapsedMs` and everything computed from it, the binary's own hash, and the
worktree status. Everything else is compared, including every step row the
schedule emits.
"""
import hashlib
import json
import os
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import gatelib  # noqa: E402
import runlib  # noqa: E402

PARENTS = '/var/lib/t3/tmp/csched'
DESIGN_SLICE_UNITS = 3_341_379

# NOT a new finding: this is the parallel-compression-schedule round's repair,
# re-applied because the copy this campaign's protocol points at predates it.
#
# `gatelib.py` here is the m34-wall-price `gatelib.py`, and that file's
# `VOLATILE` was assembled against the four *gate* documents. The
# parallel-compression-schedule round then hit exactly this on mode-34
# documents and fixed it in ITS `lib.py:160`, adding `repairMs`,
# `confirmationMs` and `currentPoseOverlaySetupMs` - the "second repair of this
# list", by its own comment. That fix lives in a file this round does not
# import, so the older copy reproduces the older bug.
#
# Re-measured here before extending it, because a borrowed reason is not a
# measurement (`leafdiff.py`, off-serial-s0, two processes of the SAME binary):
# of **26,989** leaves, exactly **2** differ, and both are these.
#
# `confirmationMs` is also the field this feature is *for*, so it is the one
# place flag-off and flag-on are expected to differ and the digest must not be
# asked to hold it. The unstripped leaf comparison is reported alongside so
# that the removal is visible rather than assumed.
SCHEDULE_VOLATILE = {'repairMs', 'confirmationMs', 'currentPoseOverlaySetupMs'}


def schedule_digest(doc):
    """`gatelib.doc_digest` with the schedule's two wall clocks also removed."""
    saved = set(gatelib.VOLATILE)
    gatelib.VOLATILE |= SCHEDULE_VOLATILE
    try:
        return gatelib.doc_digest(doc)
    finally:
        gatelib.VOLATILE = saved


def differing_leaves(first, second):
    """How many leaves differ, with only the inherited volatiles removed."""
    def leaves(node, path='', out=None):
        if out is None:
            out = {}
        if isinstance(node, dict):
            for key, value in node.items():
                leaves(value, f'{path}/{key}', out)
        elif isinstance(node, list):
            for index, value in enumerate(node):
                leaves(value, f'{path}/{index}', out)
        else:
            out[path] = node
        return out

    a = leaves(gatelib.strip_volatile(first))
    b = leaves(gatelib.strip_volatile(second))
    differ = [k for k in sorted(set(a) | set(b)) if a.get(k) != b.get(k)]
    return {'leaves': len(set(a) | set(b)), 'differing': len(differ),
            'fields': sorted({k.rsplit('/', 1)[-1] for k in differ})}
ARMS = {
    'serial': f'past=1,rollback=0,work={DESIGN_SLICE_UNITS},lanes=1,pconfirm=0',
    'pconfirm': f'past=1,rollback=0,work={DESIGN_SLICE_UNITS},lanes=1,pconfirm=1',
    'both': f'past=1,rollback=0,work={DESIGN_SLICE_UNITS},lanes=8,pconfirm=1',
}


def parents(limit=3):
    rows = []
    for manifest in (f'{PARENTS}/parents/parents.json',
                     f'{PARENTS}/parents-rest/parents.json'):
        if os.path.exists(manifest):
            rows += [r for r in json.load(open(manifest))['rows']
                     if 'fixture' in r]
    return sorted(rows, key=lambda r: r['seed'])[:limit]


def run(binary, seed, fixture, target, spec, path):
    args = [a.format(seed=seed) for a in runlib.ARGS]
    tail = ['34', fixture, f'{target:.17g}', '', runlib.DEFAULT_ALLOWANCE]
    env = dict(os.environ)
    env.pop('POLYGON_NESTING_PROFILE', None)
    env.pop('POLYGON_NESTING_CONTRACT_VALIDATOR_CENSUS', None)
    env['POLYGON_NESTING_COMPRESSION_SCHEDULE'] = spec
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, 'w') as handle:
        subprocess.run([binary, runlib.REQUESTS['mixed-61']] + args + tail,
                       stdout=handle, stderr=subprocess.DEVNULL, check=False,
                       env=env)
    return json.load(open(path))


def main():
    outdir, off_binary, on_binary = sys.argv[1], sys.argv[2], sys.argv[3]
    repeats = int(sys.argv[4])
    binaries = {'off': off_binary, 'on': on_binary}
    rows = parents()
    result = {
        'binaries': binaries,
        'binarySha256': {k: hashlib.sha256(open(v, 'rb').read()).hexdigest()
                         for k, v in binaries.items()},
        'repeats': repeats, 'budget': 'work', 'arms': ARMS, 'cells': [],
    }
    for flag, binary in binaries.items():
        for arm, spec in ARMS.items():
            for parent in rows:
                target = parent['rawDepthMm'] - 0.3
                digests, depths, fingerprints, walls = [], [], [], []
                docs = []
                for index in range(repeats):
                    started = time.monotonic()
                    doc = run(binary, parent['seed'], parent['fixture'], target,
                              spec,
                              f'{outdir}/{flag}-{arm}-s{parent["seed"]}-r{index}.json')
                    walls.append(time.monotonic() - started)
                    docs.append(doc)
                    digests.append(schedule_digest(doc))
                    pop = ((doc.get('relaxedDiagnostics') or {})
                           .get('coupledDynamicSeparator') or {}).get(
                               'persistentVacancyPopulation') or {}
                    depths.append(pop.get('rawSourceDepthMm'))
                    fingerprints.append(pop.get('finalPlacementFingerprint'))
                result['cells'].append({
                    'flag': flag, 'arm': arm, 'seed': parent['seed'],
                    'processes': repeats,
                    'distinctDocDigests': len(set(digests)),
                    'docDigest': digests[0],
                    'processToProcessLeaves': differing_leaves(docs[0],
                                                               docs[-1]),
                    'distinctDepths': len(set(depths)),
                    'rawSourceDepthMm': depths[0],
                    'distinctFingerprints': len(set(fingerprints)),
                    'reproducible': len(set(digests)) == 1,
                    'processWallSeconds': walls,
                })
                json.dump(result, open(f'{outdir}/determinism.json', 'w'),
                          indent=1)
    result['ALL_REPRODUCIBLE'] = all(c['reproducible'] for c in result['cells'])
    by_key = {(c['flag'], c['arm'], c['seed']): c for c in result['cells']}
    result['flagEquivalence'] = [{
        'arm': arm, 'seed': seed,
        'digestsEqual': (by_key[('off', arm, seed)]['docDigest']
                         == by_key[('on', arm, seed)]['docDigest']),
        'depthsEqual': (by_key[('off', arm, seed)]['rawSourceDepthMm']
                        == by_key[('on', arm, seed)]['rawSourceDepthMm']),
        # The unstripped reading: how many leaves differ between the two
        # binaries at all, and which. The expectation is the two wall clocks
        # and nothing else - `confirmationMs` because it is what the feature
        # moves, `repairMs` because it is a clock.
        'crossFlagLeaves': differing_leaves(
            json.load(open(f'{outdir}/off-{arm}-s{seed}-r0.json')),
            json.load(open(f'{outdir}/on-{arm}-s{seed}-r0.json'))),
        'offWallSeconds': by_key[('off', arm, seed)]['processWallSeconds'],
        'onWallSeconds': by_key[('on', arm, seed)]['processWallSeconds'],
    } for arm in ARMS for seed in sorted({c['seed'] for c in result['cells']})]
    result['ALL_FLAG_EQUIVALENT'] = all(c['digestsEqual']
                                        for c in result['flagEquivalence'])
    json.dump(result, open(f'{outdir}/determinism.json', 'w'), indent=1)
    print(json.dumps({
        'ALL_REPRODUCIBLE': result['ALL_REPRODUCIBLE'],
        'ALL_FLAG_EQUIVALENT': result['ALL_FLAG_EQUIVALENT'],
        'flagEquivalence': [{k: c[k] for k in
                             ('arm', 'seed', 'digestsEqual', 'depthsEqual')}
                            for c in result['flagEquivalence']],
        'nonReproducible': [{k: c[k] for k in ('flag', 'arm', 'seed',
                                               'distinctDocDigests')}
                            for c in result['cells'] if not c['reproducible']],
    }, indent=1))


if __name__ == '__main__':
    main()
