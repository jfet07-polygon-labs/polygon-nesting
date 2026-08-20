#!/usr/bin/env python3
"""The whole gate round: both binaries, a repeat, and the paired document diff.

    python3 gate_round.py <off binary> <on binary> [outdir]

Three runs, not two. The repeat of the flag-off binary is the **noise floor**:
without it a document comparison between two binaries cannot distinguish "the
flag changed something" from "this document was never stable in the first
place", which is precisely how the inherited `lib.doc_digest` was able to look
like an instrument while being none (see `docdiff.py`).

Emits one evidence document: per gate the pinned-scalar check for all three
runs, the three digests, and the leaf-path diff of flag-off against
flag-on-unarmed measured against the off-vs-off floor.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import docdiff  # noqa: E402
import lib  # noqa: E402

off_binary, on_binary = sys.argv[1], sys.argv[2]
outdir = sys.argv[3] if len(sys.argv) > 3 else '/var/lib/t3/tmp/se2gateround'
os.makedirs(outdir, exist_ok=True)

RUNS = [('off', off_binary), ('off-repeat', off_binary),
        ('on-unarmed', on_binary)]

result = {'binaries': {'off': off_binary, 'on': on_binary}, 'gates': {}}
documents = {}
for label, binary in RUNS:
    for gate in lib.GATES:
        doc, wall, _ = lib.run_gate(binary, gate, outdir, label=label + '-')
        documents[(label, gate[0])] = (doc, wall)

for gate in lib.GATES:
    tag = gate[0]
    entry = {}
    for label, _binary in RUNS:
        doc, wall = documents[(label, tag)]
        check = lib.gate_check(gate, doc)
        check['wallSeconds'] = round(wall, 2)
        check['docDigest'] = lib.doc_digest(doc)
        entry[label] = check
    digests = {entry[label]['docDigest'] for label, _ in RUNS}
    entry['ALL_DIGESTS_MATCH'] = len(digests) == 1
    floor = docdiff.differing(f'{outdir}/off-{tag}.json',
                              f'{outdir}/off-repeat-{tag}.json')
    claim = docdiff.differing(f'{outdir}/off-{tag}.json',
                              f'{outdir}/on-unarmed-{tag}.json')
    entry['noiseFloorPaths'] = sorted(floor)
    entry['claimPaths'] = sorted(claim)
    entry['claimPathsOutsideFloor'] = sorted(claim - floor)
    result['gates'][tag] = entry

result['ALL_PASS'] = all(
    result['gates'][g[0]][label]['hit']
    for g in lib.GATES for label, _ in RUNS)
result['ALL_DIGESTS_MATCH'] = all(
    result['gates'][g[0]]['ALL_DIGESTS_MATCH'] for g in lib.GATES)
result['CLAIM_PATHS_OUTSIDE_FLOOR'] = sorted({
    path for g in lib.GATES
    for path in result['gates'][g[0]]['claimPathsOutsideFloor']})

print(json.dumps(result, indent=1))
json.dump(result, open(f'{outdir}/gate-round.json', 'w'), indent=1)
