#!/usr/bin/env python3
"""Does the block operator compose with the continuation it loses to?

    compose.py OUTDIR BINARY PARENTSJSON ROUNDTRIPDIR M34WORK [ALLOWANCE]

Sol review 10 §3's gate is head to head - the block against the same work handed
to m34/m22 - and this round's answer to it is no. That leaves a second question
the gate does not ask and a reader will: the block wins on the minority of seeds
where the schedule's step-down stalls, so is the *composition* worth anything?

Three arms from the same pinned parent, all at the same slice cap:

* `m34` - the continuation alone, the gate's control;
* `blockThenM34` - the same slice run from the layout the block left, which
  `roundtrip.py` has already written out as a fixture and the engine has already
  accepted;
* `blockOnly` - the block's own number, carried through so the composition can
  be read against both of its parts.

The composed arm is **not** equal-work with the control: it is the control's
work plus the block's. That is stated rather than corrected for, because the
honest question here is whether the composition reaches somewhere the control
cannot at any budget, and the budget curve in `matched.json` already shows what
more work buys the control on its own.
"""
import hashlib
import json
import os
import statistics
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import matched  # noqa: E402
import runlib  # noqa: E402


def main():
    outdir, binary, parents_json, roundtrip_dir, work = sys.argv[1:6]
    work = int(work)
    allowance = sys.argv[6] if len(sys.argv) > 6 else runlib.DEFAULT_ALLOWANCE
    parents = json.load(open(parents_json))['rows']
    trips = {c['seed']: c for c in
             json.load(open(f'{roundtrip_dir}/roundtrip.json'))['cells']}
    os.makedirs(outdir, exist_ok=True)
    result = {
        'binary': binary,
        'binarySha256': hashlib.sha256(open(binary, 'rb').read()).hexdigest(),
        'parents': parents_json, 'm34Work': work, 'allowance': allowance,
        'roundtripDir': roundtrip_dir, 'cells': [],
    }
    for parent in parents:
        seed = parent['seed']
        trip = trips.get(seed) or {}
        block_fixture = f'{roundtrip_dir}/seed{seed}-blockparent.json'
        cell = {'seed': seed, 'parentRawDepthMm': parent['rawDepthMm'],
                'blockOnlyDepthMm': trip.get('operatorDepthMm')}
        target = parent['rawDepthMm'] - matched.DEFAULT_DROP_MM
        base, _ = matched.run_m34(binary, seed, parent['fixture'], target, work,
                                  f'{outdir}/seed{seed}-m34.json', allowance)
        cell['m34'] = base
        if os.path.exists(block_fixture):
            composed, _ = matched.run_m34(
                binary, seed, block_fixture, target, work,
                f'{outdir}/seed{seed}-blockthen.json', allowance)
            cell['blockThenM34'] = composed
        for key in ('m34', 'blockThenM34'):
            row = cell.get(key)
            if row and row.get('rawSourceDepthMm') is None:
                row['rawSourceDepthMm'] = parent['rawDepthMm']
        print(f"seed{seed}: parent={parent['rawDepthMm']:.4f} "
              f"blockOnly={cell['blockOnlyDepthMm']} "
              f"m34={cell['m34'].get('rawSourceDepthMm')} "
              f"blockThenM34="
              f"{(cell.get('blockThenM34') or {}).get('rawSourceDepthMm')}",
              flush=True)
        result['cells'].append(cell)
        json.dump(result, open(f'{outdir}/compose.json', 'w'), indent=1)

    pairs = [(c['m34']['rawSourceDepthMm'],
              c['blockThenM34']['rawSourceDepthMm'])
             for c in result['cells']
             if c.get('blockThenM34', {}).get('rawSourceDepthMm') is not None
             and c['m34'].get('rawSourceDepthMm') is not None]
    diffs = [right - left for left, right in pairs]
    result['summary'] = {
        'seeds': len(pairs),
        'medianM34Mm': statistics.median([left for left, _ in pairs]),
        'medianBlockThenM34Mm': statistics.median([r for _, r in pairs]),
        'medianComposedMinusM34Mm': statistics.median(diffs) if diffs else None,
        'seedsComposedShallower': sum(1 for d in diffs if d < 0),
        'seedsM34Shallower': sum(1 for d in diffs if d > 0),
        'seedsEqual': sum(1 for d in diffs if d == 0),
        'medianComposedWorkUnits': statistics.median(
            [c['blockThenM34'].get('processWorkUnits') or 0
             for c in result['cells'] if c.get('blockThenM34')]),
        'medianM34WorkUnits': statistics.median(
            [c['m34'].get('processWorkUnits') or 0
             for c in result['cells']]),
    }
    json.dump(result, open(f'{outdir}/compose.json', 'w'), indent=1)
    print(json.dumps(result['summary'], indent=1))


if __name__ == '__main__':
    main()
