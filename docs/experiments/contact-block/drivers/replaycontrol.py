#!/usr/bin/env python3
"""The round trip's control: the UNTOUCHED parents through the same replay.

    replaycontrol.py OUTDIR BINARY PARENTSJSON [ALLOWANCE]

`roundtrip.py` reports whatever the engine says about the operator's output. A
number is only a finding if the same question asked of a layout nobody touched
gives a different answer, and this asks it. Same binary, same replay spec, same
target offset, same reader - the only difference is that the fixture is the
committed parent rather than the operator's output.
"""
import hashlib
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import matched  # noqa: E402
import roundtrip  # noqa: E402
import runlib  # noqa: E402


def main():
    outdir, binary, parents_json = sys.argv[1:4]
    allowance = sys.argv[4] if len(sys.argv) > 4 else runlib.DEFAULT_ALLOWANCE
    parents = json.load(open(parents_json))['rows']
    os.makedirs(outdir, exist_ok=True)
    result = {
        'binary': binary,
        'binarySha256': hashlib.sha256(open(binary, 'rb').read()).hexdigest(),
        'parents': parents_json, 'allowance': allowance, 'cells': [],
    }
    for parent in parents:
        seed = parent['seed']
        target = parent['rawDepthMm'] + 0.5
        doc, wall, err, code = roundtrip.replay(
            binary, seed, parent['fixture'], target,
            f'{outdir}/seed{seed}-control.json', allowance)
        cell = {'seed': seed, 'parentRawDepthMm': parent['rawDepthMm']}
        if doc is None:
            cell['replayError'] = err[-800:]
            print(f'seed{seed}: FAILED {err[-200:]}', flush=True)
        else:
            pop = matched.population(doc) or {}
            cell['engineExactValid'] = pop.get('exactValid')
            cell['engineContractValid'] = pop.get('contractValid')
            cell['engineRawSourceDepthMm'] = pop.get('rawSourceDepthMm')
            cell['replayWallSeconds'] = wall
            print(f"seed{seed}: parent={parent['rawDepthMm']:.6f} "
                  f"engine={cell['engineRawSourceDepthMm']} "
                  f"exact={cell['engineExactValid']} "
                  f"contract={cell['engineContractValid']}", flush=True)
        result['cells'].append(cell)
        json.dump(result, open(f'{outdir}/replaycontrol.json', 'w'), indent=1)
    judged = [c for c in result['cells'] if 'engineExactValid' in c]
    result['summary'] = {
        'cells': len(result['cells']),
        'engineExactValid': sum(1 for c in judged if c['engineExactValid']),
        'engineContractValid': sum(1 for c in judged
                                   if c['engineContractValid']),
    }
    json.dump(result, open(f'{outdir}/replaycontrol.json', 'w'), indent=1)
    print(json.dumps(result['summary'], indent=1))


if __name__ == '__main__':
    main()
