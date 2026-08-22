#!/usr/bin/env python3
"""Determinism across two processes: the hard gate.

    determinism.py OUTDIR BINARY PARENTSJSON SPEC [ALLOWANCE]

Two separate processes, same binary, same parent, same spec. The whole
operator document must be byte-identical after the wall-clock fields are
stripped - the round table, every model bound, every validated delta, and the
moved placements.

The placements are the part that matters most and the part a weaker check would
miss: a run that produced the same *depth* from a different layout would pass a
scalar comparison and still be non-deterministic. `elapsedMs` is the only field
allowed to move.
"""
import hashlib
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import gatelib as lib  # noqa: E402
import runlib  # noqa: E402


def main():
    outdir, binary, parents_json, spec = sys.argv[1:5]
    spec = spec.replace(';', ',')
    allowance = sys.argv[5] if len(sys.argv) > 5 else runlib.DEFAULT_ALLOWANCE
    parents = json.load(open(parents_json))['rows']
    os.makedirs(outdir, exist_ok=True)
    result = {
        'binary': binary,
        'binarySha256': hashlib.sha256(open(binary, 'rb').read()).hexdigest(),
        'parents': parents_json, 'spec': spec, 'cells': [],
    }
    for parent in parents:
        seed = parent['seed']
        digests, depths = [], []
        for run in (0, 1):
            doc, _, err, code = runlib.probe(
                binary, 'mixed-61', seed, parent['fixture'],
                {'POLYGON_NESTING_CONTACT_BLOCK': spec},
                f'{outdir}/seed{seed}-run{run}.json', allowance=allowance,
                timeout=3600)
            if doc is None:
                digests.append(f'ERROR:{code}:{err[-200:]}')
                depths.append(None)
                continue
            digests.append(lib.doc_digest(doc))
            depths.append(doc.get('finalDepthMm'))
        cell = {
            'seed': seed,
            'digests': digests,
            'depths': depths,
            'identical': len(set(digests)) == 1 and not any(
                d.startswith('ERROR') for d in digests
                if isinstance(d, str)),
        }
        print(f"seed{seed}: identical={cell['identical']} "
              f"{digests[0][:16]} {digests[1][:16]} depths={depths}",
              flush=True)
        result['cells'].append(cell)
        json.dump(result, open(f'{outdir}/determinism.json', 'w'), indent=1)
    result['summary'] = {
        'cells': len(result['cells']),
        'identical': sum(1 for c in result['cells'] if c['identical']),
        'ALL_IDENTICAL': all(c['identical'] for c in result['cells']),
    }
    json.dump(result, open(f'{outdir}/determinism.json', 'w'), indent=1)
    print(json.dumps(result['summary'], indent=1))


if __name__ == '__main__':
    main()
