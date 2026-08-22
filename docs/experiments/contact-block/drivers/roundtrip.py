#!/usr/bin/env python3
"""Hand the block's output back to the engine and let the engine judge it.

    roundtrip.py OUTDIR BINARY PARENTSJSON SPEC [ALLOWANCE]

The operator validates its own steps with
`validation::general_polygon::validate_publication`, which is the exact gate -
but a module checking its own output is not an independent check, and this
campaign has already been burned once by a negative that was measuring its own
harness argument (`next-generation-engine-plan.md`, the joint-replacement
lesson). So every cell here does the full round trip:

1. run the operator on the pinned parent through the diagnostic door, and take
   the moved placements it emits;
2. write them out as a **pinned-parent fixture** claiming the operator's own
   depth. `load_pinned_vacancy_parent` re-derives that depth from the placements
   and hard-errors on a mismatch, so a wrong depth claim fails the run rather
   than producing a plausible number;
3. replay that fixture through mode 34 at a target above its own depth, so the
   engine loads it, confirms it, and publishes it through its own path;
4. read `failureReason`, `exactValid`, `contractValid` and `rawSourceDepthMm`
   off the engine's own report.

**The field that decides is `failureReason`, not `exactValid`.** The first
version of this driver read `exactValid`, saw `False`, checked it against a
control that also said `False`, and concluded the operator's output was judged
exactly as its parent was. It was not: the control said `False` because the
driver had handed it a target above its own depth, and the operator's output
said `False` because *it was refused as a parent outright* — "pieces ... overlap
on the canonical collision grid". Two different events with the same boolean.

So a cell passes when the engine does **not** refuse the layout at
`compression schedule parent validation`, and the engine's re-derived depth
agrees with the operator's to the grid. `PARENT_REFUSAL` is the prefix
`general_relaxed.rs:6413` writes, and matching on it is the whole check.

The fixture's `expectedPlacementFingerprint` is deliberately a provenance label
rather than a digest: `is_placement_fingerprint` leaves non-digest labels alone,
and inventing a digest here would mean reimplementing the engine's fingerprint
in Python and pinning the reimplementation instead of the engine.
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
import matched  # noqa: E402

GRID_MM = 1e-6
# The prefix `general_relaxed.rs:6413` writes when
# `validate_and_measure_placements` refuses a layout handed to mode 34 as a
# parent. This is the string the round trip exists to look for.
PARENT_REFUSAL = 'compression schedule parent validation'


def fixture_from(parent_fixture_path, placements, depth_mm, spec):
    source = json.load(open(parent_fixture_path))
    return {
        'schemaVersion': source['schemaVersion'],
        'description': f'contact-block output, spec `{spec}`, from '
                       f'{os.path.basename(parent_fixture_path)}',
        'requestSha256': source['requestSha256'],
        'expectedPlacementFingerprint': 'contact-block-operator-output',
        'reportedDepthMm': depth_mm,
        'independentDepthMm': depth_mm,
        'provenance': {'source': parent_fixture_path, 'spec': spec},
        'settings': source['settings'],
        'placements': placements,
    }


def replay(binary, seed, fixture, target, out_path, allowance):
    """Mode 34 on the block's output, with a target it already meets.

    A target above the parent's own depth means the schedule has nothing to
    walk down to and publishes the parent it was handed, which is exactly the
    question: does the engine accept this layout.
    """
    args = [a.format(seed=seed) for a in runlib.ARGS]
    command = ([binary, runlib.REQUESTS['mixed-61']] + args
               + ['34', fixture, f'{target:.17g}', '', allowance])
    env = dict(os.environ)
    env['POLYGON_NESTING_PROFILE'] = '1'
    env['POLYGON_NESTING_COMPRESSION_SCHEDULE'] = \
        'past=1,rollback=0,work=200000,lanes=1,pconfirm=0'
    env.pop('POLYGON_NESTING_CONTACT_BLOCK', None)
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
    outdir, binary, parents_json, spec = sys.argv[1:5]
    spec = spec.replace(';', ',')
    allowance = sys.argv[5] if len(sys.argv) > 5 else runlib.DEFAULT_ALLOWANCE
    parents = json.load(open(parents_json))['rows']
    os.makedirs(outdir, exist_ok=True)
    result = {
        'binary': binary,
        'binarySha256': hashlib.sha256(open(binary, 'rb').read()).hexdigest(),
        'parents': parents_json, 'spec': spec, 'allowance': allowance,
        'cells': [],
    }
    for parent in parents:
        seed = parent['seed']
        doc, _, err, code = runlib.probe(
            binary, 'mixed-61', seed, parent['fixture'],
            {'POLYGON_NESTING_CONTACT_BLOCK': spec},
            f'{outdir}/seed{seed}-block.json', allowance=allowance,
            timeout=3600)
        cell = {'seed': seed, 'parentRawDepthMm': parent['rawDepthMm']}
        if doc is None:
            cell['error'] = err[-500:]
            result['cells'].append(cell)
            continue
        cell['operatorDepthMm'] = doc.get('finalDepthMm')
        cell['operatorDeltaMm'] = doc.get('deltaMm')
        placements = doc.get('proposalPlacements')
        if not placements:
            cell['note'] = 'operator returned no proposal'
            result['cells'].append(cell)
            print(f"seed{seed}: no proposal", flush=True)
            continue
        fixture_path = f'{outdir}/seed{seed}-blockparent.json'
        json.dump(fixture_from(parent['fixture'], placements,
                               cell['operatorDepthMm'], spec),
                  open(fixture_path, 'w'), indent=1)
        target = cell['operatorDepthMm'] + 0.5
        replayed, wall, err, code = replay(
            binary, seed, fixture_path, target,
            f'{outdir}/seed{seed}-replay.json', allowance)
        if replayed is None:
            cell['replayError'] = err[-800:]
            cell['replayExit'] = code
            print(f"seed{seed}: REPLAY FAILED {err[-300:]}", flush=True)
            result['cells'].append(cell)
            json.dump(result, open(f'{outdir}/roundtrip.json', 'w'), indent=1)
            continue
        pop = matched.population(replayed) or {}
        cell['engineFailureReason'] = pop.get('failureReason')
        cell['engineRefusedAsParent'] = bool(
            (pop.get('failureReason') or '').startswith(PARENT_REFUSAL))
        cell['engineExactValid'] = pop.get('exactValid')
        cell['engineContractValid'] = pop.get('contractValid')
        cell['engineRawSourceDepthMm'] = pop.get('rawSourceDepthMm')
        cell['engineIndependentDepthMm'] = \
            replayed.get('independentUsedLongAxisDepthMm')
        cell['replayWallSeconds'] = wall
        engine_depth = cell['engineRawSourceDepthMm']
        cell['agreesToGrid'] = (
            engine_depth is not None
            and abs(engine_depth - cell['operatorDepthMm']) <= GRID_MM)
        cell['engineDeltaVsParentMm'] = (
            parent['rawDepthMm'] - engine_depth
            if engine_depth is not None else None)
        print(f"seed{seed}: operator={cell['operatorDepthMm']:.6f} "
              f"engine={engine_depth} refusedAsParent="
              f"{cell['engineRefusedAsParent']} "
              f"contract={cell['engineContractValid']} "
              f"agrees={cell['agreesToGrid']} "
              f"| {(cell['engineFailureReason'] or '')[:90]}", flush=True)
        result['cells'].append(cell)
        json.dump(result, open(f'{outdir}/roundtrip.json', 'w'), indent=1)

    judged = [c for c in result['cells'] if 'engineExactValid' in c]
    deltas = [c['engineDeltaVsParentMm'] for c in judged
              if c['engineDeltaVsParentMm'] is not None]
    result['summary'] = {
        'cells': len(result['cells']),
        'judged': len(judged),
        'engineRefusedAsParent': sum(1 for c in judged
                                     if c['engineRefusedAsParent']),
        'engineAcceptedAsParent': sum(1 for c in judged
                                      if not c['engineRefusedAsParent']),
        'engineExactValid': sum(1 for c in judged if c['engineExactValid']),
        'engineContractValid': sum(1 for c in judged
                                   if c['engineContractValid']),
        'agreesToGrid': sum(1 for c in judged if c['agreesToGrid']),
        'medianEngineDeltaMm': statistics.median(deltas) if deltas else None,
        'replayFailures': sum(1 for c in result['cells'] if 'replayError' in c),
        'noProposal': sum(1 for c in result['cells'] if 'note' in c),
    }
    json.dump(result, open(f'{outdir}/roundtrip.json', 'w'), indent=1)
    print(json.dumps(result['summary'], indent=1))


if __name__ == '__main__':
    main()
