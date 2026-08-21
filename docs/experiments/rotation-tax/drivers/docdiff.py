#!/usr/bin/env python3
"""Field-level comparison of two benchmark documents, with a noise floor.

`lib.doc_digest`, as inherited from the `constructor-inner-certificate` drivers,
hashes a whole document after dropping a hand-written VOLATILE key list. That
list was incomplete: two runs of the *same* binary on the same gate produced
different digests, so a digest mismatch between two binaries said nothing on its
own and a digest match would have been luck. (`lib.py` is fixed now; this note
records what the fix was for. The `sol5/se2-rigidity-certificate` branch's own
`docdiff.py` did not have this bug.) Measured before the fix, off-vs-off:

    g1 bdfdecb4... vs d2872ad1...   g2 09d4226a... vs f566b887...
    g3 2f8a707e... vs 6284e99a...   g4 29089c43... vs b6804532...

all four differ, same binary, same arguments.

So the instrument here is a *paired* one. It flattens both documents to leaf
paths and reports which paths differ, and the caller is expected to compute two
diffs:

  * a **noise floor** - two runs of the same binary - which is the set of paths
    that vary for reasons that have nothing to do with the change;
  * the **claim** - the flag-off binary against the flag-on one - which is only
    meaningful relative to that floor.

The claim "the flag changes nothing on the default path" is supported when the
claim's differing-path set is contained in the noise floor's, and refuted by a
single path outside it. That is a weaker statement than "bit-identical
documents", and it is the strongest one this instrument can actually make.

    python3 docdiff.py <a.json> <b.json> [<floor-a.json> <floor-b.json>]
"""
import json
import sys


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
        # `repr` on floats so 1.0 and 1 do not compare equal by accident.
        out[path] = repr(node) if isinstance(node, float) else node
    return out


def differing(first_path, second_path):
    first, second = leaves(json.load(open(first_path))), \
        leaves(json.load(open(second_path)))
    keys = set(first) | set(second)
    return {key for key in keys if first.get(key) != second.get(key)}


if __name__ == '__main__':
    claim = differing(sys.argv[1], sys.argv[2])
    result = {'a': sys.argv[1], 'b': sys.argv[2],
              'differingPaths': len(claim)}
    if len(sys.argv) > 4:
        floor = differing(sys.argv[3], sys.argv[4])
        outside = sorted(claim - floor)
        result.update({
            'floorA': sys.argv[3], 'floorB': sys.argv[4],
            'noiseFloorPaths': len(floor),
            'claimPathsOutsideFloor': len(outside),
            'outside': outside[:60],
            # The whole point: contained in the floor, or not.
            'CONTAINED_IN_NOISE_FLOOR': not outside,
        })
    print(json.dumps(result, indent=1))
