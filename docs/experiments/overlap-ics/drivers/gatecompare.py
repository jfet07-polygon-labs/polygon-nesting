#!/usr/bin/env python3
"""The two gate binaries as documents, not only as pinned scalars.

    python3 gatecompare.py <base-dir> <meas-dir> [out] [base-label] [meas-label]

Re-reads the gate documents the two binaries wrote, re-derives every pinned
check, and compares the **whole documents** with `gatelib.VOLATILE` stripped -
the protocol's own field list, which removes the elapsed-derived summary
statistics, the binary hash and the worktree identity and nothing else.

`ALL_PASS` on both plus `WHOLE_DOCUMENT_IDENTITY` is the strong form of the
claim: compiling `overlap-ics` in changes nothing a gate document can see.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import gatelib  # noqa: E402

base_dir = sys.argv[1]
meas_dir = sys.argv[2]
out = sys.argv[3] if len(sys.argv) > 3 else '/var/lib/t3/tmp/overlapics'
# `gates.py` prefixes each document with its label, so a comparison across two
# labelled runs has to be told both labels rather than assuming `base`/`meas`.
base_label = sys.argv[4] if len(sys.argv) > 4 else 'base'
meas_label = sys.argv[5] if len(sys.argv) > 5 else 'meas'

result = {'baseDir': base_dir, 'measDir': meas_dir,
          'baseLabel': base_label, 'measLabel': meas_label, 'gates': {}}
identical = True
base_pass = True
meas_pass = True
for gate in gatelib.GATES:
    tag = gate[0]
    with open(f'{base_dir}/{base_label}-{tag}.json') as handle:
        base = json.load(handle)
    with open(f'{meas_dir}/{meas_label}-{tag}.json') as handle:
        meas = json.load(handle)
    base_check = gatelib.gate_check(gate, base)
    meas_check = gatelib.gate_check(gate, meas)
    base_digest = gatelib.doc_digest(base)
    meas_digest = gatelib.doc_digest(meas)
    identical = identical and base_digest == meas_digest
    base_pass = base_pass and bool(base_check.get('hit'))
    meas_pass = meas_pass and bool(meas_check.get('hit'))
    result['gates'][tag] = {
        'pinnedDepthMm': gate[5],
        'pinnedFingerprintPrefix': gate[6],
        'base': base_check,
        'meas': meas_check,
        'baseDocDigest': base_digest,
        'measDocDigest': meas_digest,
        'documentsIdentical': base_digest == meas_digest,
    }
result['BASE_ALL_PASS'] = base_pass
result['MEAS_ALL_PASS'] = meas_pass
result['WHOLE_DOCUMENT_IDENTITY'] = identical
print(json.dumps(result, indent=1))
os.makedirs(out, exist_ok=True)
with open(f'{out}/gates.json', 'w') as handle:
    json.dump(result, handle, indent=1)
