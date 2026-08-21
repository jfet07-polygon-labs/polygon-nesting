#!/usr/bin/env python3
"""The four pinned gates across four binaries, compared as whole documents.

    python3 gates4.py OUTDIR LABEL=BINARY [LABEL=BINARY ...]

The previous round's `gates.py` + `gatecompare.py` proved a three-way parity -
base commit, flag-off, flag-on - which answered "does arming the feature move a
gate?". This round has a second question that the three-way cannot answer,
because this round *edited the flag-on code*: the numeric-domain guard and the
outward-rounded slab bounds change what the certificate computes, so a
flag-on-only comparison against a **pre-patch flag-on** binary is the one that
says whether any skip decision moved.

So four binaries, one table:

  base-off  pre-patch, feature off   - the default build before this round
  base-on   pre-patch, feature on    - the certificate as the previous round shipped it
  off       post-patch, feature off  - the default build after this round
  on        post-patch, feature on   - the certificate as this round ships it

`base-off == off` says the default build is untouched by source that is all
behind a `#[cfg]`. `base-on == on` says the guard and the outward rounding
changed no verdict on the pinned gates. `off == on` is the property the feature
is held to. All three are digests of the whole document with only the
wall-clock and build-identity fields removed, so this is field-for-field and not
a comparison of the pinned scalars.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import gatelib as lib  # noqa: E402


def main():
    outdir = sys.argv[1]
    arms = {}
    for item in sys.argv[2:]:
        label, _, binary = item.partition('=')
        arms[label] = binary
    os.makedirs(outdir, exist_ok=True)

    result = {
        'arms': arms,
        'armSha256': {label: lib.hashlib.sha256(open(binary, 'rb').read()).hexdigest()
                      for label, binary in arms.items()},
        'gates': {},
        'perArm': {},
    }
    for label, binary in arms.items():
        result['perArm'][label] = {}
        for gate in lib.GATES:
            doc, wall, _ = lib.run_gate(binary, gate, outdir, label=label + '-')
            check = lib.gate_check(gate, doc)
            check['wallSeconds'] = wall
            check['docDigest'] = lib.doc_digest(doc)
            result['perArm'][label][gate[0]] = check
            print(f'{label} {gate[0]} hit={check.get("hit")} '
                  f'digest={check["docDigest"][:16]}', file=sys.stderr)
        json.dump(result, open(f'{outdir}/gates4.json', 'w'), indent=1)

    # The cross-arm table: one row per gate, one digest per arm.
    labels = list(arms)
    for gate in lib.GATES:
        tag = gate[0]
        digests = {label: result['perArm'][label][tag]['docDigest']
                   for label in labels}
        result['gates'][tag] = {
            'pinnedDepth': gate[5],
            'pinnedFingerprintPrefix': gate[6],
            'hit': {label: result['perArm'][label][tag].get('hit')
                    for label in labels},
            'digests': digests,
            'allDigestsEqual': len(set(digests.values())) == 1,
        }
    result['ALL_PASS'] = all(
        all(row['hit'].values()) for row in result['gates'].values())
    result['ALL_DIGESTS_EQUAL'] = all(
        row['allDigestsEqual'] for row in result['gates'].values())
    # The three named comparisons, spelled out so the reader does not have to
    # diff the table by eye.
    def pairwise(a, b):
        if a not in labels or b not in labels:
            return None
        return all(result['perArm'][a][g[0]]['docDigest']
                   == result['perArm'][b][g[0]]['docDigest'] for g in lib.GATES)
    result['DEFAULT_BUILD_UNTOUCHED'] = pairwise('base-off', 'off')
    result['CERTIFICATE_UNCHANGED'] = pairwise('base-on', 'on')
    result['FLAG_EQUIVALENT'] = pairwise('off', 'on')
    json.dump(result, open(f'{outdir}/gates4.json', 'w'), indent=1)
    print(json.dumps({k: v for k, v in result.items() if k != 'perArm'},
                     indent=1))


if __name__ == '__main__':
    main()
