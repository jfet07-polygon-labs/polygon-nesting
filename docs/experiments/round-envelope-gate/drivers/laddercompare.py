#!/usr/bin/env python3
"""Two ladders, two binaries: every verdict must be the same number.

    laddercompare.py LADDER_A.json LADDER_B.json OUT.json

The ladder in this round's evidence was collected **twice**, on two different
binaries. The first pass ran before the instrument was committed; the commit
changes the binary whether or not it changes behaviour, because the benchmark
embeds `engineCommit`, `engineWorktreeDirty` and `relevantSourceTreeSha256`, and
because the door's parse was split into a testable function on the way in. The
campaign's provenance rule is that evidence must be regenerable from a clean
rebuild of the round's own HEAD, and "the refactor cannot have changed anything"
is an argument rather than a measurement - which is what the previous round
refused to accept from itself and re-collected instead.

So this compares the two, cell by cell, on every quantity that is not a clock:
the published depth, the placement fingerprint, the schedule's step digest, and
its confirmation counts. Wall and work counters are reported as ratios and
asserted on nothing - the box is shared and they are the only fields that are
allowed to move.
"""
import json
import statistics
import sys

EXACT = ('rawSourceDepthMm', 'fingerprint', 'exactValid', 'contractValid',
         'schedule_stepDigest', 'schedule_confirmationsAttempted',
         'schedule_confirmationsAccepted', 'schedule_confirmationsRefused',
         'schedule_stepsTaken', 'schedule_finalDepthMm',
         'schedule_workUnits', 'reportedKernelMode')
TIMED = ('operatorWallSeconds', 'processWallSeconds', 'msPerConfirmation')


def cells_of(document):
    out = {}
    for cell in document['cells']:
        for label, row in cell['arms'].items():
            out[(cell['seed'], label)] = row
    return out


def main():
    a_path, b_path, out_path = sys.argv[1:4]
    a, b = cells_of(json.load(open(a_path))), cells_of(json.load(open(b_path)))
    shared = sorted(set(a) & set(b))
    mismatches = []
    ratios = {name: [] for name in TIMED}
    for key in shared:
        for field in EXACT:
            if a[key].get(field) != b[key].get(field):
                mismatches.append({'cell': f'seed{key[0]} {key[1]}',
                                   'field': field,
                                   'a': a[key].get(field),
                                   'b': b[key].get(field)})
        for field in TIMED:
            first, second = a[key].get(field), b[key].get(field)
            if first and second:
                ratios[field].append(second / first)
    result = {
        'a': a_path, 'b': b_path,
        'cellsInA': len(a), 'cellsInB': len(b), 'cellsCompared': len(shared),
        'onlyInA': sorted(f'seed{s} {l}' for s, l in set(a) - set(b)),
        'onlyInB': sorted(f'seed{s} {l}' for s, l in set(b) - set(a)),
        'exactFieldsCompared': list(EXACT),
        'mismatches': mismatches,
        'ALL_IDENTICAL': not mismatches,
        'timingRatiosBOverA': {
            name: {'median': statistics.median(values),
                   'range': [min(values), max(values)], 'n': len(values)}
            for name, values in ratios.items() if values},
    }
    json.dump(result, open(out_path, 'w'), indent=1)
    print(json.dumps({k: v for k, v in result.items()
                      if k != 'exactFieldsCompared'}, indent=1))
    raise SystemExit(0 if result['ALL_IDENTICAL'] else 1)


if __name__ == '__main__':
    main()
