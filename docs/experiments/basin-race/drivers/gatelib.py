#!/usr/bin/env python3
"""Shared runner for the four pinned regression gates.

Byte-identical to `docs/experiments/se2-rigidity/drivers/lib.py` - the repaired
`doc_digest`, with the five elapsed-derived summary statistics and
`engineWorktreeStatus` stripped - with `ROOT` repointed at this worktree and
renamed so it cannot shadow this round's own `lib`-free drivers.

Point ROOT at the worktree; every driver here takes the benchmark binary as an
argument so a paired A/B can hold two of them side by side.
"""
import hashlib
import json
import os
import subprocess
import time

ROOT = '/var/lib/t3/src/macs/polygon-nesting/.claude/worktrees/wf_30e47560-32c-2'
REQ = f'{ROOT}/tests/fixtures/mixed-61/mixed61-request-exact-clearance.json'
TRUE = (f'{ROOT}/docs/experiments/persistent-vacancy-descent/exact-contract/'
        'true-contract')

# The pinned positional CLI tail every replay driver in this repository uses.
ARGS = ('1 4 0 0 0 0 1 0 0 1 1 0 16 4 8 {clamp} 0 5 5 24 8 40 10 10 5 {seed} '
        '0.005 0.001 1 6 0 0 0 structured 0 10 1 0 0 0 0').split()

# tag, mode, parent, target, trailing allowance, expected raw depth,
# expected fingerprint prefix.
GATES = [
    ('g1', 20, '/var/lib/t3/tmp/ex5-seed-native.json', '320.000', None,
     206.869, '8a7737381238fa4d'),
    ('g2', 22, f'{TRUE}/record-159.092/pinned-parent-159.092.json',
     '159.892624', '0.0005', 159.09233022733062, 'fa01012af1d559ae09c'),
    ('g3', 22, f'{TRUE}/finer-ladder/pinned-parent-159.079.json',
     '159.87876', '0.0005', 159.07876040364795, 'e28fba007f8031d49f'),
    ('g4', 22, f'{TRUE}/finer-ladder/pinned-fs-parent-164.0376.json',
     '164.837568', '0.0005', 164.0375677990678, '49f094d7e59a9008'),
]


def argv_for(binary, mode, parent, target, allowance, seed='5', clamp='0'):
    argv = ([binary, REQ] + [a.format(clamp=clamp, seed=seed) for a in ARGS]
            + [str(mode), parent, str(target)])
    if allowance:
        argv += ['', allowance]
    return argv


def run(binary, tag, mode, parent, target, allowance, outdir, env=None,
        seed='5'):
    os.makedirs(outdir, exist_ok=True)
    path = f'{outdir}/{tag}.json'
    environment = dict(os.environ)
    if env:
        environment.update(env)
    start = time.time()
    with open(path, 'w') as handle:
        proc = subprocess.run(
            argv_for(binary, mode, parent, target, allowance, seed=seed),
            stdout=handle, stderr=subprocess.PIPE, env=environment, check=False)
    wall = time.time() - start
    try:
        doc = json.load(open(path))
    except json.JSONDecodeError:
        doc = {'_loadError': (proc.stderr or b'').decode()[-2000:]}
    return doc, wall, (proc.stderr or b'').decode()


def run_gate(binary, gate, outdir, env=None, label=''):
    tag, mode, parent, target, allowance = gate[:5]
    return run(binary, f'{label}{tag}', mode, parent, target, allowance,
               outdir, env=env)


def collect(node, keys, path='', out=None):
    if out is None:
        out = {}
    if isinstance(node, dict):
        for key, value in node.items():
            if key in keys and not isinstance(value, (dict, list)):
                out.setdefault(path + '/' + key, value)
            collect(value, keys, path + '/' + key, out)
    elif isinstance(node, list):
        for index, value in enumerate(node):
            collect(value, keys, path + f'/{index}', out)
    return out


def gate_check(gate, doc):
    """The pinned fields of one gate, plus whether they reproduce."""
    tag = gate[0]
    depth, prefix = gate[5], gate[6]
    if '_loadError' in doc:
        return {'error': doc['_loadError'][:400], 'hit': False}
    if tag == 'g1':
        found = collect(doc, {'independentDepthMm', 'finalPlacementFingerprint',
                              'placementFingerprint'})
        depths = sorted({v for k, v in found.items()
                         if k.endswith('independentDepthMm') and v is not None})
        fps = sorted({v for k, v in found.items()
                      if 'ingerprint' in k and v is not None})
        return {'depths': depths,
                'fingerprints': fps,
                'hit': depth in depths
                       and any(f.startswith(prefix) for f in fps)}
    pop = (doc.get('relaxedDiagnostics', {})
              .get('coupledDynamicSeparator', {})
              .get('persistentVacancyPopulation'))
    if pop is None:
        return {'error': 'no population', 'hit': False}
    raw, fingerprint = pop.get('rawSourceDepthMm'), pop.get(
        'finalPlacementFingerprint') or ''
    return {'raw': raw, 'fp': fingerprint,
            'depth': pop.get('independentDepthMm'),
            'exactValid': pop.get('exactValid'),
            'contractValid': pop.get('contractValid'),
            'hit': raw == depth and fingerprint.startswith(prefix)}


# Fields that legitimately differ between two runs of the same binary, or
# between a profiled and an unprofiled build. Everything else is compared.
#
# This list was incomplete as inherited, which made `doc_digest` useless as a
# reproducibility instrument: it dropped `elapsedMs` but not the five summary
# statistics the benchmark computes *from* `elapsedMs`, so two runs of the SAME
# binary on the SAME gate hashed differently every time. Measured on this
# branch before the fix, flag-off against flag-off, all four gates differed. A
# digest mismatch therefore proved nothing and a match would have been luck.
#
# The five quartile/extremum fields below are the fix. `executableSha256` is
# the second addition and a different kind: it is the binary's own identity, the
# analogue of the `buildIdentity` and `binaryPath` already here, and it MUST
# differ whenever two binaries are compared - which is the entire use of this
# function. Leaving it in the digest made every cross-binary comparison fail on
# the one field guaranteed to differ.
#
# `docdiff.py` is the paired instrument that measured this and is the one to
# reach for when a digest does mismatch: it reports which leaf paths differ,
# against a same-binary noise floor.
VOLATILE = {
    'elapsedMs', 'elapsedSeconds', 'engineElapsedSeconds', 'wallMs',
    'durationMs', 'timestamp', 'totalMs', 'ms', 'processWallSeconds',
    'phaseProfile', 'phases', 'profile', 'leafSeconds', 'engineVersion',
    'buildIdentity', 'binaryPath', 'peakResidentBytes', 'allocatedBytes',
    # Summary statistics over `elapsedMs`; wall-clock, one per run.
    'medianElapsedMs', 'minElapsedMs', 'maxElapsedMs',
    'firstQuartileElapsedMs', 'thirdQuartileElapsedMs',
    # Build and source identity. `executableSha256` is the binary's own hash and
    # two binaries are the point of the comparison; the rest describe the
    # worktree the binary was built from, and `engineWorktreeStatus` in
    # particular changes every time any file in the tree is edited - so leaving
    # it in makes the digest a function of the author's editor, not the engine.
    'executableSha256', 'relevantSourceTreeSha256', 'engineWorktreeStatus',
    'engineCommit', 'engineWorktreeDirty',
}


def strip_volatile(node):
    if isinstance(node, dict):
        return {k: strip_volatile(v) for k, v in sorted(node.items())
                if k not in VOLATILE}
    if isinstance(node, list):
        return [strip_volatile(v) for v in node]
    if isinstance(node, float):
        return repr(node)
    return node


def doc_digest(doc):
    return hashlib.sha256(
        json.dumps(strip_volatile(doc), sort_keys=True).encode()).hexdigest()


def engine_seconds(doc):
    """The engine's own measured stream, in seconds.

    `medianElapsedMs` is the benchmark's own clock around the measured stream
    only — request loading, probe setup and result serialisation are outside it
    — so it is the arm's number and the process wall is the box's.
    """
    value = doc.get('medianElapsedMs')
    return None if value is None else value / 1000.0
