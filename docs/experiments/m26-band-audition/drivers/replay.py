#!/usr/bin/env python3
"""Step 1 of the audition: do the twelve pinned parents still replay exact-valid?

    replay.py OUTDIR BINARY PARENTSJSON [ALLOWANCE]

A diffable copy of `docs/experiments/contact-block/drivers/replaycontrol.py`
(itself the round trip's control) with nothing changed but the import path and
the fact that it carries its own `replay` instead of importing one from a
driver that also runs an operator this round does not run.

The question and the way it is asked are the contact-block round's verbatim:
mode 34 from the pinned parent with a target the parent *already meets*
(`raw + 0.5`) and a 200,000-unit work cap.

**What that answer actually is, stated plainly, because the field names are
misleading.** With a target *above* the parent's depth the mode refuses on
`"persistent vacancy mode 34 final bound must be below the parent depth"` and
publishes nothing, so `exactValid` reads `false` on all twelve - as it does in
`docs/experiments/contact-block/evidence/replaycontrol.json`, which this driver
reproduces cell for cell. That `false` is "this run published nothing", not
"this parent is invalid". The parent verdict is upstream of it and is the thing
worth reading: `general_relaxed.rs:6408` runs the authoritative publication gate
`validate_and_measure_placements` on the parent, and `:6414`
`coupled_independent_source_depth`, **before** the bound comparison at `:6425`.
A cell that fails on the bound message has therefore passed the exact
publication gate, and its `parentIndependentDepthMm` and `parentFingerprint`
are the engine's own re-measure of the pinned layout under this round's binary,
this round's request and this round's `0.002` allowance.

So the audition's precondition is checked on three fields together -
`parentValidationPassed`, `depthMatchesPin`, `fingerprintMatchesPin` - and a
parent that misses any of them is not a parent this audition may run from.
"""
import hashlib
import json
import os
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import runlib  # noqa: E402

REPLAY_SPEC = 'past=1,rollback=0,work=200000,lanes=1,pconfirm=0'
TARGET_OFFSET_MM = 0.5


def population(doc):
    return ((doc.get('relaxedDiagnostics') or {})
            .get('coupledDynamicSeparator') or {}).get(
                'persistentVacancyPopulation')


def replay(binary, seed, fixture, target, out_path, allowance):
    args = [a.format(seed=seed) for a in runlib.ARGS]
    command = ([binary, runlib.REQUESTS['mixed-61']] + args
               + ['34', fixture, f'{target:.17g}', '', allowance])
    env = dict(os.environ)
    env['POLYGON_NESTING_PROFILE'] = '1'
    env['POLYGON_NESTING_COMPRESSION_SCHEDULE'] = REPLAY_SPEC
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
    stderr = (proc.stderr or b'').decode()[-1200:]
    try:
        doc = json.load(open(out_path))
    except json.JSONDecodeError:
        return None, wall, stderr, proc.returncode
    return doc, wall, stderr, proc.returncode


def main():
    outdir, binary, parents_json = sys.argv[1:4]
    allowance = sys.argv[4] if len(sys.argv) > 4 else runlib.DEFAULT_ALLOWANCE
    parents = json.load(open(parents_json))['rows']
    os.makedirs(outdir, exist_ok=True)
    result = {
        'binary': binary,
        'binarySha256': hashlib.sha256(open(binary, 'rb').read()).hexdigest(),
        'parents': parents_json,
        'allowance': allowance,
        'replaySpec': REPLAY_SPEC,
        'targetOffsetMm': TARGET_OFFSET_MM,
        'cells': [],
    }
    for parent in parents:
        seed = parent['seed']
        target = parent['rawDepthMm'] + TARGET_OFFSET_MM
        doc, wall, err, code = replay(
            binary, seed, parent['fixture'], target,
            f'{outdir}/seed{seed}-replay.json', allowance)
        cell = {'seed': seed,
                'fixture': parent['fixture'],
                'pinnedRawDepthMm': parent['rawDepthMm'],
                'pinnedFingerprint': parent['fingerprint']}
        if doc is None:
            cell['replayError'] = err[-800:]
            cell['exitCode'] = code
            print(f'seed{seed}: FAILED {err[-200:]}', flush=True)
        else:
            pop = population(doc) or {}
            reason = pop.get('failureReason') or ''
            cell['failureReason'] = reason
            cell['publishedExactValid'] = pop.get('exactValid')
            cell['publishedContractValid'] = pop.get('contractValid')
            cell['engineRawSourceDepthMm'] = pop.get('rawSourceDepthMm')
            cell['engineParentDepthMm'] = pop.get('parentIndependentDepthMm')
            cell['engineParentFingerprint'] = pop.get('parentFingerprint')
            # Past the bound message means past `validate_and_measure_placements`
            # and past `coupled_independent_source_depth`: the parent cleared
            # the authoritative exact publication gate.
            cell['parentValidationPassed'] = bool(
                'final bound must be below the parent depth' in reason
                or (not reason and pop.get('attempted')))
            cell['depthMatchesPin'] = (
                cell['engineRawSourceDepthMm'] is not None
                and abs(cell['engineRawSourceDepthMm']
                        - parent['rawDepthMm']) < 5e-7)
            cell['fingerprintMatchesPin'] = (
                cell['engineParentFingerprint'] == parent['fingerprint'])
            cell['replayWallSeconds'] = wall
            cell['exitCode'] = code
            # The harness floor. This process refused the mode on its bound and
            # ran no search at all, so every work unit on its counters is what
            # the benchmark spends *before* any deep operator sees the parent -
            # phase 0's construction and the parent's own validation. Both arms
            # of the audition pay it, so it is common mode and the audition
            # subtracts it to get each operator's own spend.
            counters = (doc.get('searchProfile') or {}).get('counters') or {}
            queries = counters.get('candidateQueries', 0)
            tests = counters.get('exactPairTests', 0)
            cell['harnessFloorCandidateQueries'] = queries
            cell['harnessFloorExactPairTests'] = tests
            cell['harnessFloorWorkUnits'] = queries + 5 * tests
            print(f"seed{seed}: pinned={parent['rawDepthMm']:.6f} "
                  f"engine={cell['engineRawSourceDepthMm']} "
                  f"parentGatePassed={cell['parentValidationPassed']} "
                  f"depthPin={cell['depthMatchesPin']} "
                  f"fpPin={cell['fingerprintMatchesPin']}", flush=True)
        result['cells'].append(cell)
        json.dump(result, open(f'{outdir}/replay.json', 'w'), indent=1)
    judged = [c for c in result['cells'] if 'parentValidationPassed' in c]
    result['summary'] = {
        'cells': len(result['cells']),
        'judged': len(judged),
        'parentValidationPassed': sum(1 for c in judged
                                      if c['parentValidationPassed']),
        'depthMatchesPin': sum(1 for c in judged if c['depthMatchesPin']),
        'fingerprintMatchesPin': sum(1 for c in judged
                                     if c['fingerprintMatchesPin']),
        'publishedExactValid': sum(1 for c in judged
                                   if c['publishedExactValid']),
        'publishedContractValid': sum(1 for c in judged
                                      if c['publishedContractValid']),
    }
    total = len(result['cells'])
    result['ALL_PARENTS_VALID'] = (
        result['summary']['judged'] == total
        and result['summary']['parentValidationPassed'] == total
        and result['summary']['depthMatchesPin'] == total
        and result['summary']['fingerprintMatchesPin'] == total)
    json.dump(result, open(f'{outdir}/replay.json', 'w'), indent=1)
    print(json.dumps({'summary': result['summary'],
                      'ALL_PARENTS_VALID': result['ALL_PARENTS_VALID']},
                     indent=1))


if __name__ == '__main__':
    main()
