#!/usr/bin/env python3
"""Does this build, with every new key off, reproduce the base commit?

    reproduce.py OUT BASEBIN NEWBIN REQUESTS SEEDS WORKUNITS

Whole documents, at a work budget so both sides are deterministic and
load-independent, through the coordinator rather than around it. The new
build's arm is spelled out key by key - `m34wall=0,m34entry=0,m34skip=0,
m34bit=0` - because "the default is the same" is the claim under test.

Fields the two documents cannot agree on by construction - every timing, the
executable hash, the worktree status, and the `scheduleSlice` block this round
adds - are removed from both sides before the comparison and listed in the
output, so the diff that remains is a diff of the search.
"""
import hashlib
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import runlib  # noqa: E402

# Provenance and this round's own new telemetry. Removed from *both* sides, so
# a field that only one build emits cannot hide a change in the other.
VOLATILE = {
    'executableSha256', 'engineWorktreeStatus', 'engineWorktreeDirty',
    'engineCommit', 'relevantSourceTreeSha256', 'rustflags', 'buildProfile',
    'scheduleSlice', 'cpuModel', 'actualThreads', 'requestedThreads',
    # A clock reading per archive admission, paired with an occupancy.
    'occupancyOverTime',
    # The action rows' and publication events' own clock, which has no suffix.
    'seconds',
}
# Every elapsed-derived statistic, by suffix: two processes on a shared box do
# not agree on a millisecond and never have, and this comparison is about the
# search rather than about the schedule of the box it ran on.
VOLATILE_SUFFIXES = ('Seconds', 'Ms')


def volatile(key):
    return key in VOLATILE or key.endswith(VOLATILE_SUFFIXES)


def strip(node):
    if isinstance(node, dict):
        return {key: strip(value) for key, value in node.items()
                if not volatile(key)}
    if isinstance(node, list):
        return [strip(value) for value in node]
    return node


def digest(doc):
    return hashlib.sha256(
        json.dumps(strip(doc), sort_keys=True,
                   separators=(',', ':')).encode()).hexdigest()


def main():
    out = sys.argv[1]
    base_bin, new_bin = sys.argv[2], sys.argv[3]
    requests = sys.argv[4].split(',')
    seeds = [int(v) for v in sys.argv[5].split(',')]
    units = sys.argv[6]
    off = 'm34wall=0,m34entry=0,m34skip=0,m34drop=0,m34probe=0,m34bit=0'
    result = {'baseBinary': base_bin, 'newBinary': new_bin,
              'offArm': off, 'workUnits': units, 'rows': [],
              'volatileFieldsRemoved': sorted(VOLATILE),
              'volatileSuffixesRemoved': list(VOLATILE_SUFFIXES)}
    ok = True
    for request in requests:
        for seed in seeds:
            base_spec = runlib.spec_for(seed, 'work', units, True)
            new_spec = runlib.spec_for(seed, 'work', units, True, off)
            tag = f'{request}-s{seed}'
            base_doc, _, base_err = runlib.run(
                base_bin, request, seed, base_spec,
                f'{runlib.OUT}/{out}/base-{tag}.json')
            new_doc, _, new_err = runlib.run(
                new_bin, request, seed, new_spec,
                f'{runlib.OUT}/{out}/new-{tag}.json')
            base_digest, new_digest = digest(base_doc), digest(new_doc)
            row = {'tag': tag, 'baseSpec': base_spec, 'newSpec': new_spec,
                   'baseDigest': base_digest, 'newDigest': new_digest,
                   'equal': base_digest == new_digest,
                   'baseRawDepthMm':
                       base_doc.get('portfolio', {})
                       .get('incumbent', {}).get('rawDepthMm'),
                   'newRawDepthMm':
                       new_doc.get('portfolio', {})
                       .get('incumbent', {}).get('rawDepthMm')}
            if base_err.strip() or new_err.strip():
                row['stderr'] = (base_err[-200:], new_err[-200:])
            ok = ok and row['equal']
            result['rows'].append(row)
            print(f'{tag}: equal={row["equal"]} '
                  f'{row["baseRawDepthMm"]} / {row["newRawDepthMm"]}',
                  flush=True)
    result['allEqual'] = ok
    os.makedirs(f'{runlib.OUT}/{out}', exist_ok=True)
    json.dump(result, open(f'{runlib.OUT}/{out}/reproduce.json', 'w'),
              indent=1)
    print(f'allEqual={ok}')
    return 0 if ok else 1


if __name__ == '__main__':
    raise SystemExit(main())
