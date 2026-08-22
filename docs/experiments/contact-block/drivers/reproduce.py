#!/usr/bin/env python3
"""Re-derives the committed depths from a freshly built binary.

    reproduce.py BINARY EVIDENCEJSON SPEC [OUT]

The protocol's requirement is that every quoted number is reproducible from the
**committed** tree, and a hash comparison cannot establish that: this crate does
not build bit-reproducibly across invocations, so two binaries from the same
source have different SHA-256s and the difference proves nothing either way.

What does establish it is re-deriving the numbers. This rebuilds nothing itself -
`build.sh` does that - and simply re-runs the twelve-parent probe on the binary
it is given, then compares each seed's `finalDepthMm` against the committed
`evidence/blockprobe.json` **exactly**, not to a tolerance. The operator is a
deterministic function of `(parent, settings)`, so exact equality is the right
test and anything else would be hiding a drift behind an epsilon.
"""
import hashlib
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import runlib  # noqa: E402


def main():
    binary, evidence_json, spec = sys.argv[1:4]
    spec = spec.replace(';', ',')
    out_path = sys.argv[4] if len(sys.argv) > 4 \
        else '/var/lib/t3/tmp/cblock/out/reproduce.json'
    committed = json.load(open(evidence_json))
    want = {}
    for cell in committed['cells']:
        arm = cell['specs'].get(spec)
        if arm and 'finalDepthMm' in arm:
            want[cell['seed']] = arm['finalDepthMm']
    if not want:
        raise SystemExit(f'no cells for spec {spec!r} in {evidence_json}')

    parents = json.load(open(
        os.path.join(os.path.dirname(os.path.abspath(__file__)),
                     'parents12.json')))['rows']
    result = {
        'binary': binary,
        'binarySha256': hashlib.sha256(open(binary, 'rb').read()).hexdigest(),
        'committedEvidence': evidence_json,
        'committedBinarySha256': committed['binarySha256'],
        'spec': spec,
        'note': 'the two binary hashes differ by construction; the depths are '
                'what is compared',
        'cells': [],
    }
    outdir = os.path.dirname(out_path) or '.'
    for parent in parents:
        seed = parent['seed']
        if seed not in want:
            continue
        doc, _, err, code = runlib.probe(
            binary, 'mixed-61', seed, parent['fixture'],
            {'POLYGON_NESTING_CONTACT_BLOCK': spec},
            f'{outdir}/reproduce-seed{seed}.json', timeout=3600)
        got = None if doc is None else doc.get('finalDepthMm')
        cell = {'seed': seed, 'committedDepthMm': want[seed],
                'rebuiltDepthMm': got, 'identical': got == want[seed]}
        if doc is None:
            cell['error'] = err[-400:]
            cell['exitCode'] = code
        print(f"seed{seed}: committed={want[seed]!r} rebuilt={got!r} "
              f"identical={cell['identical']}", flush=True)
        result['cells'].append(cell)
    result['summary'] = {
        'cells': len(result['cells']),
        'identical': sum(1 for c in result['cells'] if c['identical']),
        'ALL_IDENTICAL': all(c['identical'] for c in result['cells']),
    }
    json.dump(result, open(out_path, 'w'), indent=1)
    print(json.dumps(result['summary'], indent=1))


if __name__ == '__main__':
    main()
